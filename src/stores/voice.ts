import type { RealtimeChannel } from "@supabase/supabase-js";
import { toast } from "sonner";
import { create } from "zustand";
import {
  captureScreen,
  configureRemoteAudio,
  createMicrophonePipeline,
  createRemoteAudioElement,
  setMicrophoneGain,
  stopMicrophonePipeline,
  type MicrophonePipeline,
} from "@/lib/voice-media";
import { playAppSound } from "@/lib/app-sounds";
import { playSoundboardUrl } from "@/lib/soundboard-audio";
import { createCloudflareScreenPublisher, createCloudflareScreenSubscriber } from "@/lib/cloudflare-realtime";
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
  status: VoiceConnectionStatus;
  activeConversationId: string | null;
  sessionId: string | null;
  muted: boolean;
  deafened: boolean;
  sharingScreen: boolean;
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
}

const ICE_SERVERS: RTCIceServer[] = [
  { urls: ["stun:stun.cloudflare.com:3478"] },
];
const HEARTBEAT_MS = 45_000;
const TURN_CREDENTIAL_TTL_SAFETY_MS = 10 * 60_000;
const CHANNEL_TIMEOUT_MS = 12_000;

let currentUserId: string | null = null;
let discoveryChannel: RealtimeChannel | null = null;
let roomChannel: RealtimeChannel | null = null;
let roomSubscribed = false;
let microphone: MicrophonePipeline | null = null;
let peerConnection: RTCPeerConnection | null = null;
let remoteAudio: HTMLAudioElement | null = null;
let localScreenStream: MediaStream | null = null;
let localScreenTrack: MediaStreamTrack | null = null;
let cloudflareScreenConnection: RTCPeerConnection | null = null;
let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
let disconnectTimer: ReturnType<typeof setTimeout> | null = null;
let preferencesUnsubscribe: (() => void) | null = null;
let beforeUnloadHandler: (() => void) | null = null;
let remoteSessionId: string | null = null;
let polite = false;
let makingOffer = false;
let ignoreOffer = false;
let isSettingRemoteAnswerPending = false;
let restartAttempted = false;
let disconnecting = false;
let pendingCandidates: RTCIceCandidateInit[] = [];
let lastRemoteSoundboardAt = 0;
let activeIceServers: RTCIceServer[] = ICE_SERVERS;
let turnCredentialsExpireAt = 0;

