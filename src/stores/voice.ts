import type { RealtimeChannel } from "@supabase/supabase-js";
import { toast } from "sonner";
import { create } from "zustand";
import { useAlerts } from "./alerts";
import {
  boostScreenShareAudio,
  captureScreen,
  configureRemoteAudio,
  createMicrophonePipeline,
  createRemoteAudioElement,
  setMicrophoneGain,
  stopMicrophonePipeline,
  type MicrophonePipeline,
} from "@/lib/voice-media";
import { playAppSound } from "@/lib/app-sounds";
import { playSoundboardUrl, preloadSoundboardClip, type SoundboardPlayback } from "@/lib/soundboard-audio";
import { createCloudflareScreenPublisher, createCloudflareScreenSubscriber } from "@/lib/cloudflare-realtime";
import { monitorVoiceActivity, type VoiceActivityMonitor } from "@/lib/voice-activity";
import { supabase } from "@/lib/supabase";
import type {
  VoiceConnectionStatus,
  VoiceParticipant,
  VoicePresence,
  VoiceRoom,
  VoiceSignal,
} from "@/lib/types";
import { useChat } from "./chat";
import { usePreferences } from "./preferences";

interface VoiceJoinParticipant {
  user_id: string;
  session_id: string;
  joined_at: string;
  last_seen_at: string;
  sharing_screen: boolean;
}

interface VoiceJoinResponse {
  status: "joined" | "conflict";
  conversation_id?: string;
  generation?: string;
  started_at?: string;
  started_by?: string;
  joined_at?: string;
  participants?: VoiceJoinParticipant[];
}

interface RpcStatus {
  status?: "ok" | "left" | "not_found";
  conversation_id?: string;
}

interface VoiceState {
  rooms: Record<string, VoiceRoom>;
  participants: Record<string, VoiceParticipant[]>;
  /** Per-user_id voice activity, for the in-call talking indicator/HUD. */
  speaking: Record<string, boolean>;
  level: Record<string, number>;
  /** Per-user_id mute/deafen state read from the partner's presence
   * broadcast (see syncRoomPresence) - the local user's own mute/deafened
   * fields below are the source of truth for themselves. */
  remoteMuted: Record<string, boolean>;
  remoteDeafened: Record<string, boolean>;
  status: VoiceConnectionStatus;
  activeConversationId: string | null;
  sessionId: string | null;
  muted: boolean;
  deafened: boolean;
  sharingScreen: boolean;
  localScreenStream: MediaStream | null;
  remoteScreenStream: MediaStream | null;
  error: string | null;
  init: (userId: string) => () => void;
  join: (conversationId: string, takeover?: boolean) => Promise<void>;
  leave: () => Promise<void>;
  toggleMute: () => void;
  toggleDeafen: () => void;
  startScreenShare: () => Promise<void>;
  stopScreenShare: () => Promise<void>;
  retryConnection: () => void;
  /** Hard fallback for when the automatic ICE-restart/signaling recovery
   * gets stuck: fully leaves and rejoins the same room instead of trying
   * to repair the existing connection. */
  forceReconnect: () => Promise<void>;
}

const ICE_SERVERS: RTCIceServer[] = [
  { urls: ["stun:stun.cloudflare.com:3478"] },
];
const HEARTBEAT_MS = 45_000;
const TURN_CREDENTIAL_TTL_SAFETY_MS = 10 * 60_000;
const TURN_CREDENTIAL_REQUEST_TIMEOUT_MS = 5_000;
const CHANNEL_TIMEOUT_MS = 15_000;
const SIGNAL_SEND_ATTEMPTS = 2;
const SIGNAL_SEND_TIMEOUT_MS = 2_500;
const SIGNAL_OUTBOX_MAX = 64;
const SIGNAL_OUTBOX_TTL_MS = 20_000;
const RELAY_RECOVERY_MAX_ATTEMPTS = 5;
const CONNECTION_FAILURE_DELAY_MS = 6_000;
// A connection can remain formally "connected" while its direct path is
// shedding enough audio packets to sound robotic. Sample the receiving RTP
// stats a few times before doing anything so normal Wi-Fi blips never cause a
// route switch, then prefer the managed relay for the rest of that call.
const VOICE_HEALTH_SAMPLE_MS = 3_000;
const VOICE_HEALTH_SAMPLES_BEFORE_RELAY = 2;
const VOICE_HEALTH_MIN_PACKETS = 40;
const VOICE_HEALTH_PACKET_LOSS_RATIO = 0.08;
const VOICE_HEALTH_JITTER_SECONDS = 0.08;

let currentUserId: string | null = null;
let discoveryChannel: RealtimeChannel | null = null;
let roomChannel: RealtimeChannel | null = null;
let roomSubscribed = false;
let microphone: MicrophonePipeline | null = null;
let peerConnection: RTCPeerConnection | null = null;
let soundboardDataChannel: RTCDataChannel | null = null;
let remoteAudio: HTMLAudioElement | null = null;
let remoteAudioStream: MediaStream | null = null;
let localScreenStream: MediaStream | null = null;
let localScreenTrack: MediaStreamTrack | null = null;
// Stops the boosted audio track and its processing graph (see
// boostScreenShareAudio) - separate from localScreenStream's own tracks,
// which stay the raw capture used for the muted local preview.
let screenAudioCleanup: (() => void) | null = null;
let cloudflareScreenConnection: RTCPeerConnection | null = null;
let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
let disconnectTimer: ReturnType<typeof setTimeout> | null = null;
let voiceHealthTimer: ReturnType<typeof setInterval> | null = null;
let signalingRecoveryTimer: ReturnType<typeof setTimeout> | null = null;
let preferencesUnsubscribe: (() => void) | null = null;
let beforeUnloadHandler: (() => void) | null = null;
let networkOnlineHandler: (() => void) | null = null;
let remoteSessionId: string | null = null;
let polite = false;
let makingOffer = false;
let ignoreOffer = false;
let isSettingRemoteAnswerPending = false;
let restartAttempted = false;
let connectionRecoveryInProgress = false;
let peerRebuildAttempts = 0;
let relayRecoveryAttempted = false;
let relayRecoveryInProgress = false;
let relayRecoveryAttempts = 0;
let relayRecoveryRetryAfter = 0;
let directFallbackAttempted = false;
let forceRelayTransport = false;
let unhealthyVoiceSamples = 0;
let healthAssessmentInProgress = false;
let previousInboundAudioStats: {
  packetsReceived: number;
  packetsLost: number;
} | null = null;
let signalingRecoveryAttempts = 0;
let disconnecting = false;
let localVoiceActivity: VoiceActivityMonitor | null = null;
let remoteVoiceActivity: VoiceActivityMonitor | null = null;
let pendingCandidates: RTCIceCandidateInit[] = [];
let pendingVoiceSignals: { signal: VoiceSignal; expiresAt: number }[] = [];
let flushingVoiceSignals = false;
let signalFlushRunId = 0;
let signalFlushRetryTimer: ReturnType<typeof setTimeout> | null = null;
const receivedVoiceSignalIds = new Map<string, number>();
let turnCredentialRequestId = 0;
let lastRemoteSoundboardAt = 0;
const remoteSoundboardPlaybacks = new Map<string, SoundboardPlayback>();
const cancelledRemoteSoundboardPlaybacks = new Set<string>();
const pendingSoundboardReadiness = new Map<string, { resolve: (ready: boolean) => void; timeout: number }>();

function hasTurnRelay(servers = activeIceServers): boolean {
  return servers.some((server) => {
    const urls = Array.isArray(server.urls) ? server.urls : [server.urls];
    return urls.some((url) => typeof url === "string" && /^turns?:/i.test(url));
  });
}

function getVoiceConnectionConfiguration(): RTCConfiguration {
  return {
    iceServers: activeIceServers,
    iceCandidatePoolSize: 4,
    // "relay" is deliberately only enabled after a measured failure. Direct
    // P2P stays the lowest-latency route when it is healthy.
    iceTransportPolicy: forceRelayTransport ? "relay" : "all",
  };
}

function clearVoiceHealthMonitor(): void {
  if (voiceHealthTimer) window.clearInterval(voiceHealthTimer);
  voiceHealthTimer = null;
  unhealthyVoiceSamples = 0;
  previousInboundAudioStats = null;
}

function startVoiceHealthMonitor(connection: RTCPeerConnection): void {
  clearVoiceHealthMonitor();
  voiceHealthTimer = window.setInterval(() => {
    void assessVoiceHealth(connection);
  }, VOICE_HEALTH_SAMPLE_MS);
}

function getSelectedVoiceRoute(report: RTCStatsReport): {
  roundTripTime?: number;
  localCandidateType?: string;
} | null {
  const transport = Array.from(report.values()).find((entry) => entry.type === "transport") as
    | (RTCStats & { selectedCandidatePairId?: string })
    | undefined;
  let pair = transport?.selectedCandidatePairId
    ? report.get(transport.selectedCandidatePairId)
    : undefined;
  if (!pair) {
    pair = Array.from(report.values()).find((entry) => {
      if (entry.type !== "candidate-pair") return false;
      const stats = entry as RTCStats & { state?: string; nominated?: boolean; selected?: boolean };
      return stats.state === "succeeded" && (stats.nominated === true || stats.selected === true);
    });
  }
  if (!pair || pair.type !== "candidate-pair") return null;

  const pairStats = pair as RTCStats & {
    currentRoundTripTime?: number;
    localCandidateId?: string;
  };
  const localCandidate = pairStats.localCandidateId
    ? report.get(pairStats.localCandidateId)
    : undefined;
  const candidateStats = localCandidate as (RTCStats & { candidateType?: string }) | undefined;
  return {
    roundTripTime: pairStats.currentRoundTripTime,
    localCandidateType: candidateStats?.candidateType,
  };
}

