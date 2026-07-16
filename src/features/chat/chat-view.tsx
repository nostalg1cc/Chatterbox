import { useEffect, useRef, useState } from "react";
import {
  HeadphonesIcon,
  ImagesIcon,
  MonitorUpIcon,
  MonitorXIcon,
  MicIcon,
  MicOffIcon,
  VolumeXIcon,
  PhoneOffIcon,
  RefreshCwIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { WindowControls } from "@/components/titlebar";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { appWindow, isTauri } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useIsOnline } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";
import { useSoundboard } from "@/stores/soundboard";
import { formatVoiceElapsed, useVoice } from "@/stores/voice";
import { Composer } from "./composer";
import { ChatSwitcher } from "./chat-switcher";
import { MessageList } from "./message-list";
import { ScreenSharePreview } from "./screen-share-preview";
import { SoundboardPopover } from "./soundboard-popover";

export function ChatView({ conversationId }: { conversationId: string }) {
  const myId = useAuth((state) => state.userId) ?? "";
  const channel = useChat((state) => state.channel);
  const mediaOnly = channel === "media";
  const conv = useChat((state) =>
    state.conversations.find((conversation) => conversation.id === conversationId)
  );
  const friendId = conv
    ? conv.user1_id === myId
      ? conv.user2_id
      : conv.user1_id
    : undefined;
  const friend = useProfiles((state) =>
    friendId ? state.byId[friendId] : undefined
  );
  const online = useIsOnline(friendId);
  const typingUserId = useChat((state) => state.typing[conversationId]);
  const isTyping = Boolean(typingUserId && typingUserId === friendId);
  const room = useVoice((state) => state.rooms[conversationId]);
  const partnerInVoice = useVoice((state) =>
    friendId
      ? (state.participants[conversationId] ?? []).some(
          (participant) => participant.user_id === friendId
        )
      : false
  );
  const selfInVoiceElsewhere = useVoice((state) =>
    (state.participants[conversationId] ?? []).some(
      (participant) => participant.user_id === myId
    )
  );
  const activeVoiceId = useVoice((state) => state.activeConversationId);
  const voiceStatus = useVoice((state) => state.status);
  const sharingScreen = useVoice((state) => state.sharingScreen);
  const muted = useVoice((state) => state.muted);
  const deafened = useVoice((state) => state.deafened);
  const isJoined = activeVoiceId === conversationId;
  const elapsed = useVoiceElapsed(room?.started_at);
  const composerOverlayRef = useRef<HTMLDivElement>(null);
  const [composerInset, setComposerInset] = useState(80);

  useEffect(
    () => useChat.getState().joinTyping(conversationId),
    [conversationId]
  );

  useEffect(() => {
    if (isJoined) void useSoundboard.getState().loadAvailable(conversationId);
  }, [conversationId, isJoined]);

  useEffect(() => {
    const onFocus = () => useChat.getState().markRead(conversationId);
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [conversationId]);

  useEffect(() => {
    const overlay = composerOverlayRef.current;
    if (!overlay) return;

    const updateInset = () => {
      setComposerInset(Math.ceil(overlay.getBoundingClientRect().height));
    };
    updateInset();

    const observer = new ResizeObserver(updateInset);
    observer.observe(overlay);
    return () => observer.disconnect();
  }, [mediaOnly]);

  const startWindowDrag = () => {
    if (isTauri) void appWindow().startDragging();
  };

  const presenceLabel = voicePresenceLabel({
    isJoined,
    partnerInVoice,
    selfInVoiceElsewhere,
    status: voiceStatus,
    elapsed,
    typing: isTyping,
    online,
  });

  return (
    <div className={cn("conversation-canvas relative flex h-full min-h-0 flex-col", isJoined && "voice-active")}>
      <div aria-hidden="true" className="conversation-drag-rail absolute top-[21px] right-[124px] left-0 z-[59] h-[76px]" onMouseDown={(event) => { if (event.button === 0) startWindowDrag(); }} />
      <header
        className="surface-panel floating-surface conversation-dock isolate absolute top-[21px] left-1/2 z-[60] flex h-11 w-max max-w-[calc(100%-116px)] -translate-x-1/2 items-center gap-1 px-[3px] py-1 shadow-[0_12px_28px_rgb(0_0_0_/_0.28)]"
        onMouseDown={(event) => {
          if (!isTauri || event.button !== 0) return;
          const target = event.target as HTMLElement;
          if (target.closest("button, input, textarea, select, a, [role=button]")) return;
          startWindowDrag();
        }}
      >
        <div aria-hidden="true" className="conversation-drag-shield absolute inset-0 z-0 pointer-events-auto" />
        <div className="relative z-10"><ChatSwitcher conversationId={conversationId} /></div>
        {(mediaOnly || isTyping || partnerInVoice || isJoined || selfInVoiceElsewhere) && (
          <p className={cn("dock-status relative z-10 h-9 w-[220px] shrink-0 truncate px-2 text-center text-xs leading-9 tabular-nums text-muted-foreground", (partnerInVoice || isJoined) && "text-foreground/75", voiceStatus === "failed" && isJoined && "text-destructive")}>
            {mediaOnly ? "Media - Shared images and videos" : presenceLabel}
          </p>
        )}

        <div className="dock-actions relative z-10 flex h-9 shrink-0 items-center gap-1">
          {!isJoined ? (
            <VoiceButton
              label={
                partnerInVoice
                  ? "Join voice"
                  : selfInVoiceElsewhere
                    ? "Take over voice"
                    : "Voicechat"
              }
              tone={partnerInVoice ? "join" : "neutral"}
              onClick={() => void useVoice.getState().join(conversationId, selfInVoiceElsewhere)}
            />
          ) : (
            <>
              <CallAction label={muted ? "Unmute" : "Mute"} pressed={muted} danger={muted} onClick={() => useVoice.getState().toggleMute()}>
                {muted ? <MicOffIcon /> : <MicIcon />}
              </CallAction>
              <CallAction label={deafened ? "Undeafen" : "Deafen"} pressed={deafened} danger={deafened} onClick={() => useVoice.getState().toggleDeafen()}>
                {deafened ? <VolumeXIcon /> : <HeadphonesIcon />}
              </CallAction>
              <SoundboardPopover
                conversationId={conversationId}
                partnerName={friend?.display_name ?? "Partner"}
              />
              {voiceStatus === "failed" && (
                <CallAction
                  label="Retry direct connection"
                  onClick={() => useVoice.getState().retryConnection()}
                >
                  <RefreshCwIcon />
                </CallAction>
              )}
              <CallAction
                label={sharingScreen ? "Stop sharing screen" : "Share screen"}
                pressed={sharingScreen}
                onClick={() =>
                  sharingScreen
                    ? void useVoice.getState().stopScreenShare()
                    : void useVoice.getState().startScreenShare()
                }
              >
                {sharingScreen ? <MonitorXIcon /> : <MonitorUpIcon />}
              </CallAction>
              <VoiceButton
                label="Leave voice"
                tone="leave"
                onClick={() => void useVoice.getState().leave()}
              />
            </>
          )}
        </div>

      </header>
      <div className="window-controls-reveal absolute top-[9px] right-[9px] z-40 h-10 w-[108px]"><WindowControls /></div>

      <div className="relative min-h-0 flex-1">
        <MessageList
          conversationId={conversationId}
          bottomInset={mediaOnly ? 0 : composerInset}
          topInset={82}
          mediaOnly={mediaOnly}
        />

        {!mediaOnly && <div
          ref={composerOverlayRef}
          className="pointer-events-none absolute right-[21px] bottom-[21px] left-[21px] z-30"

        >
          <div className="h-5 px-2 text-xs text-muted-foreground">
            {isTyping && (
              <span>{friend?.display_name ?? "Friend"} is typing...</span>
            )}
          </div>

          <div className="pointer-events-auto mx-auto w-full max-w-[640px]">
            <Composer
              conversationId={conversationId}
              placeholder={
                friend ? "Message @" + friend.username : "Message"
              }
            />
          </div>
        </div>}
        {mediaOnly && (
          <div className="pointer-events-none absolute top-[78px] right-[21px] z-10 flex items-center gap-1.5 rounded-md border border-white/[0.13] bg-background/90 px-2.5 py-1.5 text-[11px] text-muted-foreground shadow-sm">
            <ImagesIcon className="size-3.5" />
            Media channel
          </div>
        )}
        <ScreenSharePreview />
      </div>
    </div>
  );
}

function voicePresenceLabel({
  isJoined,
  partnerInVoice,
  selfInVoiceElsewhere,
  status,
  elapsed,
  typing,
  online,
}: {
  isJoined: boolean;
  partnerInVoice: boolean;
  selfInVoiceElsewhere: boolean;
  status: ReturnType<typeof useVoice.getState>["status"];
  elapsed: string;
  typing: boolean;
  online: boolean;
}): string {
  if (isJoined) {
    if (status === "solo") return "In voice - waiting for partner - " + elapsed;
    if (status === "connected") return "Voice connected - " + elapsed;
    if (status === "reconnecting") return "Voice reconnecting - " + elapsed;
    if (status === "failed") return "Direct connection failed - " + elapsed;
    if (status === "joining") return "Joining voice...";
    return "Connecting voice - " + elapsed;
  }
  if (partnerInVoice) return "In voice - join anytime - " + elapsed;
  if (selfInVoiceElsewhere) return "Voice active on another device";
  if (typing) return "typing...";
  return online ? "Online" : "Offline";
}

function useVoiceElapsed(startedAt: string | undefined): string {
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (!startedAt) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [startedAt]);

  return formatVoiceElapsed(startedAt, now);
}

