import { useEffect, useRef, useState } from "react";
import {
  HeadphonesIcon,
  ImagesIcon,
  MonitorUpIcon,
  MonitorXIcon,
  PhoneOffIcon,
  RefreshCwIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { WindowControls } from "@/components/titlebar";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UserAvatar } from "@/components/user-avatar";
import { nameColorClass } from "@/lib/name-colors";
import { appWindow, isTauri } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useIsOnline } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";
import { formatVoiceElapsed, useVoice } from "@/stores/voice";
import { Composer } from "./composer";
import { MessageList } from "./message-list";
import { SoundboardPopover } from "./soundboard-popover";

const COMPOSER_GRADIENT_STYLE = {
  background: "linear-gradient(to top, var(--background), transparent)",
} as const;

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
  const isJoined = activeVoiceId === conversationId;
  const elapsed = useVoiceElapsed(room?.started_at);
  const composerOverlayRef = useRef<HTMLDivElement>(null);
  const [composerInset, setComposerInset] = useState(80);

  useEffect(
    () => useChat.getState().joinTyping(conversationId),
    [conversationId]
  );

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
    <div className="flex h-full min-h-0 flex-col">
      <header
        className="flex h-12 shrink-0 items-center gap-2.5 overflow-hidden border-b border-white/[0.13] pl-4 pr-0"
        onMouseDown={(event) => {
          if (!isTauri || event.button !== 0) return;
          const target = event.target as HTMLElement;
          if (target.closest("button, input, textarea, select, a, [role=button]")) return;
          void appWindow().startDragging();
        }}
      >
        <div className="flex min-w-0 flex-1 items-center gap-2.5">
          <UserAvatar profile={friend} online={online} />
          <div className="min-w-0 leading-tight">
            <p
              className={cn(
                "truncate text-sm font-medium",
                nameColorClass(friend?.name_color)
              )}
            >
              {friend?.display_name ?? "..."}
            </p>
            <p
              className={cn(
                "truncate text-xs text-muted-foreground",
                (partnerInVoice || isJoined) && "text-foreground/75",
                voiceStatus === "failed" && isJoined && "text-destructive"
              )}
            >
              {mediaOnly ? "Media - Shared images and videos" : presenceLabel}
            </p>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {!isJoined ? (
            partnerInVoice ? (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void useVoice.getState().join(conversationId)}
              >
                <HeadphonesIcon />
                Join voice
              </Button>
            ) : (
              <CallAction
                label={
                  selfInVoiceElsewhere
                    ? "Take over voice on this device"
                    : "Join voice channel"
                }
                onClick={() => void useVoice.getState().join(conversationId)}
              >
                <HeadphonesIcon />
              </CallAction>
            )
          ) : (
            <>
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
              <CallAction
                label="Leave voice"
                danger
                onClick={() => void useVoice.getState().leave()}
              >
                <PhoneOffIcon />
              </CallAction>
            </>
          )}
        </div>
        <WindowControls />
      </header>

      <div className="relative min-h-0 flex-1">
        <MessageList
          conversationId={conversationId}
          bottomInset={mediaOnly ? 0 : composerInset}
          mediaOnly={mediaOnly}
        />

        {!mediaOnly && <div
          ref={composerOverlayRef}
          className="pointer-events-none absolute inset-x-0 bottom-0 z-10"
          style={COMPOSER_GRADIENT_STYLE}
        >
          <div className="h-5 px-4 text-xs text-muted-foreground">
            {isTyping && (
              <span>{friend?.display_name ?? "Friend"} is typing...</span>
            )}
          </div>

          <div className="pointer-events-auto">
            <Composer
              conversationId={conversationId}
              placeholder={
                friend ? "Message @" + friend.username : "Message"
              }
            />
          </div>
        </div>}
        {mediaOnly && (
          <div className="pointer-events-none absolute top-3 right-3 z-10 flex items-center gap-1.5 rounded-md border border-white/[0.13] bg-background/90 px-2.5 py-1.5 text-[11px] text-muted-foreground shadow-sm">
            <ImagesIcon className="size-3.5" />
            Media channel
          </div>
        )}
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
            "text-muted-foreground",
            pressed && "bg-muted text-foreground",
            danger &&
              "text-destructive hover:bg-destructive/15 hover:text-destructive"
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
