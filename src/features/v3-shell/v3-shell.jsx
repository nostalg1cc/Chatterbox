import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ScreenSharePreview } from "@/features/chat/screen-share-preview";
import { SettingsDialog } from "@/features/settings/settings-dialog";
import { playAppSound } from "@/lib/app-sounds";
import { decorationUrl } from "@/lib/avatar-decorations";
import { supabase } from "@/lib/supabase";
import { isTauri } from "@/lib/tauri";
import { hideVoiceHud, resizeVoiceHud, showVoiceHud, updateVoiceHud } from "@/lib/voice-hud";
import { eventKeybind } from "@/lib/keybinds";
import { prepareChatMedia } from "@/lib/media";
import { toast } from "sonner";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useProfiles } from "@/stores/profiles";
import { usePresenceStatus } from "@/stores/presence";
import { usePreferences } from "@/stores/preferences";
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
import { V3Lightbox } from "./components/V3Lightbox";
import { useUiSounds } from "./hooks/useUiSounds";
import "./styles.css";

const EMPTY_MESSAGES = [];
const EMPTY_PARTICIPANTS = [];

const alertVariants = [
  { type: "info", message: "Test alert", icon: CircleAlert },
  { type: "success", message: "Changes saved", icon: CircleCheck },
  { type: "warning", message: "Connection unstable", icon: TriangleAlert },
  { type: "error", message: "Action needs attention", icon: CircleX },
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
  const [activeAlert, setActiveAlert] = useState(null);
  const [alertVariantIndex, setAlertVariantIndex] = useState(0);
  const [composerValue, setComposerValue] = useState("");
  const [pendingMedia, setPendingMedia] = useState(null);
  const [pendingPreviewUrl, setPendingPreviewUrl] = useState(null);
  const [mediaStage, setMediaStage] = useState("idle");
  const [hoveredDecorationHeaderId, setHoveredDecorationHeaderId] = useState(null);
  const [isSelfTyping, setIsSelfTyping] = useState(false);
  const userId = useAuth((state) => state.userId);
  const selfProfile = useAuth((state) => state.profile);
  const activeId = useChat((state) => state.activeId);
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
      if (!isDeletedChat || startsNewMessageGroup(message, liveMessages[index - 1])) flush(index);
      if (isDeletedChat && runStart === -1) runStart = index;
    });
    flush(liveMessages.length);
    return { counts, hidden };
  }, [liveMessages]);
  const hasActiveMessages = Boolean(activeId && Object.prototype.hasOwnProperty.call(messagesByConversation, activeId));
  const isInitialLoad = !loaded || (conversations.length > 0 && (!activeId || !hasActiveMessages));
  const uiSounds = useUiSounds();
  const messageHistoryRef = useRef(null);
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
  useEffect(() => {
    if (voiceStatus === "reconnecting" || voiceStatus === "failed") {
      if (lastVoiceHealthRef.current !== voiceStatus) playAppSound("voice_reconnect", true);
      lastVoiceHealthRef.current = voiceStatus;
    } else lastVoiceHealthRef.current = null;
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
  const dismissAlert = useCallback((id) => {
    setActiveAlert((currentAlert) => (currentAlert?.id === id ? null : currentAlert));
  }, []);

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
      toast.error(error instanceof Error ? error.message : "That attachment could not be prepared.");
    }
  }

  async function handleComposerSubmit() {
    if (!activeId || (!composerValue.trim() && !pendingMedia)) return;
    if (pendingMedia) setMediaStage("uploading");
    const sent = await useChat.getState().sendMessage(activeId, composerValue, pendingMedia, useChat.getState().replyTo?.id ?? null);
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
    const variant = alertVariants[alertVariantIndex];

    setActiveAlert({ id: `${Date.now()}-${Math.random()}`, ...variant });
    setAlertVariantIndex((currentIndex) => (currentIndex + 1) % alertVariants.length);
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
    prependRestoreRef.current = null;
  }, [liveMessages]);

  useEffect(() => {
    const history = messageHistoryRef.current;
    if (!history) return;
    const count = liveMessages.length;
    const previousCount = lastMessageCountRef.current;
    lastMessageCountRef.current = count;
    if (initialConversationRef.current !== activeId) {
      initialConversationRef.current = activeId;
      requestAnimationFrame(() => { history.scrollTop = history.scrollHeight; });
      return;
    }
    if (prependRestoreRef.current || count <= previousCount) return;
    const last = liveMessages[count - 1];
    const nearBottom = history.scrollHeight - history.scrollTop - history.clientHeight < 80;
    if (nearBottom || last?.sender_id === userId) requestAnimationFrame(() => { history.scrollTop = history.scrollHeight; });
  }, [activeId, liveMessages, userId]);

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
    <main className={"stage" + (joinedVoice ? " is-in-call" : "")} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); const file = [...event.dataTransfer.files].find((entry) => entry.type.startsWith("image/") || entry.type.startsWith("video/")); if (file) void prepareAttachment(file); }}>
      <div className={"v3-call-glow" + (joinedVoice ? " is-active" : "")} aria-hidden="true" />
      <div className="window-drag-region" data-tauri-drag-region aria-hidden="true" />
      {activeAlert && (
        <div className="top-alert-region" aria-live="polite" aria-atomic="true">
          <TopAlert
            id={activeAlert.id}
            message={activeAlert.message}
            type={activeAlert.type}
            icon={activeAlert.icon}
            isPrimary
            onDismiss={dismissAlert}
          />
        </div>
      )}

      <section ref={messageHistoryRef} className="message-history" aria-label="Chat messages" onScroll={(event) => { if (event.currentTarget.scrollTop < 96) void loadOlderMessages(); }}>
        <div className="message-list">
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
      </section>

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

      <nav className="top-navigation-controls" aria-label="Navigation controls">
        <SettingsDialog trigger={<ActionButton label="Settings" icon={Settings} />} />
      </nav>
      <div className="v3-screen-previews">
        <ScreenSharePreview source="remote" />
        <ScreenSharePreview source="local" />
      </div>

      <div className="bottom-composer">
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
        <div className="composer-row">
          <input ref={attachmentInputRef} className="v3-file-input" type="file" accept="image/*,video/*" onChange={(event) => { void prepareAttachment(event.currentTarget.files?.[0]); event.currentTarget.value = ""; }} />
          <InputBar value={composerValue} onChange={handleComposerChange} onSubmit={() => void handleComposerSubmit()} onPaste={handleComposerPaste} onAttach={() => attachmentInputRef.current?.click()} />
        </div>
      </div>
      <div className="temporarily-hidden" aria-hidden="true">
        <AvatarButton />
        <AvatarBadge />
        <AlertBar message="test alert" />
        <MiniBadge icon={CircleAlert} label="Show alert" onClick={showAlert} playClickSound={false} />
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
