import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ScreenSharePreview } from "@/features/chat/screen-share-preview";
import { playAppSound } from "@/lib/app-sounds";
import { decorationUrl } from "@/lib/avatar-decorations";
import { supabase } from "@/lib/supabase";
import { isTauri } from "@/lib/tauri";
import { hideVoiceHud, resizeVoiceHud, showVoiceHud, updateVoiceHud } from "@/lib/voice-hud";
import { eventKeybind } from "@/lib/keybinds";
import { prepareChatMedia } from "@/lib/media";
import { toast } from "sonner";
import { checkForUpdateManually } from "@/lib/updater";
import { useAlerts } from "@/stores/alerts";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useProfiles } from "@/stores/profiles";
import { usePresenceStatus } from "@/stores/presence";
import { usePreferences } from "@/stores/preferences";
import { useSoundboard } from "@/stores/soundboard";
import { useVoice } from "@/stores/voice";
import {
  CircleAlert,
  CircleCheck,
  CircleX,
  LoaderCircle,
  MessageCircle,
  ScreenShare,
  Settings,
  X,
  TriangleAlert,
} from "lucide-react";
import { ActionButton } from "./components/ActionButton";
import { AlertBar } from "./components/AlertBar";
import { AvatarBadge } from "./components/AvatarBadge";
import { AvatarButton } from "./components/AvatarButton";
import { DeafenToggleDropdown } from "./components/DeafenToggleDropdown";
import { InputBar } from "./components/InputBar";
import { HistoryMarker } from "./components/HistoryMarker";
import { MessageTemplate } from "./components/MessageTemplate";
import { MicrophoneToggleDropdown } from "./components/MicrophoneToggleDropdown";
import { MiniBadge } from "./components/MiniBadge";
import { SoundboardDropdown } from "./components/SoundboardDropdown";
import { SoundToggleButton } from "./components/SoundToggleButton";
import { TopAlert } from "./components/TopAlert";
import { TypingIndicator } from "./components/TypingIndicator";
import { VoiceCallButton } from "./components/VoiceCallButton";
import { V3ChatSwitcher } from "./components/V3ChatSwitcher";
import { V3Dashboard } from "./components/V3Dashboard";
import { V3Lightbox } from "./components/V3Lightbox";
import { V3MediaSidebar } from "./components/V3MediaSidebar";
import { useUiSounds } from "./hooks/useUiSounds";
import "./styles.css";

const EMPTY_MESSAGES = [];
const EMPTY_PARTICIPANTS = [];

// Slash commands never get sent as chat messages - handleComposerSubmit
// intercepts anything that resolves to one of these before it ever reaches
// sendMessage, so nothing about them touches the message history.
const SLASH_COMMANDS = [
  { name: "shrug", description: "Append ¯\\_(ツ)_/¯ to your message" },
  { name: "randomsound", description: "Play a random soundboard sound (while in a call)" },
  { name: "update", description: "Check for app updates" },
  { name: "testalert", description: "Show a random alert to test the banner" },
];

const alertVariants = [
  { severity: "neutral", message: "Heads up — nothing urgent here.", icon: CircleAlert },
  { severity: "neutral", message: "Settings saved.", icon: CircleCheck },
  { severity: "warning", message: "Connection looks unstable.", icon: TriangleAlert },
  {
    severity: "danger",
    message: "Delete this conversation? This can't be undone.",
    icon: CircleX,
    actions: [{ label: "Cancel" }, { label: "Delete", confirm: true }],
  },
];

function displayName(profile, fallback = "Unknown") {
  return profile?.display_name?.trim() || profile?.username?.trim() || fallback;
}

function avatarUrl(profile) {
  const path = profile?.avatar_animated_path || profile?.avatar_path;
  if (!path) return null;
  const url = supabase.storage.from("avatars").getPublicUrl(path).data.publicUrl;
  return profile?.avatar_updated_at ? `${url}?v=${encodeURIComponent(profile.avatar_updated_at)}` : url;
}

function dayLabel(value) {
  const date = new Date(value);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return "Today";
  if (date.toDateString() === yesterday.toDateString()) return "Yesterday";
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(date);
}

function startsNewMessageGroup(message, previous) {
  if (!previous || message.message_kind !== "chat" || previous.message_kind !== "chat") return true;
  return message.sender_id !== previous.sender_id || new Date(message.created_at).getTime() - new Date(previous.created_at).getTime() >= 60_000;
}

// Same "same sender, nothing else in between" check as startsNewMessageGroup,
// minus the 60-second cutoff - three "(deleted)" rows from the same person
// twenty minutes apart still have nothing to visually separate them (no
// call marker, no message from the other side), so they read better merged
// into one "(3 deleted)" line than as three near-identical empty headers.
function breaksDeletedRun(message, previous) {
  if (!previous || message.message_kind !== "chat" || previous.message_kind !== "chat") return true;
  return message.sender_id !== previous.sender_id;
}