async function assessVoiceHealth(connection: RTCPeerConnection): Promise<void> {
  if (
    peerConnection !== connection ||
    connection.connectionState !== "connected" ||
    disconnecting ||
    relayRecoveryInProgress ||
    forceRelayTransport ||
    relayRecoveryAttempted ||
    healthAssessmentInProgress
  ) {
    return;
  }

  healthAssessmentInProgress = true;
  try {
    const report = await connection.getStats();
    const route = getSelectedVoiceRoute(report);
    // If ICE already selected TURN naturally, changing policy cannot improve
    // the route. Keep the call stable rather than repeatedly forcing a restart.
    if (route?.localCandidateType === "relay") {
      unhealthyVoiceSamples = 0;
      return;
    }
    const inbound = Array.from(report.values()).find((entry) => {
      if (entry.type !== "inbound-rtp") return false;
      const stats = entry as RTCStats & { kind?: unknown; mediaType?: unknown };
      return stats.kind === "audio" || stats.mediaType === "audio";
    }) as (RTCStats & {
      packetsReceived?: unknown;
      packetsLost?: unknown;
      jitter?: unknown;
    }) | undefined;

    if (!inbound) return;
    const packetsReceived = typeof inbound.packetsReceived === "number" ? inbound.packetsReceived : 0;
    const packetsLost = typeof inbound.packetsLost === "number" ? Math.max(0, inbound.packetsLost) : 0;
    const jitter = typeof inbound.jitter === "number" ? inbound.jitter : 0;
    const previous = previousInboundAudioStats;
    previousInboundAudioStats = { packetsReceived, packetsLost };
    if (!previous) return;

    const receivedDelta = Math.max(0, packetsReceived - previous.packetsReceived);
    const lostDelta = Math.max(0, packetsLost - previous.packetsLost);
    const totalPackets = receivedDelta + lostDelta;
    if (totalPackets < VOICE_HEALTH_MIN_PACKETS) {
      unhealthyVoiceSamples = 0;
      return;
    }

    const lossRatio = lostDelta / totalPackets;
    const unhealthy =
      lossRatio >= VOICE_HEALTH_PACKET_LOSS_RATIO ||
      jitter >= VOICE_HEALTH_JITTER_SECONDS;
    unhealthyVoiceSamples = unhealthy ? unhealthyVoiceSamples + 1 : 0;
    if (!unhealthy && !forceRelayTransport && !relayRecoveryAttempted) {
      relayRecoveryAttempts = 0;
      relayRecoveryRetryAfter = 0;
    }
    if (unhealthyVoiceSamples < VOICE_HEALTH_SAMPLES_BEFORE_RELAY) return;

    console.warn("Voice path is unhealthy; attempting TURN relay recovery.", {
      lossRatio: Number(lossRatio.toFixed(3)),
      jitterMs: Math.round(jitter * 1000),
      roundTripTimeMs: typeof route?.roundTripTime === "number"
        ? Math.round(route.roundTripTime * 1000)
        : null,
      localCandidateType: route?.localCandidateType ?? "unknown",
    });
    clearVoiceHealthMonitor();
    const recovered = await attemptRelayRecovery("audio quality");
    if (
      !recovered &&
      peerConnection === connection &&
      connection.connectionState === "connected" &&
      !forceRelayTransport &&
      !relayRecoveryAttempted
    ) {
      startVoiceHealthMonitor(connection);
    }
  } catch (error) {
    // Stats are purely diagnostic. A browser that declines one sample must not
    // destabilise an otherwise healthy call.
    console.debug("Voice health sample was unavailable", error);
  } finally {
    healthAssessmentInProgress = false;
  }
}

function setSpeaking(userId: string, value: boolean): void {
  useVoice.setState((state) =>
    state.speaking[userId] === value
      ? state
      : { speaking: { ...state.speaking, [userId]: value } }
  );
}

function setLevel(userId: string, value: number): void {
  useVoice.setState((state) => {
    const previous = state.level[userId] ?? 0;
    // Round to avoid a state update (and downstream HUD emit) on every
    // 50ms tick when the level barely moved.
    if (Math.abs(previous - value) < 0.03 && value !== 0) return state;
    return { level: { ...state.level, [userId]: value } };
  });
}

// The analyser taps the pipeline's gain node directly (see below), which
// sits upstream of the mute gate (applyLocalMuteState only flips
// track.enabled on the outbound MediaStreamTrack) - so it keeps reading live
// mic input while muted. Force speaking/level to false/0 in that case rather
// than trusting the analyser, so the HUD can't visibly react to your voice
// while you're muted or deafened.
function isLocalMicSilenced(): boolean {
  const state = useVoice.getState();
  return state.muted || state.deafened;
}

// Prefer tapping the pipeline's own gain node in its own already-running
// AudioContext (the exact signal actually being sent) over asking a second,
// independent AudioContext to consume the same MediaStreamTrack.
function startLocalVoiceActivity(pipeline: MicrophonePipeline): VoiceActivityMonitor | null {
  if (!currentUserId) return null;
  const userId = currentUserId;
  const source = pipeline.context && pipeline.gain
    ? { context: pipeline.context, node: pipeline.gain }
    : pipeline.outputStream;
  return monitorVoiceActivity(
    source,
    (value) => setSpeaking(userId, value && !isLocalMicSilenced()),
    (value) => setLevel(userId, isLocalMicSilenced() ? 0 : value)
  );
}

usePreferences.subscribe((state, previousState) => {
  if (state.soundboardVolume !== previousState.soundboardVolume) {
    for (const playback of remoteSoundboardPlaybacks.values()) {
      playback.setVolume(state.soundboardVolume);
    }
  }
});
let activeIceServers: RTCIceServer[] = ICE_SERVERS;
let turnCredentialsExpireAt = 0;
let joinAttempt = 0;

export const useVoice = create<VoiceState>()((set, get) => ({
  rooms: {},
  participants: {},
  speaking: {},
  level: {},
  remoteMuted: {},
  remoteDeafened: {},
  status: "idle",
  activeConversationId: null,
  sessionId: null,
  muted: false,
  deafened: false,
  sharingScreen: false,
  localScreenStream: null,
  remoteScreenStream: null,
  error: null,

  init: (userId) => initializeVoice(userId),

  join: async (conversationId, takeover = false) => {
    const attempt = ++joinAttempt;
    if (get().status === "joining") { await disconnectLocal(true); takeover = true; }
    if (
      get().activeConversationId === conversationId &&
      get().status !== "idle"
    ) {
      await disconnectLocal(true);
      takeover = true;
    }

    if (get().activeConversationId && get().activeConversationId !== conversationId) {
      if (!takeover) {
        useAlerts.getState().show({
          severity: "neutral",
          message: "You are already in another voice channel.",
          actions: [{ label: "Switch", confirm: true, onClick: () => void get().join(conversationId, true) }],
        });
        return;
      }
      await disconnectLocal(true);
    }

    set({ status: "joining", error: null });
    const sessionId = crypto.randomUUID();

    try {
      microphone = await createPreferredMicrophone();
      if (attempt !== joinAttempt) { await stopMicrophonePipeline(microphone); microphone = null; return; }
      applyLocalMuteState();
      localVoiceActivity?.stop();
      localVoiceActivity = startLocalVoiceActivity(microphone);

      const { data, error } = await supabase.rpc("join_voice_room", {
        p_conversation_id: conversationId,
        p_session_id: sessionId,
        p_takeover: takeover,
      });
      if (error) throw new Error(error.message);

      const response = data as VoiceJoinResponse;
      if (response.status === "conflict") {
        await stopMicrophonePipeline(microphone);
        microphone = null;
        set({ status: "idle", error: null });
        useAlerts.getState().show({
          severity: "neutral",
          message: "Voice is active on another device.",
          actions: [{ label: "Take over", confirm: true, onClick: () => void get().join(conversationId, true) }],
        });
        return;
      }

      if (
        !response.conversation_id ||
        !response.generation ||
        !response.started_at ||
        !response.started_by
      ) {
        throw new Error("The voice room returned an incomplete response.");
      }

      const room: VoiceRoom = {
        conversation_id: response.conversation_id,
        generation: response.generation,
        started_at: response.started_at,
        started_by: response.started_by,
        updated_at: new Date().toISOString(),
      };
      const participants = (response.participants ?? []).map(
        (participant): VoiceParticipant => ({
          ...participant,
          conversation_id: room.conversation_id,
        })
      );

      set((state) => ({
        rooms: { ...state.rooms, [room.conversation_id]: room },
        participants: {
          ...state.participants,
          [room.conversation_id]: participants,
        },
        activeConversationId: room.conversation_id,
        sessionId,
        status: "connecting",
        error: null,
      }));

      // Voice signaling and direct ICE should come up immediately; a slow
      // credential broker must not hold the call setup path hostage. The TURN
      // servers are installed for any ICE restart/recovery once they arrive.
      void refreshTurnCredentials();
      try {
        await connectRoomChannel(room, sessionId);
      } catch (error) {
        if (get().sessionId !== sessionId) throw error;
        try {
          await connectRoomChannel(room, sessionId);
        } catch (retryError) {
          if (get().sessionId !== sessionId) throw retryError;
      console.warn("Voice signaling is still unavailable; retrying in the background.", retryError);
          set({ status: "reconnecting", error: null });
          scheduleSignalingRecovery(room, sessionId);
        }
      }
      playAppSound("voice_join");
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Voice could not start.";
      if (get().sessionId === sessionId) {
        await disconnectLocal(true);
      } else {
        await stopMicrophonePipeline(microphone);
        microphone = null;
      }
      set({ status: "idle", error: message });
      useAlerts.getState().show({ severity: "danger", message });
    }
  },

  leave: async () => {
    joinAttempt += 1;
    const wasActive = Boolean(get().activeConversationId);
    await disconnectLocal(true);
    if (wasActive) playAppSound("voice_leave");
  },

  toggleMute: () => {
    const state = get();
    if (state.deafened) {
      set({ muted: false, deafened: false });
    } else {
      set({ muted: !state.muted });
    }
    applyLocalMuteState();
    applyRemoteAudioPreferences();
    void updateRoomPresence();
    if (get().muted && currentUserId) { setSpeaking(currentUserId, false); setLevel(currentUserId, 0); }
    playAppSound(get().muted ? "mute_on" : "mute_off");
  },

  toggleDeafen: () => {
    const next = !get().deafened;
    set({
      deafened: next,
      muted: next ? true : get().muted,
    });
    applyLocalMuteState();
    applyRemoteAudioPreferences();
    void updateRoomPresence();
    if (next && currentUserId) { setSpeaking(currentUserId, false); setLevel(currentUserId, 0); }
    playAppSound(next ? "deafen_on" : "deafen_off");
  },

  startScreenShare: async () => {
    if (!get().activeConversationId || localScreenTrack) return;
    try {
      const stream = await captureScreen();
      const track = stream.getVideoTracks()[0];
      if (!track) {
        for (const mediaTrack of stream.getTracks()) mediaTrack.stop();
        throw new Error("The selected source did not provide a video track.");
      }

      track.contentHint = "motion";
      localScreenStream = stream;
      localScreenTrack = track;
      track.onended = () => {
        void stopLocalScreen(true);
      };
      set({ sharingScreen: true, localScreenStream: stream });

      // What actually gets sent - same video, boosted audio (raw captured
      // system/window audio is commonly much quieter than what's audible;
      // see boostScreenShareAudio). localScreenStream/localScreenTrack above
      // stay the raw capture, used for the muted local preview and cleanup.
      const boosted = boostScreenShareAudio(stream);
      screenAudioCleanup = boosted.cleanup;

      // Screen share goes out via Cloudflare's dedicated connection only -
      // never the main call's peerConnection. That connection carries live
      // voice audio and must never compete with (or, worse, have screen
      // audio merged into) it; if Cloudflare can't be reached, screen
      // sharing just isn't available this session rather than falling back
      // onto a path that risks the actual call.
      const conversationId = get().activeConversationId!;
      let published = false;
      for (let attempt = 0; attempt < 2; attempt += 1) {
        try {
          if (attempt) await new Promise((resolve) => window.setTimeout(resolve, 550));
          const cloudflare = await createCloudflareScreenPublisher(conversationId, boosted.stream);
          cloudflareScreenConnection = cloudflare.connection;
          sendScreenPublished(cloudflare.sessionId, cloudflare.trackNames);
          published = true;
          break;
        } catch {
          // Retried once above; falls through to the failure branch below.
        }
      }
      if (!published) {
        throw new Error("Screen sharing isn't available right now. Try again shortly.");
      }

      if (!stream.getAudioTracks().length) {
        useAlerts.getState().show({ severity: "warning", message: "This source did not include audio. Enable Share system audio in the capture picker when available." });
      }
      await updateRoomPresence();
      await sendHeartbeat();
    } catch (error) {
      await stopLocalScreen(false);
      if (error instanceof DOMException && error.name === "NotAllowedError") return;
      useAlerts.getState().show({
        severity: "danger",
        message: error instanceof Error ? error.message : "Screen sharing could not start.",
      });
    }
  },

  stopScreenShare: () => stopLocalScreen(true),

  retryConnection: () => {
    if (!peerConnection || !remoteSessionId) return;
    restartAttempted = false;
    useVoice.setState({ status: "reconnecting", error: null });
    void attemptIceRestart();
  },

  forceReconnect: async () => {
    const conversationId = get().activeConversationId;
    if (!conversationId) return;
    await get().leave();
    await get().join(conversationId, true);
  },
}));