function VoiceButton({
  label,
  tone,
  onClick,
}: {
  label: string;
  tone: "neutral" | "join" | "leave";
  onClick: () => void;
}) {
  const styles = {
    neutral: "border-transparent bg-transparent text-foreground/72 hover:bg-white/[0.07] hover:text-foreground",
    join: "border-emerald-400/30 bg-emerald-500/18 text-emerald-100 hover:bg-emerald-500/28 hover:text-white",
    leave: "border-destructive/35 bg-destructive/18 text-red-100 hover:bg-destructive/28 hover:text-white",
  }[tone];

  return (
    <Button
      variant="ghost"
      size="sm"
      className={cn("dock-join h-9 gap-1.5 border px-2.5 text-[11px] font-medium [&_svg]:size-3.5", styles)}
      onClick={onClick}
    >
      {tone === "leave" ? <PhoneOffIcon /> : <HeadphonesIcon />}
      {label}
    </Button>
  );
}
function CallAction({
  label,
  pressed = false,
  danger = false,
  onClick,
  children,
}: {
  label: string;
  pressed?: boolean;
  danger?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          aria-pressed={pressed || undefined}
          className={cn(
            "header-action text-muted-foreground",
            pressed && (danger ? "bg-destructive/18 text-red-100 hover:bg-destructive/28 hover:text-red-50" : "bg-muted text-foreground"),
            danger && !pressed && "text-destructive hover:bg-destructive/15 hover:text-destructive"
          )}
          onClick={onClick}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