function formatCallDuration(value) {
  const seconds = Math.max(0, value ?? 0);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`
    : `${minutes}:${String(remainder).padStart(2, "0")}`;
}

function systemLabel(message) {
  if (message.message_kind === "voice_started") return "Voicechat started";
  if (message.message_kind === "voice_ended") return `Call lasted ${formatCallDuration(message.voice_duration_seconds)}`;
  return null;
}
function messageTimestamp(value) {
  return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(new Date(value));
}
const controls = [  { label: "Settings", icon: Settings },
  { label: "Chat", icon: MessageCircle, className: "chat-button" },
  { label: "Confirm", text: "Confirm", className: "chat-button action-button" },
];

function replyExcerpt(message) {
  if (!message) return "Message";
  if (message.deleted_at) return "Message deleted";
  if (message.content) return message.content;
  if (message.media_kind === "image") return "Image";
  if (message.media_kind === "video") return "Video";
  return "Message";
}

function V3LoadingShell() {
  return (
    <main className="stage v3-loading-stage" aria-busy="true" aria-label="Loading Nitro">
      <div className="v3-loading-indicator" role="status">
        <LoaderCircle aria-hidden="true" />
        <span>Loading conversations</span>
      </div>
    </main>
  );
}
function createMessageTimestamp() {
  const time = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(new Date());
  return `Today at ${time}`;
}

export function V3Shell() {
  const [openDropdown, setOpenDropdown] = useState(null);
  const activeAlert = useAlerts((state) => state.active);
  const dismissAlert = useAlerts((state) => state.dismiss);
  // Mirrors TopAlert's own internal slide visibility, not activeAlert
  // directly - activeAlert only clears ~340ms AFTER the banner starts
  // sliding away (TopAlert waits out its own exit transition before
  // calling dismiss), so driving has-alert-banner off activeAlert made the
  // content-push reverse a third of a second after the banner had already
  // finished disappearing. This flips the instant the slide starts/ends,
  // so the banner and the layout shift move together.
  const [bannerVisible, setBannerVisible] = useState(false);
  const [composerValue, setComposerValue] = useState("");
  const [pendingMedia, setPendingMedia] = useState(null);
  const [pendingPreviewUrl, setPendingPreviewUrl] = useState(null);
  const [mediaStage, setMediaStage] = useState("idle");
  const [hoveredDecorationHeaderId, setHoveredDecorationHeaderId] = useState(null);
  const [isSelfTyping, setIsSelfTyping] = useState(false);
  // Lives in useChat, not local state - the toggle button moved into the
  // global titlebar (a separate portaled React tree, see
  // components/titlebar.tsx), which needs to read/flip the same value.
  const mediaSidebarOpen = useChat((state) => state.mediaSidebarOpen);
  const userId = useAuth((state) => state.userId);
  const selfProfile = useAuth((state) => state.profile);
  const activeId = useChat((state) => state.activeId);
  const chatView = useChat((state) => state.view);
  const conversations = useChat((state) => state.conversations);
  const loaded = useChat((state) => state.loaded);
  const hasMore = useChat((state) => activeId ? state.hasMore[activeId] ?? false : false);
  const loadingOlder = useChat((state) => state.loadingOlder);
  const messagesByConversation = useChat((state) => state.messages);
  const liveMessages = activeId ? messagesByConversation[activeId] ?? EMPTY_MESSAGES : EMPTY_MESSAGES;
  const activeConversation = useChat((state) => state.conversations.find((entry) => entry.id === activeId));
  const partnerId = activeConversation && userId
    ? activeConversation.user1_id === userId ? activeConversation.user2_id : activeConversation.user1_id
    : null;
  const partnerProfile = useProfiles((state) => partnerId ? state.byId[partnerId] : undefined);
  const partnerPresence = usePresenceStatus(partnerId);
  const typingUserId = useChat((state) => activeId ? state.typing[activeId] : null);
  const replyTo = useChat((state) => state.replyTo);
  const replyTargets = useChat((state) => state.replyTargets);
  const voiceRooms = useVoice((state) => state.rooms);
  const voiceParticipantsByConversation = useVoice((state) => state.participants);
  const activeVoiceId = useVoice((state) => state.activeConversationId);
  const voiceStatus = useVoice((state) => state.status);
  const muted = useVoice((state) => state.muted);
  const deafened = useVoice((state) => state.deafened);
  const voiceSpeaking = useVoice((state) => state.speaking);
  const voiceLevel = useVoice((state) => state.level);
  const sharingScreen = useVoice((state) => state.sharingScreen);
  const voiceHudScale = usePreferences((state) => state.voiceHudScale);
  const voiceHudShowNames = usePreferences((state) => state.voiceHudShowNames);
  const voiceParticipants = activeId ? voiceParticipantsByConversation[activeId] ?? EMPTY_PARTICIPANTS : EMPTY_PARTICIPANTS;
  const voiceRoom = activeId ? voiceRooms[activeId] : undefined;
  const joinedVoice = activeVoiceId === activeId;
  const typingProfile = typingUserId === userId ? selfProfile : partnerProfile;
  const replyProfile = replyTo?.sender_id === userId ? selfProfile : partnerProfile;
  const voiceParticipantDetails = useMemo(() => voiceParticipants.map((participant) => {
    const isSelf = participant.user_id === userId;
    const profile = isSelf ? selfProfile : partnerProfile;
    return {
      id: participant.user_id,
      avatar: avatarUrl(profile),
      name: displayName(profile, isSelf ? "You" : "Partner"),
      avatarDecoration: profile?.avatar_decoration ?? null,
      nameColor: profile?.name_color ?? null,
      nameDecoration: profile?.name_decoration ?? null,
      nameFont: profile?.name_font ?? null,
      nameWeight: profile?.name_weight ?? null,
      speaking: Boolean(voiceSpeaking[participant.user_id]),
      level: voiceLevel[participant.user_id] ?? 0,
    };
  }), [voiceParticipants, userId, selfProfile, partnerProfile, voiceSpeaking, voiceLevel]);
  // Collapse a run of consecutive voice_started/voice_ended markers (no real
  // chat message between them) down to just the last one, as long as every
  // call in that run was under 15 minutes - short join/leave/reconnect
  // bursts otherwise spam the whole history with one line per event.
  const collapsedVoiceMarkerIds = useMemo(() => {
    const skip = new Set();
    let runStart = -1;
    let runEnd = -1;
    let hasLongCall = false;
    const flush = () => {
      if (runStart !== -1 && runEnd > runStart && !hasLongCall) {
        for (let i = runStart; i < runEnd; i += 1) skip.add(liveMessages[i].id);
      }
      runStart = -1;
      runEnd = -1;
      hasLongCall = false;
    };
    liveMessages.forEach((message, index) => {
      const isVoiceEvent = message.message_kind === "voice_started" || message.message_kind === "voice_ended";
      if (!isVoiceEvent) {
        flush();
        return;
      }
      if (runStart === -1) runStart = index;
      runEnd = index;
      if (message.message_kind === "voice_ended" && (message.voice_duration_seconds ?? 0) >= 900) hasLongCall = true;
    });
    flush();
    return skip;
  }, [liveMessages]);
  // Collapse a run of consecutive deleted messages within the same visual
  // group (same sender, no real content or event between them) down to a
  // single "(N deleted)" line instead of one empty "(deleted)" row each.
  const deletedRuns = useMemo(() => {
    const counts = new Map();
    const hidden = new Set();
    let runStart = -1;
    const flush = (end) => {
      if (runStart !== -1 && end - runStart > 1) {
        counts.set(liveMessages[runStart].id, end - runStart);
        for (let i = runStart + 1; i < end; i += 1) hidden.add(liveMessages[i].id);
      }
      runStart = -1;
    };
    liveMessages.forEach((message, index) => {
      const isDeletedChat = message.message_kind === "chat" && Boolean(message.deleted_at);
      if (!isDeletedChat || breaksDeletedRun(message, liveMessages[index - 1])) flush(index);
      if (isDeletedChat && runStart === -1) runStart = index;
    });
    flush(liveMessages.length);
    return { counts, hidden };
  }, [liveMessages]);
  const hasActiveMessages = Boolean(activeId && Object.prototype.hasOwnProperty.call(messagesByConversation, activeId));
  const isInitialLoad = !loaded || (conversations.length > 0 && (!activeId || !hasActiveMessages));
  const uiSounds = useUiSounds();
  const messageHistoryRef = useRef(null);
  const messageListRef = useRef(null);
  // Explicit "should this stay pinned to the bottom" intent, instead of a
  // pixel-gap re-check every time - content can grow across several async
  // stages (an image, then a tweet/link embed fetch resolving later, then
  // another), and re-deriving "near enough to the bottom" after each one
  // drifts further off with every stage. Tracking intent instead means it
  // stays correct no matter how many stages there are.
  const atBottomRef = useRef(true);
  // Set right before we programmatically pin scrollTop to the bottom (see
  // pinToBottom below), cleared by the very next scroll event. Assigning
  // scrollTop fires a real 'scroll' event, which the handler below also
  // uses to detect the user manually scrolling away - without this flag,
  // if content grows (an image/embed finishing its load) between our own
  // assignment and that resulting event actually firing, the handler would
  // recompute a stale gap-to-bottom from a scrollHeight that's already out
  // of date and wrongly conclude the user scrolled up, permanently
  // stopping the auto-follow the atBottomRef comment above describes -
  // this is the fix for exactly that race.
  const programmaticScrollRef = useRef(false);
  const loadingOlderRef = useRef(false);
  const prependRestoreRef = useRef(null);
  const initialConversationRef = useRef(null);
  const lastMessageCountRef = useRef(0);
  const attachmentInputRef = useRef(null);
  const typingTimerRef = useRef(null);
  const decorationGroups = useMemo(() => { const headers = new Map(); const latest = new Map(); let headerId = null; liveMessages.forEach((message, index) => { if (startsNewMessageGroup(message, liveMessages[index - 1])) headerId = message.id; headers.set(message.id, headerId); if (message.message_kind === "chat") latest.set(message.sender_id, headerId); }); return { headers, autoplay: new Set(latest.values()) }; }, [liveMessages]);

  useEffect(() => {
    if (!userId) return;
    void useChat.getState().loadConversations();
    return useChat.getState().subscribe();
  }, [userId]);

  useEffect(() => {
    if (loaded && !activeId && conversations[0]) {
      useChat.getState().openConversation(conversations[0].id);
    }
  }, [activeId, conversations, loaded]);
  useEffect(() => {
    if (!activeId) return;
    return useChat.getState().joinTyping(activeId);
  }, [activeId]);
  useEffect(() => {
    const profileIds = [partnerId, ...voiceParticipants.map((participant) => participant.user_id)].filter(Boolean);
    if (profileIds.length) void useProfiles.getState().ensure(profileIds);
  }, [partnerId, voiceParticipants]);
  const lastVoiceHealthRef = useRef(null);
  const stuckReconnectTimerRef = useRef(null);
  useEffect(() => {
    if (voiceStatus === "reconnecting" || voiceStatus === "failed") {
      if (lastVoiceHealthRef.current !== voiceStatus) playAppSound("voice_reconnect", true);
      lastVoiceHealthRef.current = voiceStatus;
    } else lastVoiceHealthRef.current = null;

    // A brief "reconnecting" blip usually clears itself within a couple of
    // seconds - only surface a banner (with a manual escape hatch) once
    // it's been stuck long enough that the automatic recovery is plausibly
    // not working.
    if (voiceStatus === "reconnecting") {
      if (!stuckReconnectTimerRef.current) {
        stuckReconnectTimerRef.current = window.setTimeout(() => {
          stuckReconnectTimerRef.current = null;
          if (useVoice.getState().status !== "reconnecting") return;
          useAlerts.getState().show({
            severity: "warning",
            message: "Still trying to reconnect your call.",
            actions: [
              { label: "Dismiss" },
              { label: "Reconnect", confirm: true, onClick: () => void useVoice.getState().forceReconnect() },
            ],
          });
        }, 10_000);
      }
    } else if (stuckReconnectTimerRef.current) {
      window.clearTimeout(stuckReconnectTimerRef.current);
      stuckReconnectTimerRef.current = null;
    }
  }, [voiceStatus]);
  useEffect(() => {
    if (replyTo) document.getElementById("message-composer")?.focus();
  }, [replyTo]);
  useEffect(() => {
    if (!isTauri) return;
    if (joinedVoice) void showVoiceHud(voiceHudScale);
    else void hideVoiceHud();
    // Scale is intentionally excluded here: opening/closing the HUD should
    // only react to joining/leaving voice, not to the slider mid-call (that's
    // handled by the resize effect below without a hide/show flash).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [joinedVoice]);
  useEffect(() => {
    if (!isTauri || !joinedVoice) return;
    void resizeVoiceHud(voiceHudScale);
  }, [joinedVoice, voiceHudScale]);
  useEffect(() => {
    if (!isTauri || !joinedVoice) return;
    void updateVoiceHud({ participants: voiceParticipantDetails, scale: voiceHudScale, showNames: voiceHudShowNames });
  }, [joinedVoice, voiceParticipantDetails, voiceHudScale, voiceHudShowNames]);

  async function prepareAttachment(file) {
    if (!file) return;
    setMediaStage("preparing");
    try {
      const { data: capability, error } = await supabase.functions.invoke("purge-chat-media", { body: { mode: "capability" } });
      if (error) throw new Error(error.message);
      const media = await prepareChatMedia(file, undefined, capability?.provider === "cloudinary" ? "cloudinary" : "storage");
      setPendingMedia(media);
      setPendingPreviewUrl(URL.createObjectURL(media.blob));
      setMediaStage("ready");
    } catch (error) {
      setMediaStage("idle");
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "That attachment could not be prepared." });
    }
  }

  // Once the leading token exactly matches a known command, it "locks in" as
  // a chip (see InputBar's commandName prop) instead of staying plain text -
  // the suggestion dropdown steps aside at that point since the chip itself
  // now shows what's about to run.
  const activeCommand = useMemo(() => {
    if (!composerValue.startsWith("/")) return null;
    const spaceIndex = composerValue.indexOf(" ");
    const name = (spaceIndex === -1 ? composerValue.slice(1) : composerValue.slice(1, spaceIndex)).toLowerCase();
    return SLASH_COMMANDS.find((command) => command.name === name) ?? null;
  }, [composerValue]);

  const matchedCommands = useMemo(() => {
    // Once there's a space, or the name is already an exact match (-> chip),
    // the user has moved past picking a command - stop suggesting.
    if (!composerValue.startsWith("/") || composerValue.includes(" ") || activeCommand) return [];
    const query = composerValue.slice(1).toLowerCase();
    return SLASH_COMMANDS.filter((command) => command.name.startsWith(query));
  }, [composerValue, activeCommand]);

  // The text the visible <input> shows while a command chip is active - just
  // the argument portion, with the "/name" prefix hidden behind the chip.
  const commandArgText = useMemo(() => {
    if (!activeCommand) return composerValue;
    const afterName = composerValue.slice(activeCommand.name.length + 1);
    return afterName.startsWith(" ") ? afterName.slice(1) : afterName;
  }, [composerValue, activeCommand]);

  function handleComposerArgChange(nextArg) {
    if (!activeCommand) {
      handleComposerChange(nextArg);
      return;
    }
    handleComposerChange(nextArg ? `/${activeCommand.name} ${nextArg}` : `/${activeCommand.name}`);
  }

  function handleComposerKeyDown(event) {
    // Backspacing on an empty argument with a chip showing would otherwise
    // do nothing (there's no text left in the visible input to delete) -
    // treat it as "never mind" and drop back to a blank composer instead of
    // leaving the chip stuck with no way to remove it.
    if (event.key === "Backspace" && activeCommand && !commandArgText) {
      event.preventDefault();
      setComposerValue("");
    }
  }

  // Returns { text } when the command transforms into a normal message to
  // send (e.g. /shrug), `true` when it's fully handled and nothing should be
  // sent (e.g. /update), or `false` if `name` isn't a recognized command.
  function runSlashCommand(name, rest = "") {
    if (name === "shrug") {
      return { text: rest ? `${rest} ¯\\_(ツ)_/¯` : "¯\\_(ツ)_/¯" };
    }
    if (name === "randomsound") {
      const sounds = useSoundboard.getState().sounds;
      if (!sounds.length) {
        useAlerts.getState().show({ severity: "neutral", message: "No soundboard sounds available." });
        return true;
      }
      void useSoundboard.getState().play(sounds[Math.floor(Math.random() * sounds.length)].id);
      return true;
    }
    if (name === "update") {
      void checkForUpdateManually();
      return true;
    }
    if (name === "testalert") {
      showAlert();
      return true;
    }
    return false;
  }

  function clearComposerAfterCommand() {
    setComposerValue("");
    setIsSelfTyping(false);
    if (typingTimerRef.current) window.clearTimeout(typingTimerRef.current);
    document.getElementById("message-composer")?.focus();
  }

  async function handleComposerSubmit() {
    if (!activeId || (!composerValue.trim() && !pendingMedia)) return;
    const trimmed = composerValue.trim();
    let textToSend = composerValue;
    if (trimmed.startsWith("/")) {
      const spaceIndex = trimmed.indexOf(" ");
      const typedName = (spaceIndex === -1 ? trimmed.slice(1) : trimmed.slice(1, spaceIndex)).toLowerCase();
      const rest = spaceIndex === -1 ? "" : trimmed.slice(spaceIndex + 1).trim();
      const exactMatch = SLASH_COMMANDS.find((command) => command.name === typedName);
      const command = exactMatch ?? (spaceIndex === -1 && matchedCommands.length === 1 ? matchedCommands[0] : null);
      if (command) {
        const result = runSlashCommand(command.name, rest);
        if (result === true) {
          clearComposerAfterCommand();
          return;
        }
        if (result && typeof result === "object") textToSend = result.text;
      }
    }
    if (pendingMedia) setMediaStage("uploading");
    const sent = await useChat.getState().sendMessage(activeId, textToSend, pendingMedia, useChat.getState().replyTo?.id ?? null);
    if (sent) {
      setComposerValue("");
      setPendingMedia(null);
      if (pendingPreviewUrl) URL.revokeObjectURL(pendingPreviewUrl);
      setPendingPreviewUrl(null);
      setMediaStage("idle");
      useChat.getState().setReplyTo(null);
      setIsSelfTyping(false);
      if (typingTimerRef.current) window.clearTimeout(typingTimerRef.current);
      uiSounds.message();
      document.getElementById("message-composer")?.focus();
    } else if (pendingMedia) {
      setMediaStage("ready");
    }
  }

  function handleComposerChange(value) {
    setComposerValue(value);
    if (!activeId) return;
    useChat.getState().notifyTyping(activeId);
    const typing = Boolean(value.trim());
    setIsSelfTyping(typing);
    if (typingTimerRef.current) window.clearTimeout(typingTimerRef.current);
    if (typing) typingTimerRef.current = window.setTimeout(() => setIsSelfTyping(false), 2_500);
  }

  function handleComposerPaste(event) {
    const file = [...(event.clipboardData?.files ?? [])].find((entry) => entry.type.startsWith("image/") || entry.type.startsWith("video/"));
    if (!file) return;
    event.preventDefault();
    void prepareAttachment(file);
  }

  // Type-to-focus: with no other UI element specifically focused, jump
  // straight into the composer and start typing rather than requiring a
  // click first - like Discord/Slack.
  useEffect(() => {
    const onKeyDown = (event) => {
      if (!activeId || event.ctrlKey || event.altKey || event.metaKey) return;
      if (event.key.length !== 1) return; // only plain printable characters
      const active = document.activeElement;
      const tag = active?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || active?.isContentEditable) return;
      if (active?.closest?.('[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]')) return;
      // A configured keybind (mute, leave voice, etc.) takes priority over
      // typing, even if it happens to be a bare key.
      const binding = eventKeybind(event);
      if (binding && Object.values(usePreferences.getState().keybinds).includes(binding)) return;
      const input = document.getElementById("message-composer");
      if (!input) return;
      event.preventDefault();
      const next = composerValue + event.key;
      handleComposerChange(next);
      input.focus();
      requestAnimationFrame(() => input.setSelectionRange(next.length, next.length));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeId, composerValue]);

  function showAlert() {
    const variant = alertVariants[Math.floor(Math.random() * alertVariants.length)];
    const actions = variant.actions?.map((action) => ({
      ...action,
      onClick: () => toast(action.confirm ? `${action.label} confirmed` : `${action.label} cancelled`),
    }));

    useAlerts.getState().show({ ...variant, actions });
    uiSounds.alert();
  }

  function closeDropdown() {
    if (openDropdown === null) {
      return;
    }

    setOpenDropdown(null);
    uiSounds.menu(false);
  }

  function handleDropdownChange(dropdown, isOpen) {
    const nextDropdown = isOpen ? dropdown : null;
    if (nextDropdown === openDropdown) {
      return;
    }

    const closingDropdown = openDropdown !== null;
    setOpenDropdown(nextDropdown);

    if (closingDropdown) {
      uiSounds.menu(false);
    }

    if (nextDropdown !== null) {
      uiSounds.menu(true);
    }
  }


  const pinToBottom = useCallback(() => {
    const history = messageHistoryRef.current;
    if (!history) return;
    programmaticScrollRef.current = true;
    history.scrollTop = history.scrollHeight;
  }, []);

  const loadOlderMessages = useCallback(async () => {
    const history = messageHistoryRef.current;
    if (!history || !activeId || !hasMore || loadingOlderRef.current) return;
    loadingOlderRef.current = true;
    prependRestoreRef.current = { height: history.scrollHeight, top: history.scrollTop };
    await useChat.getState().loadOlder(activeId);
    loadingOlderRef.current = false;
  }, [activeId, hasMore]);

  useLayoutEffect(() => {
    const history = messageHistoryRef.current;
    const restore = prependRestoreRef.current;
    if (!history || !restore) return;
    history.scrollTop = history.scrollHeight - restore.height + restore.top;
    // Leave prependRestoreRef set - the effect below (same liveMessages
    // change, but a passive effect so it always runs after this layout
    // effect) still needs to see "this update was a prepend" so it doesn't
    // undo the restore above with its own scroll-to-bottom logic. It clears
    // the ref once it's done checking.
  }, [liveMessages, isInitialLoad]);

  useEffect(() => {
    const history = messageHistoryRef.current;
    if (!history) return;
    const count = liveMessages.length;
    const previousCount = lastMessageCountRef.current;
    lastMessageCountRef.current = count;
    if (initialConversationRef.current !== activeId) {
      initialConversationRef.current = activeId;
      atBottomRef.current = true;
      requestAnimationFrame(pinToBottom);
      return;
    }
    if (prependRestoreRef.current) {
      prependRestoreRef.current = null;
      return;
    }
    if (count <= previousCount) return;
    const last = liveMessages[count - 1];
    if (atBottomRef.current || last?.sender_id === userId) {
      atBottomRef.current = true;
      requestAnimationFrame(pinToBottom);
    }
    // isInitialLoad is the real gate here: the .message-history section
    // doesn't exist in the DOM at all until it flips false (a loading
    // shell renders in its place), so messageHistoryRef.current is still
    // null on every render before that - this effect would otherwise run
    // once too early, bail out on the null ref, and never get a second
    // chance once the section actually mounts, since activeId/liveMessages
    // may not change again right at that transition.
  }, [activeId, liveMessages, userId, isInitialLoad]);

  // Messages can grow taller after they're already on screen - an image or
  // tweet/link embed finishing its own async load, for instance - which the
  // effect above never sees since liveMessages itself didn't change. Content
  // can grow across several such stages in a row (image, then a link-preview
  // fetch resolving later, then another) - re-pin on every single one of
  // them as long as atBottomRef is still true, rather than only once, so it
  // actually converges on the true final bottom instead of settling short.
  useEffect(() => {
    const history = messageHistoryRef.current;
    const list = messageListRef.current;
    if (!history || !list) return;
    const observer = new ResizeObserver(() => {
      if (prependRestoreRef.current || !atBottomRef.current) return;
      pinToBottom();
    });
    observer.observe(list);
    return () => observer.disconnect();
  }, [activeId, isInitialLoad]);

  useEffect(() => {
    function closeDropdownOnOutsideClick(event) {
      if (!(event.target instanceof Element) || !event.target.closest(".audio-control")) {
        closeDropdown();
      }
    }

    document.addEventListener("pointerdown", closeDropdownOnOutsideClick);
    return () => document.removeEventListener("pointerdown", closeDropdownOnOutsideClick);
  }, [openDropdown]);

  if (isInitialLoad) return <V3LoadingShell />;

  return (
    <main className={"stage" + (joinedVoice ? " is-in-call" : "") + (bannerVisible ? " has-alert-banner" : "") + (mediaSidebarOpen ? " has-media-sidebar" : "")} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); const file = [...event.dataTransfer.files].find((entry) => entry.type.startsWith("image/") || entry.type.startsWith("video/")); if (file) void prepareAttachment(file); }}>
      <div className={"v3-call-glow" + (joinedVoice ? " is-active" : "")} aria-hidden="true" />
      <div className="window-drag-region" data-tauri-drag-region aria-hidden="true" />
      <div className="v3-header-fade" aria-hidden="true">
        <span className="v3-header-fade__opacity" />
      </div>
      <div className="v3-footer-fade" aria-hidden="true">
        <span className="v3-footer-fade__opacity" />
      </div>
      {activeAlert && (
        <div className="top-alert-region" aria-live="polite" aria-atomic="true">
          <TopAlert
            id={activeAlert.id}
            message={activeAlert.message}
            severity={activeAlert.severity}
            icon={activeAlert.icon}
            actions={activeAlert.actions}
            onDismiss={dismissAlert}
            onVisibleChange={setBannerVisible}
          />
        </div>
      )}

      {chatView === "friends" && <V3Dashboard />}

      {chatView === "chat" && <section ref={messageHistoryRef} className="message-history" aria-label="Chat messages" onScroll={(event) => { const el = event.currentTarget; if (programmaticScrollRef.current) { programmaticScrollRef.current = false; } else { atBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40; } if (el.scrollTop < 96) void loadOlderMessages(); }}>
        <div className="message-list" ref={messageListRef}>
          {hasMore && <div className="v3-load-older">{loadingOlder ? "Loading earlier messages…" : "Scroll up for earlier messages"}</div>}
          {liveMessages.map((message, index) => {
            const previous = liveMessages[index - 1];
            const isSelf = message.sender_id === userId;
            const profile = isSelf ? selfProfile : partnerProfile;
            const marker = systemLabel(message);
            const dateChanged = !previous || new Date(previous.created_at).toDateString() !== new Date(message.created_at).toDateString();
            if (marker) {
              if (collapsedVoiceMarkerIds.has(message.id)) return null;
              return <HistoryMarker key={message.id}>{marker}</HistoryMarker>;
            }
            if (deletedRuns.hidden.has(message.id)) return null;
            const replyTarget = message.reply_to_message_id ? replyTargets[message.reply_to_message_id] : null;
            const replyTargetProfile = replyTarget ? (replyTarget.sender_id === userId ? selfProfile : partnerProfile) : null;
            const replyPreview = message.reply_to_message_id ? {
              target: replyTarget ?? null,
              authorName: replyTarget ? displayName(replyTargetProfile, "Message") : null,
              authorAvatar: replyTarget ? avatarUrl(replyTargetProfile) : null,
              authorNameColor: replyTargetProfile?.name_color,
              onJump: replyTarget ? () => document.getElementById("message-" + replyTarget.id)?.scrollIntoView({ behavior: "smooth", block: "center" }) : null,
            } : null;
            return (
              <div key={message.id}>
                {dateChanged && <HistoryMarker>{dayLabel(message.created_at)}</HistoryMarker>}
                <MessageTemplate
                  name={displayName(profile, isSelf ? "You" : "Partner")}
                  avatar={avatarUrl(profile)}
                  avatarDecoration={profile?.avatar_decoration}
                  nameDecoration={profile?.name_decoration}
                  nameColor={profile?.name_color}
                  nameFont={profile?.name_font}
                  nameWeight={profile?.name_weight}
                  showMeta={startsNewMessageGroup(message, previous)}
                  message={message.deleted_at ? null : message.content}
                  media={!message.deleted_at && message.media_kind ? message : null}
                  isDeleted={Boolean(message.deleted_at)}
                  deletedCount={deletedRuns.counts.get(message.id) ?? 1}
                  isEdited={Boolean(message.edited_at)}
                  timestamp={messageTimestamp(message.created_at)}
                  sourceMessage={message}
                  replyPreview={replyPreview}
                  decorationActive={decorationGroups.autoplay.has(message.id) || hoveredDecorationHeaderId === decorationGroups.headers.get(message.id)}
                  onDecorationHoverChange={(hovered) => setHoveredDecorationHeaderId(hovered ? decorationGroups.headers.get(message.id) : null)}
                />
              </div>
            );
          })}
        </div>
      </section>}

      <nav className="top-audio-controls" aria-label="Audio controls">
        <V3ChatSwitcher partnerProfile={partnerProfile} partnerPresence={partnerPresence} />
        <VoiceCallButton active={joinedVoice} roomStartedAt={voiceRoom?.started_at} participants={voiceParticipantDetails} participantCount={voiceParticipants.length} hasParticipants={voiceParticipants.length > 0} onJoin={() => activeId && useVoice.getState().join(activeId, true)} onLeave={() => useVoice.getState().leave()} />
        {joinedVoice && <>
          <SoundboardDropdown conversationId={activeId} isMenuOpen={openDropdown === "soundboard"} onMenuOpenChange={(isOpen) => handleDropdownChange("soundboard", isOpen)} />
          <ActionButton label={sharingScreen ? "Stop streaming" : "Start streaming"} icon={ScreenShare} className={"streaming-button" + (sharingScreen ? " is-streaming" : "")} onClick={() => void (sharingScreen ? useVoice.getState().stopScreenShare() : useVoice.getState().startScreenShare())} />
          <MicrophoneToggleDropdown isMuted={muted} onToggle={() => useVoice.getState().toggleMute()} isMenuOpen={openDropdown === "microphone"} onMenuOpenChange={(isOpen) => handleDropdownChange("microphone", isOpen)} />
          <DeafenToggleDropdown isDeafened={deafened} onToggle={() => useVoice.getState().toggleDeafen()} isMenuOpen={openDropdown === "deafen"} onMenuOpenChange={(isOpen) => handleDropdownChange("deafen", isOpen)} />
        </>}
      </nav>

      <div className="v3-screen-previews">
        <ScreenSharePreview source="remote" />
        <ScreenSharePreview source="local" />
      </div>
      {chatView === "chat" && <V3MediaSidebar />}

      {chatView === "chat" && <div className="bottom-composer">
        {(isSelfTyping || typingUserId) && <TypingIndicator name={typingUserId ? displayName(typingProfile, "Partner") : displayName(selfProfile, "You")} avatar={typingUserId ? avatarUrl(typingProfile) : avatarUrl(selfProfile)} />}
        {replyTo && <div className="v3-reply-banner">
          <span className="v3-reply-banner__icon" aria-hidden="true">↩</span>
          <div className="v3-reply-banner__copy"><strong>Replying to {displayName(replyProfile, "message")}</strong><span>{replyExcerpt(replyTo)}</span></div>
          <button type="button" aria-label="Cancel reply" onClick={() => useChat.getState().setReplyTo(null)}>×</button>
        </div>}
        {pendingMedia && <div className="v3-pending-media">
          <span className="v3-pending-media__preview">{pendingMedia.kind === "video" ? <video src={pendingPreviewUrl ?? undefined} muted playsInline /> : <img src={pendingPreviewUrl ?? undefined} alt="Attachment ready to send" />}</span>
          <span className="v3-pending-media__copy"><strong>{mediaStage === "uploading" ? "Uploading media…" : mediaStage === "preparing" ? "Preparing media…" : pendingMedia.kind === "video" ? "Video ready" : "Image ready"}</strong><span>{mediaStage === "uploading" ? "Sending securely to Cloudinary" : mediaStage === "preparing" ? "Compressing locally before upload" : pendingMedia.kind === "video" ? "Cloudinary · 720p / 30 fps" : "Cloudinary optimized"}</span></span>
          <span className={"v3-pending-media__progress is-" + mediaStage} aria-hidden="true" /><button type="button" aria-label="Remove attachment" title="Remove attachment" disabled={mediaStage === "uploading"} onClick={() => { if (pendingPreviewUrl) URL.revokeObjectURL(pendingPreviewUrl); setPendingPreviewUrl(null); setPendingMedia(null); setMediaStage("idle"); }}><X aria-hidden="true" /></button>
        </div>}
        {matchedCommands.length > 0 && (
          <div className="v3-command-menu" role="listbox" aria-label="Commands">
            {matchedCommands.map((command) => (
              <button
                key={command.name}
                type="button"
                role="option"
                onClick={() => {
                  const result = runSlashCommand(command.name);
                  if (result === true) {
                    clearComposerAfterCommand();
                    return;
                  }
                  // A transform-type command (e.g. /shrug) needs its argument
                  // text - autocomplete the name and let Enter run it through
                  // the normal submit path instead of duplicating send logic here.
                  setComposerValue(`/${command.name} `);
                  document.getElementById("message-composer")?.focus();
                }}
              >
                <span className="v3-command-menu__name">/{command.name}</span>
                <span className="v3-command-menu__description">{command.description}</span>
              </button>
            ))}
          </div>
        )}
        <div className="composer-row">
          <input ref={attachmentInputRef} className="v3-file-input" type="file" accept="image/*,video/*" onChange={(event) => { void prepareAttachment(event.currentTarget.files?.[0]); event.currentTarget.value = ""; }} />
          <InputBar
            value={commandArgText}
            onChange={handleComposerArgChange}
            onSubmit={() => void handleComposerSubmit()}
            onPaste={handleComposerPaste}
            onKeyDown={handleComposerKeyDown}
            onAttach={() => attachmentInputRef.current?.click()}
            commandName={activeCommand?.name ?? null}
          />
        </div>
      </div>}
      <div className="temporarily-hidden" aria-hidden="true">
        <AvatarButton />
        <AvatarBadge />
        <AlertBar message="test alert" />
        <MiniBadge icon={CircleAlert} label="Show alert" onClick={showAlert} />
        {controls.slice(2).map(({ label, icon, image, text, className }) => (
          <ActionButton
            key={label}
            label={label}
            icon={icon}
            image={image}
            text={text}
            className={className}
          />
        ))}
        <SoundToggleButton />
      </div>

      <V3Lightbox />
    </main>
  );
}