async function refreshTurnCredentials(force = false): Promise<boolean> {
  const state = useVoice.getState();
  const conversationId = state.activeConversationId;
  if (!conversationId) return false;
  if (!force && (turnCredentialsExpireAt - Date.now()) > TURN_CREDENTIAL_TTL_SAFETY_MS) {
    return hasTurnRelay();
  }
  const requestId = ++turnCredentialRequestId;
  try {
    const { data, error } = await supabase.functions.invoke("realtime-credentials", {
      body: { conversationId },
      timeout: TURN_CREDENTIAL_REQUEST_TIMEOUT_MS,
    });
    const latest = useVoice.getState();
    if (
      requestId !== turnCredentialRequestId ||
      latest.activeConversationId !== conversationId
    ) {
      return hasTurnRelay();
    }

    const candidate = data as { iceServers?: unknown; expiresAt?: unknown } | null;
    if (error || !Array.isArray(candidate?.iceServers)) {
      const cachedRelayUsable = hasTurnRelay() && turnCredentialsExpireAt > Date.now();
      if (!cachedRelayUsable) {
        activeIceServers = ICE_SERVERS;
        turnCredentialsExpireAt = 0;
      }
      console.warn("Cloudflare TURN is unavailable; continuing with direct WebRTC.", error);
      return cachedRelayUsable;
    }
    const servers = candidate.iceServers.filter((server): server is RTCIceServer => {
      if (!server || typeof server !== "object") return false;
      const value = server as RTCIceServer;
      return typeof value.urls === "string" || Array.isArray(value.urls);
    });
    if (!servers.length || !hasTurnRelay(servers)) {
      const cachedRelayUsable = hasTurnRelay() && turnCredentialsExpireAt > Date.now();
      if (!cachedRelayUsable) {
        activeIceServers = ICE_SERVERS;
        turnCredentialsExpireAt = 0;
      }
      console.warn("Cloudflare TURN returned no usable relay candidates.");
      return cachedRelayUsable;
    }
    activeIceServers = servers;
    turnCredentialsExpireAt = typeof candidate.expiresAt === "number"
      ? candidate.expiresAt
      : Date.now() + 12 * 60 * 60_000;

    // Existing media keeps its selected route; this updates the server list
    // for a subsequent ICE restart without forcing an unnecessary interruption.
    try {
      const connection = peerConnection;
      if (connection) {
        connection.setConfiguration({
          ...connection.getConfiguration(),
          iceServers: activeIceServers,
        });
      }
    } catch (error) {
      console.debug("TURN servers will be applied on the next peer rebuild.", error);
    }
    return true;
  } catch (error) {
    const latest = useVoice.getState();
    if (
      requestId !== turnCredentialRequestId ||
      latest.activeConversationId !== conversationId
    ) {
      return hasTurnRelay();
    }
    const cachedRelayUsable = hasTurnRelay() && turnCredentialsExpireAt > Date.now();
    if (!cachedRelayUsable) {
      activeIceServers = ICE_SERVERS;
      turnCredentialsExpireAt = 0;
    }
    console.warn("Cloudflare TURN credential request failed; keeping direct WebRTC available.", error);
    return cachedRelayUsable;
  }
}
function initializeVoice(userId: string): () => void {
  currentUserId = userId;
  void loadVoiceDiscovery();
  subscribeToVoiceDiscovery(userId);

  preferencesUnsubscribe?.();
  preferencesUnsubscribe = usePreferences.subscribe((state, previous) => {
    if (state.inputVolume !== previous.inputVolume) {
      setMicrophoneGain(microphone, state.inputVolume);
    }
    if (
      state.outputVolume !== previous.outputVolume ||
      state.outputDeviceId !== previous.outputDeviceId ||
      state.partnerVoiceBoost !== previous.partnerVoiceBoost
    ) {
      applyRemoteAudioPreferences();
    }
    if (
      state.inputDeviceId !== previous.inputDeviceId &&
      useVoice.getState().activeConversationId
    ) {
      void replaceMicrophone();
    }
  });

  if (beforeUnloadHandler) {
    window.removeEventListener("beforeunload", beforeUnloadHandler);
  }
  beforeUnloadHandler = () => {
    void disconnectLocal(true);
  };
  window.addEventListener("beforeunload", beforeUnloadHandler);

  if (networkOnlineHandler) window.removeEventListener("online", networkOnlineHandler);
  networkOnlineHandler = () => {
    // Network/VPN changes can invalidate an ICE pair after a call has connected.
    if (useVoice.getState().activeConversationId && peerConnection?.connectionState !== "connected") {
      restartAttempted = false;
      void attemptIceRestart();
    }
  };
  window.addEventListener("online", networkOnlineHandler);
  return () => {
    if (beforeUnloadHandler) {
      window.removeEventListener("beforeunload", beforeUnloadHandler);
      beforeUnloadHandler = null;
    }
    if (networkOnlineHandler) {
      window.removeEventListener("online", networkOnlineHandler);
      networkOnlineHandler = null;
    }
    preferencesUnsubscribe?.();
    preferencesUnsubscribe = null;
    if (discoveryChannel) {
      void supabase.removeChannel(discoveryChannel);
      discoveryChannel = null;
    }
    void disconnectLocal(true);
    currentUserId = null;
    useVoice.setState({ rooms: {}, participants: {} });
  };
}

async function loadVoiceDiscovery(): Promise<void> {
  const [roomsResult, participantsResult] = await Promise.all([
    supabase.from("voice_rooms").select("*"),
    // Ordered so both callers' clients agree on participant order from the
    // start (unordered selects have no guaranteed row order) - applyParticipant
    // then preserves that order across updates instead of reshuffling it.
    supabase.from("voice_participants").select("*").order("joined_at", { ascending: true }),
  ]);
  if (roomsResult.error || participantsResult.error) {
    console.error("Voice discovery failed", roomsResult.error, participantsResult.error);
    return;
  }

  const rooms: Record<string, VoiceRoom> = {};
  for (const room of (roomsResult.data ?? []) as VoiceRoom[]) {
    rooms[room.conversation_id] = room;
  }

  const participants: Record<string, VoiceParticipant[]> = {};
  for (const participant of (participantsResult.data ?? []) as VoiceParticipant[]) {
    participants[participant.conversation_id] = [
      ...(participants[participant.conversation_id] ?? []),
      participant,
    ];
  }
  useVoice.setState({ rooms, participants });
}

function subscribeToVoiceDiscovery(userId: string): void {
  if (discoveryChannel) void supabase.removeChannel(discoveryChannel);

  discoveryChannel = supabase
    .channel("voice-state:" + userId)
    .on(
      "postgres_changes",
      { event: "INSERT", schema: "public", table: "voice_rooms" },
      (payload) => applyRoom(payload.new as VoiceRoom)
    )
    .on(
      "postgres_changes",
      { event: "UPDATE", schema: "public", table: "voice_rooms" },
      (payload) => applyRoom(payload.new as VoiceRoom)
    )
    .on(
      "postgres_changes",
      { event: "DELETE", schema: "public", table: "voice_rooms" },
      (payload) => removeRoom(payload.old as Partial<VoiceRoom>)
    )
    .on(
      "postgres_changes",
      { event: "INSERT", schema: "public", table: "voice_participants" },
      (payload) => applyParticipant(payload.new as VoiceParticipant, true)
    )
    .on(
      "postgres_changes",
      { event: "UPDATE", schema: "public", table: "voice_participants" },
      (payload) => applyParticipant(payload.new as VoiceParticipant, false)
    )
    .on(
      "postgres_changes",
      { event: "DELETE", schema: "public", table: "voice_participants" },
      (payload) => removeParticipant(payload.old as Partial<VoiceParticipant>)
    )
    .subscribe();
}