export const useVoice = create<VoiceState>()((set, get) => ({
  rooms: {},
  participants: {},
  status: "idle",
  activeConversationId: null,
  sessionId: null,
  muted: false,
  deafened: false,
  sharingScreen: false,
  remoteScreenStream: null,
  error: null,

  init: (userId) => initializeVoice(userId),

  join: async (conversationId, takeover = false) => {
    if (get().status === "joining") return;
    if (
      get().activeConversationId === conversationId &&
      get().status !== "idle"
    ) {
      return;
    }

    if (get().activeConversationId && get().activeConversationId !== conversationId) {
      if (!takeover) {
        toast.info("You are already in another voice channel.", {
          action: {
            label: "Switch",
            onClick: () => void get().join(conversationId, true),
          },
        });
        return;
      }
      await disconnectLocal(true);
    }

    set({ status: "joining", error: null });
    const sessionId = crypto.randomUUID();

    try {
      microphone = await createPreferredMicrophone();
      applyLocalMuteState();

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
        toast.info("Voice is active on another device.", {
          action: {
            label: "Take over",
            onClick: () => void get().join(conversationId, true),
          },
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

      await refreshTurnCredentials();
      await connectRoomChannel(room, sessionId);
      playAppSound("call_join");
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
      toast.error(message);
    }
  },

  leave: async () => {
    const wasActive = Boolean(get().activeConversationId);
    await disconnectLocal(true);
    if (wasActive) playAppSound("call_leave");
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

      try {
        const cloudflare = await createCloudflareScreenPublisher(get().activeConversationId!, stream, track);
        cloudflareScreenConnection = cloudflare.connection;
        sendScreenPublished(cloudflare.sessionId, cloudflare.trackName);
      } catch {
        if (peerConnection) await addScreenTrack(peerConnection, stream, track);
      }

      set({ sharingScreen: true });
      await updateRoomPresence();
      await sendHeartbeat();
    } catch (error) {
      if (error instanceof DOMException && error.name === "NotAllowedError") return;
      toast.error(
        error instanceof Error ? error.message : "Screen sharing could not start."
      );
    }
  },

  stopScreenShare: () => stopLocalScreen(true),

  retryConnection: () => {
    if (!peerConnection || !remoteSessionId) return;
    restartAttempted = false;
    useVoice.setState({ status: "reconnecting", error: null });
    void attemptIceRestart();
  },
}));

async function refreshTurnCredentials(): Promise<void> {
  const state = useVoice.getState();
  if (!state.activeConversationId || (turnCredentialsExpireAt - Date.now()) > TURN_CREDENTIAL_TTL_SAFETY_MS) return;
  const { data, error } = await supabase.functions.invoke("realtime-credentials", {
    body: { conversationId: state.activeConversationId },
  });
  const candidate = data as { iceServers?: unknown; expiresAt?: unknown } | null;
  if (error || !Array.isArray(candidate?.iceServers)) {
    activeIceServers = ICE_SERVERS;
    turnCredentialsExpireAt = 0;
    console.info("Cloudflare TURN is unavailable; continuing with direct WebRTC.");
    return;
  }
  const servers = candidate.iceServers.filter((server): server is RTCIceServer => {
    if (!server || typeof server !== "object") return false;
    const value = server as RTCIceServer;
    return typeof value.urls === "string" || Array.isArray(value.urls);
  });
  if (!servers.length) return;
  activeIceServers = servers;
  turnCredentialsExpireAt = typeof candidate.expiresAt === "number" ? candidate.expiresAt : Date.now() + 12 * 60 * 60_000;
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
      state.outputDeviceId !== previous.outputDeviceId
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

  return () => {
    if (beforeUnloadHandler) {
      window.removeEventListener("beforeunload", beforeUnloadHandler);
      beforeUnloadHandler = null;
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
    supabase.from("voice_participants").select("*"),
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
    toast.error("The voice channel expired after losing its connection.");
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

  useVoice.setState((state) => ({
    participants: {
      ...state.participants,
      [participant.conversation_id]: [
        ...(state.participants[participant.conversation_id] ?? []).filter(
          (entry) => entry.user_id !== participant.user_id
        ),
        participant,
      ],
    },
  }));

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
    playAppSound("call_join");
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
    toast.error("Your voice session expired.");
  } else {
    closePeerConnection();
    playAppSound("call_leave");
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
    toast.info("The selected microphone is unavailable; using the Windows default.");
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
    toast.error(
      error instanceof Error ? error.message : "The microphone could not be changed."
    );
  }
}

async function connectRoomChannel(
  room: VoiceRoom,
  sessionId: string
): Promise<void> {
  if (roomChannel) await supabase.removeChannel(roomChannel);
  roomSubscribed = false;

  const topic =
    "voice:" + room.conversation_id + ":" + room.generation;
  roomChannel = supabase.channel(topic, {
    config: {
      private: true,
      broadcast: { ack: false, self: false },
      presence: { key: sessionId },
    },
  });

  roomChannel
    .on("broadcast", { event: "signal" }, (message) => {
      void handleSignal(message.payload);
    })
    .on("broadcast", { event: "soundboard" }, (message) => {
      const payload = message.payload as Partial<VoiceSoundboardPayload>;
      if (
        payload.version !== 1 ||
        typeof payload.id !== "string" ||
        typeof payload.signedUrl !== "string" ||
        typeof payload.playAt !== "number" ||
        !isTrustedSoundboardUrl(payload.signedUrl) ||
        useVoice.getState().deafened ||
        Date.now() - lastRemoteSoundboardAt < 750
      ) return;
      lastRemoteSoundboardAt = Date.now();
      void playSoundboardUrl(
        payload.id,
        payload.signedUrl,
payload.playAt,
        usePreferences.getState().soundboardVolume,
        usePreferences.getState().outputDeviceId
      ).catch((error) => console.warn("Soundboard playback failed", error));
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

    roomChannel?.subscribe(async (status) => {
      if (status === "SUBSCRIBED") {
        try {
          roomSubscribed = true;
          await updateRoomPresence();
          startHeartbeat();
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
        status === "TIMED_OUT"
      ) {
        if (!settled) {
          settled = true;
          window.clearTimeout(timeout);
          reject(new Error("Voice signaling could not connect."));
        }
      }
    });
  });
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

  const connection = new RTCPeerConnection({
    iceServers: activeIceServers,
    iceCandidatePoolSize: 4,
  });
  peerConnection = connection;

  const audioTrack = microphone.outputStream.getAudioTracks()[0];
  if (audioTrack) connection.addTrack(audioTrack, microphone.outputStream);
  if (localScreenTrack && localScreenStream) {
    void addScreenTrack(connection, localScreenStream, localScreenTrack);
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

  connection.onconnectionstatechange = () => {
    if (peerConnection !== connection) return;
    if (connection.connectionState === "connected") {
      clearDisconnectTimer();
      restartAttempted = false;
      useVoice.setState({ status: "connected", error: null });
    } else if (connection.connectionState === "disconnected") {
      useVoice.setState({ status: "reconnecting" });
      scheduleConnectionFailure();
    } else if (connection.connectionState === "failed") {
      attemptIceRestart();
    } else if (
      connection.connectionState === "connecting" ||
      connection.connectionState === "new"
    ) {
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
      cloudflareScreenConnection = await createCloudflareScreenSubscriber(conversationId, signal.cloudflareSessionId, signal.trackName, (stream) => useVoice.setState({ remoteScreenStream: stream }));
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

function handleRemoteTrack(event: RTCTrackEvent): void {
  const stream = event.streams[0] ?? new MediaStream([event.track]);
  if (event.track.kind === "audio") {
    remoteAudio ??= createRemoteAudioElement();
    void configureRemoteAudio(remoteAudio, {
      stream,
      outputVolume: usePreferences.getState().outputVolume,
      outputDeviceId: usePreferences.getState().outputDeviceId,
      deafened: useVoice.getState().deafened,
    });
    event.track.onended = () => {
      if (remoteAudio?.srcObject === stream) remoteAudio.srcObject = null;
    };
  } else if (event.track.kind === "video") {
    useVoice.setState({ remoteScreenStream: stream });
    event.track.onended = () => {
      if (useVoice.getState().remoteScreenStream === stream) {
        useVoice.setState({ remoteScreenStream: null });
      }
    };
  }
}

function sendScreenPublished(cloudflareSessionId: string, trackName: string): void {
  const signal = buildSignal({ type: "screen-published", cloudflareSessionId, trackName });
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
    | { type: "screen-published"; cloudflareSessionId: string; trackName: string }
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

function sendSignal(signal: VoiceSignal): void {
  if (!roomChannel || !roomSubscribed) return;
  void roomChannel.send({
    type: "broadcast",
    event: "signal",
    payload: signal,
  });
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
      toast.error("Your voice session expired.");
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
    toast.error("Your voice session expired.");
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
  });
}

async function addScreenTrack(
  connection: RTCPeerConnection,
  stream: MediaStream,
  track: MediaStreamTrack
): Promise<void> {
  const sender = connection.addTrack(track, stream);
  const parameters = sender.getParameters();
  if (parameters.encodings.length > 0) {
    parameters.degradationPreference = "maintain-framerate";
    parameters.encodings[0].maxBitrate = 8_000_000;
    parameters.encodings[0].maxFramerate = 60;
    await sender.setParameters(parameters).catch(() => undefined);
  }
}

async function stopLocalScreen(updateServer: boolean): Promise<void> {
  const track = localScreenTrack;
  if (!track) return;

  const sender = peerConnection
    ?.getSenders()
    .find((entry) => entry.track === track);
  if (sender && peerConnection) peerConnection.removeTrack(sender);

  track.onended = null;
  for (const mediaTrack of localScreenStream?.getTracks() ?? []) {
    mediaTrack.stop();
  }
  localScreenStream = null;
  localScreenTrack = null;
  cloudflareScreenConnection?.close();
  cloudflareScreenConnection = null;
  sendScreenStopped();
  useVoice.setState({ sharingScreen: false });

  if (updateServer) {
    await updateRoomPresence();
    await sendHeartbeat();
  }
}

function scheduleConnectionFailure(): void {
  clearDisconnectTimer();
  disconnectTimer = setTimeout(() => {
    if (peerConnection && peerConnection.connectionState !== "connected") void attemptIceRestart();
  }, 10_000);
}

async function attemptIceRestart(): Promise<void> {
  if (!peerConnection || !remoteSessionId || peerConnection.connectionState === "connected") return;
  if (!restartAttempted) {
    restartAttempted = true;
    useVoice.setState({ status: "reconnecting", error: null });
    await refreshTurnCredentials();
    peerConnection?.setConfiguration({ iceServers: activeIceServers });
    peerConnection?.restartIce();
    clearDisconnectTimer();
    disconnectTimer = setTimeout(() => markConnectionFailed(), 20_000);
    return;
  }
  markConnectionFailed();
}

function markConnectionFailed(): void {
  clearDisconnectTimer();
  const message =
    "Direct connection failed. This network may require a TURN relay.";
  useVoice.setState({ status: "failed", error: message });
  toast.error(message, {
    action: {
      label: "Retry",
      onClick: () => useVoice.getState().retryConnection(),
    },
  });
}

function clearDisconnectTimer(): void {
  if (disconnectTimer) clearTimeout(disconnectTimer);
  disconnectTimer = null;
}

function closePeerConnection(setSolo = true): void {
  clearDisconnectTimer();
  const connection = peerConnection;
  peerConnection = null;
  if (connection) {
    connection.onicecandidate = null;
    connection.onnegotiationneeded = null;
    connection.ontrack = null;
    connection.onconnectionstatechange = null;
    connection.close();
  }

  if (remoteAudio) {
    remoteAudio.pause();
    remoteAudio.srcObject = null;
  }
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
  await stopLocalScreen(false);
  closePeerConnection(false);

  const channel = roomChannel;
  roomChannel = null;
  roomSubscribed = false;
  if (channel) {
    await channel.untrack().catch(() => undefined);
    await supabase.removeChannel(channel);
  }

  await stopMicrophonePipeline(microphone);
  microphone = null;

  if (notifyServer && sessionId) {
    await supabase
      .rpc("leave_voice_room", { p_session_id: sessionId })
      .then(({ error }) => {
        if (error) console.warn("Voice leave failed", error);
      });
  }

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
      remoteScreenStream: null,
      error: null,
    };
  });

  disconnecting = false;
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

export function broadcastVoiceSoundboard(payload: VoiceSoundboardPayload): void {
  if (!roomChannel || !roomSubscribed || !useVoice.getState().activeConversationId) return;
  void roomChannel.send({
    type: "broadcast",
    event: "soundboard",
    payload,
  });
}
export function formatVoiceElapsed(
  startedAt: string | undefined,
  now = Date.now()
): string {
  if (!startedAt) return "0:00";
  const seconds = Math.max(
    0,
    Math.floor((now - new Date(startedAt).getTime()) / 1000)
  );
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  if (hours > 0) {
    return (
      hours +
      ":" +
      String(minutes).padStart(2, "0") +
      ":" +
      String(remainder).padStart(2, "0")
    );
  }
  return minutes + ":" + String(remainder).padStart(2, "0");
}