function applyRoom(room: VoiceRoom): void {
  if (!room.conversation_id) return;
  useVoice.setState((state) => ({
    rooms: { ...state.rooms, [room.conversation_id]: room },
  }));
}

function removeRoom(partial: Partial<VoiceRoom>): void {
  const conversationId = partial.conversation_id;
  if (!conversationId || disconnecting) return;
  const active = useVoice.getState().activeConversationId === conversationId;
  // With RLS, Realtime DELETE payloads only include the primary key. A deleted
  // old room can therefore arrive after its replacement has been created. Check
  // the active lease before treating the event as a real room expiration.
  if (active) {
    void verifyActiveLease(conversationId);
    return;
  }
  useVoice.setState((state) => {
    const rooms = { ...state.rooms };
    const participants = { ...state.participants };
    delete rooms[conversationId];
    delete participants[conversationId];
    return { rooms, participants };
  });
  if (active) {
    void disconnectLocal(false);
    useAlerts.getState().show({ severity: "danger", message: "The voice channel expired after losing its connection." });
  }
}

function applyParticipant(
  participant: VoiceParticipant,
  announce: boolean
): void {
  if (!participant.conversation_id || !participant.user_id) return;
  const existing =
    useVoice.getState().participants[participant.conversation_id] ?? [];
  const wasPresent = existing.some(
    (entry) => entry.user_id === participant.user_id
  );

  useVoice.setState((state) => {
    const existingList = state.participants[participant.conversation_id] ?? [];
    const index = existingList.findIndex((entry) => entry.user_id === participant.user_id);
    // Update in place rather than filter-then-push - the periodic
    // heartbeat (last_seen_at) fires this same UPDATE path for whichever
    // side happens to tick most recently, and re-appending on every update
    // used to shuffle that participant to the end of the list each time,
    // making the HUD/call-bar order flip unpredictably between callers.
    const nextList = index === -1
      ? [...existingList, participant]
      : existingList.map((entry, i) => (i === index ? participant : entry));
    return {
      participants: {
        ...state.participants,
        [participant.conversation_id]: nextList,
      },
    };
  });

  // Removing a sender track does not reliably fire `ended` on the remote
  // WebRTC track. The room heartbeat is the authoritative share-state signal,
  // so clear the preview as soon as the active partner reports it is off.
  const state = useVoice.getState();
  if (
    participant.user_id !== currentUserId &&
    participant.conversation_id === state.activeConversationId &&
    !participant.sharing_screen &&
    state.remoteScreenStream
  ) {
    useVoice.setState({ remoteScreenStream: null });
  }

  if (
    announce &&
    !wasPresent &&
    participant.user_id !== currentUserId
  ) {
    playAppSound("voice_join");
    toast.info("Your partner joined voice.");
  }
}

function removeParticipant(partial: Partial<VoiceParticipant>): void {
  const conversationId = partial.conversation_id;
  const userId = partial.user_id;
  if (!conversationId || !userId) return;
  // A deliberate leave or takeover produces the same DELETE event as an
  // expired lease. The local teardown owns that event, so it must not surface
  // an "expired" error or tear down a replacement session.
  if (disconnecting && userId === currentUserId) return;
  if (
    userId === currentUserId &&
    useVoice.getState().activeConversationId === conversationId
  ) {
    void verifyActiveLease(conversationId);
    return;
  }

  const currentParticipant = (
    useVoice.getState().participants[conversationId] ?? []
  ).find((participant) => participant.user_id === userId);
  if (
    partial.session_id &&
    currentParticipant?.session_id &&
    partial.session_id !== currentParticipant.session_id
  ) {
    return;
  }

  useVoice.setState((state) => ({
    participants: {
      ...state.participants,
      [conversationId]: (
        state.participants[conversationId] ?? []
      ).filter((participant) => participant.user_id !== userId),
    },
  }));

  if (useVoice.getState().activeConversationId !== conversationId) return;
  if (userId === currentUserId) {
    void disconnectLocal(false);
    useAlerts.getState().show({ severity: "danger", message: "Your voice session expired." });
  } else {
    closePeerConnection();
    playAppSound("voice_leave");
    useVoice.setState({ status: "solo", error: null });
  }
}

async function createPreferredMicrophone(): Promise<MicrophonePipeline> {
  const preferences = usePreferences.getState();
  const pipeline = await createMicrophonePipeline(
    preferences.inputDeviceId,
    preferences.inputVolume,
    preferences.noiseSuppression
  );
  if (pipeline.fellBackToDefault) {
    useAlerts.getState().show({ severity: "warning", message: "The selected microphone is unavailable; using the Windows default." });
  }
  return pipeline;
}

async function replaceMicrophone(): Promise<void> {
  if (!useVoice.getState().activeConversationId) return;
  try {
    const replacement = await createPreferredMicrophone();
    const old = microphone;
    microphone = replacement;
    applyLocalMuteState();
    localVoiceActivity?.stop();
    localVoiceActivity = startLocalVoiceActivity(replacement);

    const nextTrack = replacement.outputStream.getAudioTracks()[0] ?? null;
    const sender = peerConnection
      ?.getSenders()
      .find((entry) => entry.track?.kind === "audio");
    if (sender) {
      await sender.replaceTrack(nextTrack);
    } else if (peerConnection && nextTrack) {
      peerConnection.addTrack(nextTrack, replacement.outputStream);
    }

    await stopMicrophonePipeline(old);
  } catch (error) {
    useAlerts.getState().show({
      severity: "danger",
      message: error instanceof Error ? error.message : "The microphone could not be changed.",
    });
  }
}

async function connectRoomChannel(
  room: VoiceRoom,
  sessionId: string
): Promise<void> {
  const previousChannel = roomChannel;
  roomChannel = null;
  roomSubscribed = false;
  if (previousChannel) await supabase.removeChannel(previousChannel);

  const topic =
    "voice:" + room.conversation_id + ":" + room.generation;
  const channel = supabase.channel(topic, {
    config: {
      private: true,
      // Wait for server receipt for SDP/ICE packets. This is not an end-peer
      // delivery acknowledgement, but it prevents send() from appearing
      // successful while the Realtime socket has not accepted the signal.
      broadcast: { ack: true, self: false },
      presence: { key: sessionId },
    },
  });
  roomChannel = channel;

  channel
    .on("broadcast", { event: "signal" }, (message) => {
      void receiveVoiceSignal(message.payload);
    })
    .on("broadcast", { event: "soundboard" }, (message) => {
      handleRemoteSoundboardPlay(message.payload);
    })
    .on("broadcast", { event: "soundboard-stop" }, (message) => {
      handleRemoteSoundboardStop(message.payload);
    })
    .on("presence", { event: "sync" }, () => {
      syncRoomPresence();
    });

  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const timeout = window.setTimeout(() => {
      if (settled) return;
      settled = true;
      reject(new Error("Voice signaling timed out."));
    }, CHANNEL_TIMEOUT_MS);

    channel.subscribe(async (status) => {
      const state = useVoice.getState();
      if (
        roomChannel !== channel ||
        state.sessionId !== sessionId ||
        state.activeConversationId !== room.conversation_id
      ) return;

      if (status === "SUBSCRIBED") {
        try {
          roomSubscribed = true;
          await updateRoomPresence();
          if (roomChannel !== channel) return;
          clearSignalingRecovery();
          startHeartbeat();
          void flushPendingVoiceSignals();
          syncRoomPresence();
          if (!settled) {
            settled = true;
            window.clearTimeout(timeout);
            resolve();
          }
        } catch (error) {
          if (!settled) {
            settled = true;
            window.clearTimeout(timeout);
            reject(
              error instanceof Error
                ? error
                : new Error("Voice presence could not start.")
            );
          }
        }
      } else if (
        status === "CHANNEL_ERROR" ||
        status === "TIMED_OUT" ||
        status === "CLOSED"
      ) {
        roomSubscribed = false;
        if (!settled) {
          settled = true;
          window.clearTimeout(timeout);
          reject(new Error("Voice signaling could not connect."));
        } else {
          scheduleSignalingRecovery(room, sessionId);
        }
      }
    });
  });
}

function scheduleSignalingRecovery(room: VoiceRoom, sessionId: string): void {
  if (signalingRecoveryTimer) return;
  // Keep retrying the signaling channel for as long as the call is still
  // wanted (conditions are re-checked below on every attempt). A network
  // blip that also drops the Realtime socket must not leave the call stuck
  // in "reconnecting" forever with no further attempts and no way out short
  // of leaving and rejoining.
  const delay = Math.min(8_000, 1_000 * 2 ** Math.min(signalingRecoveryAttempts, 10));
  signalingRecoveryTimer = setTimeout(() => {
    signalingRecoveryTimer = null;
    const state = useVoice.getState();
    if (roomSubscribed || state.activeConversationId !== room.conversation_id || state.sessionId !== sessionId || state.rooms[room.conversation_id]?.generation !== room.generation || disconnecting) return;
    signalingRecoveryAttempts += 1;
    useVoice.setState({ status: "reconnecting", error: null });
    void connectRoomChannel(room, sessionId).then(() => {
      signalingRecoveryAttempts = 0;
      if (peerConnection?.connectionState !== "connected") void attemptIceRestart();
    }).catch(() => scheduleSignalingRecovery(room, sessionId));
  }, delay);
}

function clearSignalingRecovery(): void {
  if (signalingRecoveryTimer) clearTimeout(signalingRecoveryTimer);
  signalingRecoveryTimer = null;
  signalingRecoveryAttempts = 0;
}

function syncRoomPresence(): void {
  if (!roomChannel || !currentUserId) return;
  const entries = Object.values(roomChannel.presenceState()).flat() as unknown[];
  const remote = entries
    .map((entry) => entry as Partial<VoicePresence>)
    .find(
      (entry) =>
        entry.userId &&
        entry.sessionId &&
        entry.userId !== currentUserId
    );

  if (!remote?.userId || !remote.sessionId) {
    if (peerConnection) closePeerConnection();
    if (useVoice.getState().activeConversationId) {
      useVoice.setState({ status: "solo", error: null });
    }
    return;
  }

  useVoice.setState((state) => ({
    remoteMuted: { ...state.remoteMuted, [remote.userId!]: Boolean(remote.muted) },
    remoteDeafened: { ...state.remoteDeafened, [remote.userId!]: Boolean(remote.deafened) },
  }));

  const changedSession = remoteSessionId !== remote.sessionId;
  if (changedSession && peerConnection) closePeerConnection(false);
  remoteSessionId = remote.sessionId;
  ensurePeerConnection(remote.userId);
}

function ensurePeerConnection(remoteUserId: string): void {
  if (peerConnection) return;
  const myId = currentUserId;
  if (!myId || !microphone) return;

  polite = myId.localeCompare(remoteUserId) > 0;
  makingOffer = false;
  ignoreOffer = false;
  isSettingRemoteAnswerPending = false;
  restartAttempted = false;
  pendingCandidates = [];
  clearVoiceHealthMonitor();

  const connection = new RTCPeerConnection(getVoiceConnectionConfiguration());
  peerConnection = connection;

  const audioTrack = microphone.outputStream.getAudioTracks()[0];
  if (audioTrack) {
    const micSender = connection.addTrack(audioTrack, microphone.outputStream);
    // Explicit high priority for the actual voice call, belt-and-suspenders
    // alongside screen share never sharing this connection at all (see
    // startScreenShare) - this connection should only ever carry voice.
    const micParameters = micSender.getParameters();
    if (micParameters.encodings.length > 0) {
      micParameters.encodings[0].priority = "high";
      micParameters.encodings[0].networkPriority = "high";
      void micSender.setParameters(micParameters).catch(() => undefined);
    }
  }
  // Screen share (if active) is carried entirely by its own Cloudflare
  // connection now - see startScreenShare - so nothing to re-add here on
  // reconnect; this connection is voice-only.

  connection.ondatachannel = (event) => {
    if (event.channel.label === "dislight-soundboard") configureSoundboardDataChannel(event.channel);
  };
  // Exactly one side creates the ordered reliable channel; the peer receives it
  // through ondatachannel, avoiding a second negotiation and duplicate commands.
  if (!polite) {
    configureSoundboardDataChannel(connection.createDataChannel("dislight-soundboard", { ordered: true }));
  }
  connection.onicecandidate = (event) => {
    if (event.candidate) sendCandidate(event.candidate.toJSON());
  };

  connection.onnegotiationneeded = async () => {
    try {
      makingOffer = true;
      await connection.setLocalDescription();
      if (connection.localDescription) {
        sendDescription(connection.localDescription.toJSON());
      }
    } catch (error) {
      console.error("Voice negotiation failed", error);
    } finally {
      makingOffer = false;
    }
  };

  connection.ontrack = (event) => {
    handleRemoteTrack(event);
  };

  connection.oniceconnectionstatechange = () => {
    if (peerConnection !== connection) return;
    if (
      connection.iceConnectionState === "connected" ||
      connection.iceConnectionState === "completed"
    ) {
      clearDisconnectTimer();
      useVoice.setState({ status: "connected", error: null });
      startVoiceHealthMonitor(connection);
    } else if (connection.iceConnectionState === "disconnected") {
      clearVoiceHealthMonitor();
      useVoice.setState({ status: "reconnecting" });
      scheduleConnectionFailure();
    } else if (connection.iceConnectionState === "failed") {
      clearVoiceHealthMonitor();
      void attemptIceRestart();
    }
  };

  connection.onconnectionstatechange = () => {
    if (peerConnection !== connection) return;
    if (connection.connectionState === "connected") {
      clearDisconnectTimer();
      restartAttempted = false;
      peerRebuildAttempts = 0;
      useVoice.setState({ status: "connected", error: null });
      startVoiceHealthMonitor(connection);
    } else if (connection.connectionState === "disconnected") {
      clearVoiceHealthMonitor();
      useVoice.setState({ status: "reconnecting" });
      scheduleConnectionFailure();
    } else if (connection.connectionState === "failed") {
      clearVoiceHealthMonitor();
      attemptIceRestart();
    } else if (
      connection.connectionState === "connecting" ||
      connection.connectionState === "new"
    ) {
      clearVoiceHealthMonitor();
      useVoice.setState({ status: "connecting" });
    }
  };

  useVoice.setState({ status: "connecting", error: null });
  // A direct ICE path can remain in "new"/"connecting" indefinitely when a
  // VPN or restrictive network blocks UDP. Do not leave the call UI stuck:
  // try one ICE restart, then surface the actionable TURN-relay explanation.
  scheduleConnectionFailure();
  sendReady();
}

async function handleSignal(raw: unknown): Promise<void> {
  const signal = raw as VoiceSignal;
  const state = useVoice.getState();
  const conversationId = state.activeConversationId;
  const sessionId = state.sessionId;
  const room = conversationId ? state.rooms[conversationId] : undefined;

  if (
    !conversationId ||
    !room ||
    !sessionId ||
    signal?.version !== 1 ||
    signal.generation !== room.generation ||
    signal.fromSessionId === sessionId ||
    (signal.toSessionId && signal.toSessionId !== sessionId)
  ) {
    return;
  }

  if (signal.type === "screen-published") {
    try {
      cloudflareScreenConnection?.close();
      cloudflareScreenConnection = await createCloudflareScreenSubscriber(conversationId, signal.cloudflareSessionId, signal.trackNames, (stream) => useVoice.setState({ remoteScreenStream: stream }));
    } catch {
      // The matching P2P screen track remains available as a compatibility fallback.
    }
    return;
  }
  if (signal.type === "screen-stopped") {
    useVoice.setState({ remoteScreenStream: null });
    cloudflareScreenConnection?.close(); cloudflareScreenConnection = null;
    return;
  }
  remoteSessionId = signal.fromSessionId;
  const remoteUserId = voicePartnerId(conversationId);
  if (!remoteUserId) return;
  ensurePeerConnection(remoteUserId);
  const connection = peerConnection;
  if (!connection) return;

  if (signal.type === "ready") return;

  if (signal.type === "description") {
    const readyForOffer =
      !makingOffer &&
      (connection.signalingState === "stable" ||
        isSettingRemoteAnswerPending);
    const offerCollision =
      signal.description.type === "offer" && !readyForOffer;

    ignoreOffer = !polite && offerCollision;
    if (ignoreOffer) return;

    isSettingRemoteAnswerPending =
      signal.description.type === "answer";
    try {
      await connection.setRemoteDescription(signal.description);
      isSettingRemoteAnswerPending = false;
      await flushPendingCandidates(connection);

      if (signal.description.type === "offer") {
        await connection.setLocalDescription();
        if (connection.localDescription) {
          sendDescription(connection.localDescription.toJSON());
        }
      }
    } finally {
      isSettingRemoteAnswerPending = false;
    }
    return;
  }

  if (signal.type === "ice-candidate") {
    if (!connection.remoteDescription) {
      pendingCandidates.push(signal.candidate);
      return;
    }
    try {
      await connection.addIceCandidate(signal.candidate);
    } catch (error) {
      if (!ignoreOffer) throw error;
    }
  }
}

async function flushPendingCandidates(
  connection: RTCPeerConnection
): Promise<void> {
  const candidates = pendingCandidates;
  pendingCandidates = [];
  for (const candidate of candidates) {
    await connection.addIceCandidate(candidate);
  }
}

// Only ever fires for the mic's audio track now - screen share is carried
// entirely by its own Cloudflare connection (see startScreenShare), never
// this one, so there's no video branch here anymore.
function handleRemoteTrack(event: RTCTrackEvent): void {
  if (event.track.kind === "audio") {
    remoteAudio ??= createRemoteAudioElement();
    remoteAudioStream ??= new MediaStream();
    if (!remoteAudioStream.getAudioTracks().some((track) => track.id === event.track.id)) {
      remoteAudioStream.addTrack(event.track);
    }
    void configureRemoteAudio(remoteAudio, {
      stream: remoteAudioStream,
      outputVolume: usePreferences.getState().outputVolume,
      outputDeviceId: usePreferences.getState().outputDeviceId,
      deafened: useVoice.getState().deafened,
      partnerVoiceBoost: usePreferences.getState().partnerVoiceBoost,
    });
    if (!remoteVoiceActivity) {
      const conversationId = useVoice.getState().activeConversationId;
      const remoteUserId = conversationId ? voicePartnerId(conversationId) : null;
      if (remoteUserId) {
        remoteVoiceActivity = monitorVoiceActivity(
          remoteAudioStream,
          (value) => setSpeaking(remoteUserId, value),
          (value) => setLevel(remoteUserId, value)
        );
      }
    }
    event.track.onended = () => {
      remoteAudioStream?.removeTrack(event.track);
      if (!remoteAudioStream?.getAudioTracks().length && remoteAudio) remoteAudio.srcObject = null;
    };
  }
}

function sendScreenPublished(cloudflareSessionId: string, trackNames: string[]): void {
  const signal = buildSignal({ type: "screen-published", cloudflareSessionId, trackNames });
  if (signal) sendSignal(signal);
}

function sendScreenStopped(): void {
  const signal = buildSignal({ type: "screen-stopped" });
  if (signal) sendSignal(signal);
}

function sendReady(): void {
  const signal = buildSignal({ type: "ready" });
  if (signal) sendSignal(signal);
}

function sendDescription(description: RTCSessionDescriptionInit): void {
  const signal = buildSignal({ type: "description", description });
  if (signal) sendSignal(signal);
}

function sendCandidate(candidate: RTCIceCandidateInit): void {
  const signal = buildSignal({ type: "ice-candidate", candidate });
  if (signal) sendSignal(signal);
}

function buildSignal(
  payload:
    | { type: "ready" }
    | { type: "description"; description: RTCSessionDescriptionInit }
    | { type: "ice-candidate"; candidate: RTCIceCandidateInit }
    | { type: "screen-published"; cloudflareSessionId: string; trackNames: string[] }
    | { type: "screen-stopped" }
): VoiceSignal | null {
  const state = useVoice.getState();
  const conversationId = state.activeConversationId;
  const room = conversationId ? state.rooms[conversationId] : undefined;
  if (!room || !state.sessionId) return null;

  const base = {
    version: 1 as const,
    generation: room.generation,
    fromSessionId: state.sessionId,
    signalId: crypto.randomUUID(),
    ...(remoteSessionId ? { toSessionId: remoteSessionId } : {}),
  };

  if (payload.type === "ready") return { ...base, type: "ready" };
  if (payload.type === "description") {
    return { ...base, type: "description", description: payload.description };
  }
  if (payload.type === "ice-candidate") return { ...base, type: "ice-candidate", candidate: payload.candidate };
  if (payload.type === "screen-published") return { ...base, ...payload };
  return { ...base, type: "screen-stopped" };
}

function isCurrentVoiceSignal(signal: VoiceSignal): boolean {
  const state = useVoice.getState();
  const conversationId = state.activeConversationId;
  const room = conversationId ? state.rooms[conversationId] : undefined;
  return Boolean(
    !disconnecting &&
    room &&
    state.sessionId === signal.fromSessionId &&
    room.generation === signal.generation &&
    (!signal.toSessionId || !remoteSessionId || signal.toSessionId === remoteSessionId)
  );
}

function queueVoiceSignal(signal: VoiceSignal): void {
  if (!isCurrentVoiceSignal(signal)) return;
  if (signal.signalId && pendingVoiceSignals.some((entry) => entry.signal.signalId === signal.signalId)) return;
  while (pendingVoiceSignals.length >= SIGNAL_OUTBOX_MAX) {
    const candidateIndex = pendingVoiceSignals.findIndex((entry) => entry.signal.type === "ice-candidate");
    pendingVoiceSignals.splice(candidateIndex >= 0 ? candidateIndex : 0, 1);
  }
  pendingVoiceSignals.push({ signal, expiresAt: Date.now() + SIGNAL_OUTBOX_TTL_MS });
}

function scheduleVoiceSignalFlush(delayMs = 1_000): void {
  if (signalFlushRetryTimer || disconnecting) return;
  signalFlushRetryTimer = window.setTimeout(() => {
    signalFlushRetryTimer = null;
    void flushPendingVoiceSignals();
  }, delayMs);
}

async function deliverVoiceSignal(
  channel: RealtimeChannel,
  signal: VoiceSignal
): Promise<boolean> {
  for (let attempt = 0; attempt < SIGNAL_SEND_ATTEMPTS; attempt += 1) {
    if (channel !== roomChannel || !roomSubscribed || !isCurrentVoiceSignal(signal)) return false;
    try {
      const result = await channel.send({
        type: "broadcast",
        event: "signal",
        payload: signal,
      }, { timeout: SIGNAL_SEND_TIMEOUT_MS });
      if (result === "ok") return channel === roomChannel && isCurrentVoiceSignal(signal);
      console.warn("Voice signal was not accepted by Realtime.", { type: signal.type, result });
    } catch (error) {
      console.warn("Voice signal send failed.", { type: signal.type, error });
    }
    if (attempt + 1 < SIGNAL_SEND_ATTEMPTS) {
      await new Promise<void>((resolve) => window.setTimeout(resolve, 250 * 2 ** attempt));
    }
  }
  return false;
}

async function flushPendingVoiceSignals(): Promise<void> {
  if (flushingVoiceSignals || !roomChannel || !roomSubscribed || disconnecting) return;
  flushingVoiceSignals = true;
  const runId = ++signalFlushRunId;
  if (signalFlushRetryTimer) {
    window.clearTimeout(signalFlushRetryTimer);
    signalFlushRetryTimer = null;
  }
  try {
    while (pendingVoiceSignals.length && roomChannel && roomSubscribed && !disconnecting) {
      const entry = pendingVoiceSignals[0];
      if (entry.expiresAt <= Date.now() || !isCurrentVoiceSignal(entry.signal)) {
        pendingVoiceSignals.shift();
        continue;
      }
      const channel = roomChannel;
      if (!(await deliverVoiceSignal(channel, entry.signal))) {
        return;
      }
      if (pendingVoiceSignals[0] === entry) pendingVoiceSignals.shift();
    }
  } finally {
    if (runId === signalFlushRunId) {
      flushingVoiceSignals = false;
      if (pendingVoiceSignals.length && roomChannel && roomSubscribed && !disconnecting) {
        scheduleVoiceSignalFlush(1_500);
      }
    }
  }
}

function sendSignal(signal: VoiceSignal): void {
  if (!isCurrentVoiceSignal(signal)) return;
  if (!roomChannel || !roomSubscribed || pendingVoiceSignals.length) {
    queueVoiceSignal(signal);
    void flushPendingVoiceSignals();
    return;
  }

  // Keep the normal connected path low-latency: SDP/ICE sends can proceed
  // independently. When the channel is unavailable or a send fails, the
  // bounded outbox replays them in insertion order after recovery.
  const channel = roomChannel;
  void deliverVoiceSignal(channel, signal).then((delivered) => {
    if (delivered || !isCurrentVoiceSignal(signal)) return;
    queueVoiceSignal(signal);
    void flushPendingVoiceSignals();
  });
}

async function receiveVoiceSignal(raw: unknown): Promise<void> {
  const signal = raw as VoiceSignal;
  const key = signal?.signalId && signal?.fromSessionId
    ? `${signal.fromSessionId}:${signal.signalId}`
    : null;
  const now = Date.now();
  for (const [receivedKey, timestamp] of receivedVoiceSignalIds) {
    if (now - timestamp > 60_000) receivedVoiceSignalIds.delete(receivedKey);
  }
  if (key && receivedVoiceSignalIds.has(key)) return;
  if (key) {
    receivedVoiceSignalIds.set(key, now);
    while (receivedVoiceSignalIds.size > 256) {
      const oldest = receivedVoiceSignalIds.keys().next().value;
      if (!oldest) break;
      receivedVoiceSignalIds.delete(oldest);
    }
  }

  try {
    await handleSignal(signal);
  } catch (error) {
    if (key) receivedVoiceSignalIds.delete(key);
    console.warn("Voice signal could not be processed; scheduling ICE recovery.", {
      type: signal?.type,
      error,
    });
    scheduleConnectionFailure();
  }
}

function voicePartnerId(conversationId: string): string | null {
  const participant = (
    useVoice.getState().participants[conversationId] ?? []
  ).find((entry) => entry.user_id !== currentUserId);
  if (participant) return participant.user_id;

  const conversation = useChat
    .getState()
    .conversations.find((entry) => entry.id === conversationId);
  if (!conversation || !currentUserId) return null;
  return conversation.user1_id === currentUserId
    ? conversation.user2_id
    : conversation.user1_id;
}

function startHeartbeat(): void {
  if (heartbeatTimer) clearInterval(heartbeatTimer);
  heartbeatTimer = setInterval(() => {
    void sendHeartbeat();
  }, HEARTBEAT_MS);
}

let activeLeaseVerification: Promise<void> | null = null;

async function verifyActiveLease(conversationId: string): Promise<void> {
  if (activeLeaseVerification) return activeLeaseVerification;

  const expectedSessionId = useVoice.getState().sessionId;
  if (!expectedSessionId) return;

  activeLeaseVerification = (async () => {
    // Give a replacement INSERT a moment to become observable after its DELETE.
    await new Promise<void>((resolve) => window.setTimeout(resolve, 250));
    const state = useVoice.getState();
    if (
      state.activeConversationId !== conversationId ||
      state.sessionId !== expectedSessionId ||
      disconnecting
    ) {
      return;
    }

    const { data, error } = await supabase.rpc("heartbeat_voice_room", {
      p_session_id: expectedSessionId,
      p_sharing_screen: state.sharingScreen,
    });
    if (error || (data as RpcStatus).status !== "not_found") return;

    const latest = useVoice.getState();
    if (
      latest.activeConversationId === conversationId &&
      latest.sessionId === expectedSessionId &&
      !disconnecting
    ) {
      await disconnectLocal(false);
      useAlerts.getState().show({ severity: "danger", message: "Your voice session expired." });
    }
  })().finally(() => {
    activeLeaseVerification = null;
  });

  return activeLeaseVerification;
}
async function sendHeartbeat(): Promise<void> {
  const state = useVoice.getState();
  if (!state.sessionId || !state.activeConversationId) return;

  const { data, error } = await supabase.rpc("heartbeat_voice_room", {
    p_session_id: state.sessionId,
    p_sharing_screen: state.sharingScreen,
  });
  if (error) {
    console.warn("Voice heartbeat failed", error);
    return;
  }
  const response = data as RpcStatus;
  if (response.status === "not_found") {
    await disconnectLocal(false);
    useAlerts.getState().show({ severity: "danger", message: "Your voice session expired." });
  }
}

async function updateRoomPresence(): Promise<void> {
  const state = useVoice.getState();
  const conversationId = state.activeConversationId;
  if (
    !roomChannel ||
    !roomSubscribed ||
    !currentUserId ||
    !state.sessionId ||
    !conversationId
  ) {
    return;
  }

  const ownParticipant = (
    state.participants[conversationId] ?? []
  ).find((participant) => participant.user_id === currentUserId);

  await roomChannel.track({
    userId: currentUserId,
    sessionId: state.sessionId,
    muted: state.muted,
    deafened: state.deafened,
    sharingScreen: state.sharingScreen,
    joinedAt: ownParticipant?.joined_at ?? new Date().toISOString(),
  } satisfies VoicePresence);
}

function applyLocalMuteState(): void {
  const state = useVoice.getState();
  const enabled = !state.muted && !state.deafened;
  for (const track of microphone?.outputStream.getAudioTracks() ?? []) {
    track.enabled = enabled;
  }
}

function applyRemoteAudioPreferences(): void {
  if (!remoteAudio) return;
  const preferences = usePreferences.getState();
  void configureRemoteAudio(remoteAudio, {
    outputVolume: preferences.outputVolume,
    outputDeviceId: preferences.outputDeviceId,
    deafened: useVoice.getState().deafened,
    partnerVoiceBoost: preferences.partnerVoiceBoost,
  });
}

async function stopLocalScreen(updateServer: boolean): Promise<void> {
  const track = localScreenTrack;
  if (!track) return;

  screenAudioCleanup?.();
  screenAudioCleanup = null;

  track.onended = null;
  for (const mediaTrack of localScreenStream?.getTracks() ?? []) {
    mediaTrack.stop();
  }
  localScreenStream = null;
  localScreenTrack = null;
  cloudflareScreenConnection?.close();
  cloudflareScreenConnection = null;
  sendScreenStopped();
  useVoice.setState({ sharingScreen: false, localScreenStream: null });

  if (updateServer) {
    await updateRoomPresence();
    await sendHeartbeat();
  }
}

function scheduleConnectionFailure(): void {
  clearDisconnectTimer();
  disconnectTimer = setTimeout(() => {
    const recovering = useVoice.getState().status === "reconnecting";
    if (peerConnection && (peerConnection.connectionState !== "connected" || recovering)) {
      void attemptIceRestart();
    }
  }, CONNECTION_FAILURE_DELAY_MS);
}

async function attemptRelayRecovery(reason: "audio quality" | "connection failure"): Promise<boolean> {
  const connection = peerConnection;
  if (
    !connection ||
    !remoteSessionId ||
    disconnecting ||
    relayRecoveryAttempted ||
    relayRecoveryInProgress ||
    relayRecoveryAttempts >= RELAY_RECOVERY_MAX_ATTEMPTS ||
    Date.now() < relayRecoveryRetryAfter
  ) {
    return false;
  }

  relayRecoveryInProgress = true;
  relayRecoveryAttempts += 1;
  const wasConnected = connection.connectionState === "connected";
  if (!wasConnected) useVoice.setState({ status: "reconnecting", error: null });
  try {
    const relayAvailable = await refreshTurnCredentials(true);
    if (!relayAvailable || peerConnection !== connection) {
      const retryDelay = Math.min(30_000, 2_000 * 2 ** (relayRecoveryAttempts - 1));
      relayRecoveryRetryAfter = Date.now() + retryDelay;
      console.warn(`Voice ${reason} recovery could not obtain Cloudflare TURN credentials.`);
      // The current direct pair may still be carrying audio. Keep it usable
      // and leave the monitor armed so a temporary broker outage is retryable.
      if (wasConnected && peerConnection === connection && connection.connectionState === "connected") {
        useVoice.setState({ status: "connected", error: null });
      }
      return false;
    }

    forceRelayTransport = true;
    // setConfiguration + restartIce is the supported way to replace ICE
    // routing mid-call. With "relay" selected, Chromium only considers TURN
    // candidates instead of returning to the flaky direct candidate pair.
    connection.setConfiguration({
      ...connection.getConfiguration(),
      iceServers: activeIceServers,
      iceTransportPolicy: "relay",
    });
    connection.restartIce();
    relayRecoveryAttempted = true;
    restartAttempted = true;
    directFallbackAttempted = false;
    relayRecoveryRetryAfter = 0;
    clearDisconnectTimer();
    disconnectTimer = setTimeout(() => void attemptIceRestart(), 12_000);
    return true;
  } catch (error) {
    // A fresh RTCPeerConnection is the conservative fallback for a WebView
    // that declines a live transport-policy change. It retains the current
    // room/signaling session and is constructed relay-only.
    console.warn("Live TURN route switch failed; rebuilding the voice peer.", error);
    if (forceRelayTransport && peerConnection === connection) {
      relayRecoveryAttempted = true;
      restartAttempted = true;
      await rebuildPeerConnection();
      return peerConnection !== null;
    }

    forceRelayTransport = false;
    relayRecoveryRetryAfter = Date.now() + Math.min(30_000, 2_000 * 2 ** (relayRecoveryAttempts - 1));
    if (wasConnected && peerConnection === connection && connection.connectionState === "connected") {
      useVoice.setState({ status: "connected", error: null });
    }
    return false;
  } finally {
    relayRecoveryInProgress = false;
  }
}

async function attemptIceRestart(): Promise<void> {
  const isRecovering = useVoice.getState().status === "reconnecting";
  const connection = peerConnection;
  if (
    connectionRecoveryInProgress ||
    relayRecoveryInProgress ||
    !connection ||
    !remoteSessionId ||
    (connection.connectionState === "connected" && !isRecovering) ||
    disconnecting
  ) return;
  connectionRecoveryInProgress = true;
  try {
    if (!restartAttempted) {
      restartAttempted = true;
      useVoice.setState({ status: "reconnecting", error: null });
      const wasForcingRelay = forceRelayTransport;
      // A direct ICE restart should not wait on the TURN broker. Refresh in
      // the background so it is ready for a later relay fallback; only wait
      // when relay-only is already the selected recovery policy.
      const relayAvailable = wasForcingRelay
        ? await refreshTurnCredentials()
        : hasTurnRelay();
      if (!wasForcingRelay) void refreshTurnCredentials();
      if (peerConnection !== connection) return;
      if (wasForcingRelay && !relayAvailable) {
        // Do not leave a relay-only peer with no valid TURN server configured.
        // A direct attempt is preferable to a guaranteed dead route while the
        // credential broker is unavailable.
        forceRelayTransport = false;
        relayRecoveryAttempted = false;
        directFallbackAttempted = true;
      }
      connection.setConfiguration({
        ...connection.getConfiguration(),
        iceServers: activeIceServers,
        iceTransportPolicy: forceRelayTransport ? "relay" : "all",
      });
      connection.restartIce();
      clearDisconnectTimer();
      disconnectTimer = setTimeout(() => void attemptIceRestart(), 12_000);
      return;
    }
    if (!relayRecoveryAttempted) {
      if (Date.now() < relayRecoveryRetryAfter) {
        clearDisconnectTimer();
        disconnectTimer = setTimeout(
          () => void attemptIceRestart(),
          Math.min(12_000, relayRecoveryRetryAfter - Date.now())
        );
        return;
      }
      if (await attemptRelayRecovery("connection failure")) {
        clearDisconnectTimer();
        disconnectTimer = setTimeout(() => void attemptIceRestart(), 12_000);
        return;
      }
      if (peerConnection !== connection) return;
      if (!relayRecoveryAttempted && relayRecoveryAttempts < RELAY_RECOVERY_MAX_ATTEMPTS) {
        clearDisconnectTimer();
        disconnectTimer = setTimeout(
          () => void attemptIceRestart(),
          Math.max(1_500, Math.min(12_000, relayRecoveryRetryAfter - Date.now()))
        );
        return;
      }
    }
    if (forceRelayTransport && relayRecoveryAttempted && !directFallbackAttempted) {
      // TURN can itself be a bad route (or fail behind a restrictive firewall).
      // After the relay ICE restart has had its window, try standard ICE again
      // instead of rebuilding relay-only peers until the call gives up.
      directFallbackAttempted = true;
      forceRelayTransport = false;
      relayRecoveryAttempted = false;
      connection.setConfiguration({
        ...connection.getConfiguration(),
        iceServers: activeIceServers,
        iceTransportPolicy: "all",
      });
      connection.restartIce();
      clearDisconnectTimer();
      disconnectTimer = setTimeout(() => void attemptIceRestart(), 12_000);
      return;
    }
    if (peerRebuildAttempts < 2) {
      peerRebuildAttempts += 1;
      await rebuildPeerConnection();
      return;
    }
    markConnectionFailed();
  } finally {
    connectionRecoveryInProgress = false;
  }
}

async function rebuildPeerConnection(): Promise<void> {
  const state = useVoice.getState();
  const conversationId = state.activeConversationId;
  const preservedRemoteSessionId = remoteSessionId;
  const remoteUserId = conversationId ? voicePartnerId(conversationId) : null;
  if (!conversationId || !preservedRemoteSessionId || !remoteUserId) {
    markConnectionFailed();
    return;
  }
  useVoice.setState({ status: "reconnecting", error: null });
  const wasForcingRelay = forceRelayTransport;
  const relayAvailable = wasForcingRelay
    ? await refreshTurnCredentials()
    : hasTurnRelay();
  if (!wasForcingRelay) void refreshTurnCredentials();
  if (wasForcingRelay && !relayAvailable) {
    forceRelayTransport = false;
    relayRecoveryAttempted = false;
  }
  closePeerConnection(false);
  remoteSessionId = preservedRemoteSessionId;
  ensurePeerConnection(remoteUserId);
  await updateRoomPresence();
}

function markConnectionFailed(): void {
  clearDisconnectTimer();
  const message =
    "Direct connection failed. This network may require a TURN relay.";
  useVoice.setState({ status: "failed", error: message });
  useAlerts.getState().show({
    severity: "danger",
    message,
    actions: [
      { label: "Dismiss" },
      { label: "Reconnect", confirm: true, onClick: () => void useVoice.getState().forceReconnect() },
    ],
  });
}

function clearDisconnectTimer(): void {
  if (disconnectTimer) clearTimeout(disconnectTimer);
  disconnectTimer = null;
}

function closePeerConnection(setSolo = true): void {
  clearDisconnectTimer();
  clearVoiceHealthMonitor();
  const connection = peerConnection;
  peerConnection = null;
  if (connection) {
    connection.onicecandidate = null;
    connection.onnegotiationneeded = null;
    connection.ontrack = null;
    connection.oniceconnectionstatechange = null;
    connection.onconnectionstatechange = null;
    connection.ondatachannel = null;
    connection.close();
  }

  soundboardDataChannel?.close();
  soundboardDataChannel = null;

  if (remoteAudio) {
    remoteAudio.pause();
    remoteAudio.srcObject = null;
  }
  remoteAudioStream = null;
  remoteVoiceActivity?.stop();
  remoteVoiceActivity = null;
  remoteSessionId = null;
  pendingCandidates = [];
  makingOffer = false;
  ignoreOffer = false;
  isSettingRemoteAnswerPending = false;
  restartAttempted = false;
  useVoice.setState({
    remoteScreenStream: null,
    ...(setSolo && useVoice.getState().activeConversationId
      ? { status: "solo" as const, error: null }
      : {}),
  });
}

async function disconnectLocal(notifyServer: boolean): Promise<void> {
  if (disconnecting) return;
  disconnecting = true;

  const state = useVoice.getState();
  const sessionId = state.sessionId;
  const conversationId = state.activeConversationId;
  const userId = currentUserId;

  if (heartbeatTimer) clearInterval(heartbeatTimer);
  heartbeatTimer = null;
  clearSignalingRecovery();
  if (signalFlushRetryTimer) window.clearTimeout(signalFlushRetryTimer);
  signalFlushRetryTimer = null;
  pendingVoiceSignals = [];
  receivedVoiceSignalIds.clear();
  signalFlushRunId += 1;
  flushingVoiceSignals = false;
  peerRebuildAttempts = 0;
  relayRecoveryAttempted = false;
  relayRecoveryInProgress = false;
  relayRecoveryAttempts = 0;
  relayRecoveryRetryAfter = 0;
  directFallbackAttempted = false;
  forceRelayTransport = false;
  await stopLocalScreen(false);
  closePeerConnection(false);

  const channel = roomChannel;
  roomChannel = null;
  roomSubscribed = false;
  if (channel) {
    // Realtime teardown is best-effort. It must not hold the leave button or
    // local media hostage when a network is slow or already disconnected.
    void channel.untrack().catch(() => undefined);
    void supabase.removeChannel(channel).catch(() => undefined);
  }

  const microphoneToStop = microphone;
  microphone = null;
  localVoiceActivity?.stop();
  localVoiceActivity = null;

  useVoice.setState((current) => {
    const participants = { ...current.participants };
    const remaining = conversationId
      ? (participants[conversationId] ?? []).filter(
          (participant) => participant.user_id !== userId
        )
      : [];
    if (conversationId) participants[conversationId] = remaining;

    const rooms = { ...current.rooms };
    if (conversationId && remaining.length === 0) delete rooms[conversationId];

    return {
      rooms,
      participants,
      status: "idle",
      activeConversationId: null,
      sessionId: null,
      sharingScreen: false,
      localScreenStream: null,
      remoteScreenStream: null,
      error: null,
    };
  });

  disconnecting = false;

  // Track stopping is synchronous before its first await, so audio stops right
  // away while AudioContext cleanup continues in the background.
  void stopMicrophonePipeline(microphoneToStop);
  if (notifyServer && sessionId) {
    void supabase
      .rpc("leave_voice_room", { p_session_id: sessionId })
      .then(
        ({ error }) => {
          if (error) console.warn("Voice leave failed", error);
        },
        (error: unknown) => console.warn("Voice leave request failed", error)
      );
  }
}


type VoiceDataMessage =
  | { type: "soundboard"; payload: VoiceSoundboardPayload }
  | { type: "soundboard-stop"; payload: VoiceSoundboardStopPayload }
  | { type: "soundboard-prepare"; payload: VoiceSoundboardPayload }
  | { type: "soundboard-ready"; payload: VoiceSoundboardReadyPayload };

function configureSoundboardDataChannel(channel: RTCDataChannel): void {
  soundboardDataChannel?.close();
  soundboardDataChannel = channel;
  channel.onmessage = (event) => {
    if (typeof event.data !== "string") return;
    try {
      const message = JSON.parse(event.data) as Partial<VoiceDataMessage>;
      if (message.type === "soundboard") handleRemoteSoundboardPlay(message.payload);
      if (message.type === "soundboard-stop") handleRemoteSoundboardStop(message.payload);
      if (message.type === "soundboard-prepare") handleRemoteSoundboardPrepare(message.payload);
      if (message.type === "soundboard-ready") handleSoundboardReady(message.payload);
    } catch {
      // Ignore malformed peer data; it is never allowed to affect call state.
    }
  };
  channel.onclose = () => {
    if (soundboardDataChannel === channel) soundboardDataChannel = null;
  };
}

function sendSoundboardData(message: VoiceDataMessage): boolean {
  if (soundboardDataChannel?.readyState !== "open") return false;
  try {
    soundboardDataChannel.send(JSON.stringify(message));
    return true;
  } catch {
    return false;
  }
}

function handleRemoteSoundboardPlay(raw: unknown): void {
  const payload = raw as Partial<VoiceSoundboardPayload>;
  if (!isValidSoundboardPayload(payload) || useVoice.getState().deafened || Date.now() - lastRemoteSoundboardAt < 750) return;

  lastRemoteSoundboardAt = Date.now();
  const playbackKey = soundboardPlaybackKey(payload.id, payload.nonce);
  cancelledRemoteSoundboardPlaybacks.delete(playbackKey);
  void playSoundboardUrl(
    payload.id,
    payload.signedUrl,
    payload.playAt,
    usePreferences.getState().soundboardVolume,
    usePreferences.getState().outputDeviceId,
    { onEnded: () => remoteSoundboardPlaybacks.delete(playbackKey) }
  ).then((playback) => {
    if (cancelledRemoteSoundboardPlaybacks.delete(playbackKey)) {
      playback.stop();
      return;
    }
    remoteSoundboardPlaybacks.set(playbackKey, playback);
  }).catch((error) => console.warn("Soundboard playback failed", error));
}

function isValidSoundboardPayload(payload: Partial<VoiceSoundboardPayload>): payload is VoiceSoundboardPayload {
  return payload.version === 1
    && typeof payload.id === "string"
    && typeof payload.name === "string"
    && typeof payload.nonce === "string"
    && typeof payload.signedUrl === "string"
    && typeof payload.playAt === "number"
    && isTrustedSoundboardUrl(payload.signedUrl);
}

function handleRemoteSoundboardPrepare(raw: unknown): void {
  const payload = raw as Partial<VoiceSoundboardPayload>;
  if (!isValidSoundboardPayload(payload)) return;
  void preloadSoundboardClip(payload.id, payload.signedUrl)
    .then(() => sendSoundboardData({
      type: "soundboard-ready",
      payload: { version: 1, id: payload.id, nonce: payload.nonce },
    }))
    .catch(() => undefined);
}

function handleSoundboardReady(raw: unknown): void {
  const payload = raw as Partial<VoiceSoundboardReadyPayload>;
  if (payload.version !== 1 || typeof payload.id !== "string" || typeof payload.nonce !== "string") return;
  const key = soundboardPlaybackKey(payload.id, payload.nonce);
  const pending = pendingSoundboardReadiness.get(key);
  if (!pending) return;
  window.clearTimeout(pending.timeout);
  pendingSoundboardReadiness.delete(key);
  pending.resolve(true);
}
function handleRemoteSoundboardStop(raw: unknown): void {
  const payload = raw as Partial<VoiceSoundboardStopPayload>;
  if (payload.version !== 1 || typeof payload.id !== "string" || typeof payload.nonce !== "string") return;
  stopRemoteSoundboardPlayback(soundboardPlaybackKey(payload.id, payload.nonce));
}
function isTrustedSoundboardUrl(value: string): boolean {
  try {
    const url = new URL(value);
    const project = new URL(import.meta.env.VITE_SUPABASE_URL);
    return (
      url.origin === project.origin &&
      url.pathname.includes("/storage/v1/object/sign/soundboard/")
    );
  } catch {
    return false;
  }
}
export interface VoiceSoundboardPayload {
  version: 1;
  id: string;
  name: string;
  signedUrl: string;
  playAt: number;
  nonce: string;
}

export interface VoiceSoundboardStopPayload {
  version: 1;
  id: string;
  nonce: string;
}

export interface VoiceSoundboardReadyPayload {
  version: 1;
  id: string;
  nonce: string;
}
function soundboardPlaybackKey(id: string, nonce: string): string {
  return `${id}:${nonce}`;
}

function stopRemoteSoundboardPlayback(playbackKey: string): void {
  cancelledRemoteSoundboardPlaybacks.add(playbackKey);
  remoteSoundboardPlaybacks.get(playbackKey)?.stop();
  remoteSoundboardPlaybacks.delete(playbackKey);
}

export function broadcastVoiceSoundboard(payload: VoiceSoundboardPayload): void {
  if (sendSoundboardData({ type: "soundboard", payload })) return;
  if (!roomChannel || !roomSubscribed || !useVoice.getState().activeConversationId) return;
  void roomChannel.send({ type: "broadcast", event: "soundboard", payload });
}

export function prepareVoiceSoundboard(payload: VoiceSoundboardPayload): Promise<boolean> {
  const key = soundboardPlaybackKey(payload.id, payload.nonce);
  return new Promise((resolve) => {
    const timeout = window.setTimeout(() => {
      const pending = pendingSoundboardReadiness.get(key);
      if (!pending) return;
      pendingSoundboardReadiness.delete(key);
      resolve(false);
    }, 900);
    pendingSoundboardReadiness.set(key, { resolve, timeout });
    if (sendSoundboardData({ type: "soundboard-prepare", payload })) return;
    window.clearTimeout(timeout);
    pendingSoundboardReadiness.delete(key);
    resolve(false);
  });
}
export function broadcastVoiceSoundboardStop(payload: VoiceSoundboardStopPayload): void {
  if (sendSoundboardData({ type: "soundboard-stop", payload })) return;
  if (!roomChannel || !roomSubscribed || !useVoice.getState().activeConversationId) return;
  void roomChannel.send({ type: "broadcast", event: "soundboard-stop", payload });
}

export interface VoiceCallStats {
  /** Round-trip time of the direct peer connection, in whole milliseconds. */
  rttMs: number | null;
  /** True if audio is relayed through a TURN server rather than a direct
   * path - relayed connections typically carry more latency. */
  relayed: boolean;
  /** Latest inbound RTP jitter estimate, in whole milliseconds. */
  jitterMs: number | null;
  /** Inbound audio packet loss ratio over this peer connection's lifetime. */
  packetLossPercent: number | null;
}

// The call is a direct WebRTC peer connection (occasionally TURN-relayed),
// not routed through a central voice server - so there's only one
// meaningful round-trip number here, not a separate "your ping" vs "their
// ping" to some middlebox.
export async function getVoiceCallStats(): Promise<VoiceCallStats | null> {
  if (!peerConnection) return null;
  try {
    const report = await peerConnection.getStats();
    const route = getSelectedVoiceRoute(report);
    if (!route) return null;
    const inbound = Array.from(report.values()).find((entry) => {
      if (entry.type !== "inbound-rtp") return false;
      const stats = entry as RTCStats & { kind?: unknown; mediaType?: unknown };
      return stats.kind === "audio" || stats.mediaType === "audio";
    }) as (RTCStats & {
      packetsReceived?: unknown;
      packetsLost?: unknown;
      jitter?: unknown;
    }) | undefined;
    const received = typeof inbound?.packetsReceived === "number" ? inbound.packetsReceived : null;
    const lost = typeof inbound?.packetsLost === "number" ? Math.max(0, inbound.packetsLost) : null;
    const total = received !== null && lost !== null ? received + lost : 0;
    return {
      rttMs: typeof route.roundTripTime === "number" ? Math.round(route.roundTripTime * 1000) : null,
      relayed: route.localCandidateType === "relay",
      jitterMs: typeof inbound?.jitter === "number" ? Math.round(inbound.jitter * 1000) : null,
      packetLossPercent: total > 0 && lost !== null ? Number(((lost / total) * 100).toFixed(1)) : null,
    };
  } catch {
    return null;
  }
}
