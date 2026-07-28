import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronDownIcon, HeadphoneOffIcon, HeadphonesIcon, MicIcon, MicOffIcon, MonitorUpIcon, MonitorXIcon, Volume2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverAnchor, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { UserAvatar } from "@/components/user-avatar";
import { WindowControls } from "@/components/titlebar";
import { ChatSwitcher } from "@/features/chat/chat-switcher";
import { Composer } from "@/features/chat/composer";
import { MessageList } from "@/features/chat/message-list";
import { ScreenSharePreview } from "@/features/chat/screen-share-preview";
import { SoundboardPopover } from "@/features/chat/soundboard-popover";
import { SettingsDialog } from "@/features/settings/settings-dialog";
import { appWindow, isTauri } from "@/lib/tauri";
import { playAppSound } from "@/lib/app-sounds";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useProfiles } from "@/stores/profiles";
import { useSoundboard } from "@/stores/soundboard";
import { usePreferences } from "@/stores/preferences";
import { formatVoiceElapsed, useVoice } from "@/stores/voice";

export function V2ChatView({ conversationId }: { conversationId: string }) {
  const myId = useAuth((state) => state.userId) ?? "";
  const ownProfile = useAuth((state) => state.profile);
  const conversation = useChat((state) =>
    state.conversations.find((entry) => entry.id === conversationId)
  );
  const friendId = conversation
    ? conversation.user1_id === myId
      ? conversation.user2_id
      : conversation.user1_id
    : undefined;
  const friend = useProfiles((state) =>
    friendId ? state.byId[friendId] : undefined
  );
  const activeVoiceId = useVoice((state) => state.activeConversationId);
  const room = useVoice((state) => state.rooms[conversationId]);
  const voiceStatus = useVoice((state) => state.status);
  const partnerInVoice = useVoice((state) => friendId ? (state.participants[conversationId] ?? []).some((participant) => participant.user_id === friendId) : false);
  const selfInVoiceElsewhere = useVoice((state) => (state.participants[conversationId] ?? []).some((participant) => participant.user_id === myId));
  const muted = useVoice((state) => state.muted);
  const deafened = useVoice((state) => state.deafened);
  const sharingScreen = useVoice((state) => state.sharingScreen);
  const composerRef = useRef<HTMLDivElement>(null);
  const [composerInset, setComposerInset] = useState(84);
  const isJoined = activeVoiceId === conversationId;
  const elapsed = useV2VoiceElapsed(room?.started_at);

  useEffect(
    () => useChat.getState().joinTyping(conversationId),
    [conversationId]
  );

  useEffect(() => {
    if (isJoined) void useSoundboard.getState().loadAvailable(conversationId);
  }, [conversationId, isJoined]);

  useEffect(() => {
    const element = composerRef.current;
    if (!element) return;
    const updateInset = () => setComposerInset(Math.ceil(element.getBoundingClientRect().height));
    updateInset();
    const observer = new ResizeObserver(updateInset);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const startWindowDrag = () => {
    if (isTauri) void appWindow().startDragging();
  };

  return (
    <div className={cn("conversation-canvas v2-canvas relative flex h-full min-h-0 flex-col", isJoined && "voice-active")}>
      <div
        aria-hidden="true"
        className="conversation-drag-rail absolute top-0 right-0 left-0 z-[59] h-[86px]"
        onMouseDown={(event) => {
          if (event.button === 0) startWindowDrag();
        }}
      />

      <div className="v2-floating-header absolute top-5 left-1/2 z-[60] flex -translate-x-1/2 items-center gap-2">
        <ChatSwitcher conversationId={conversationId} triggerClassName="v2-avatar-button" contentClassName="v2-control-menu v2-chat-switcher-menu" />
        <V2VoiceStatus
          joined={isJoined}
          partnerInVoice={partnerInVoice}
          takeover={selfInVoiceElsewhere}
          status={voiceStatus}
          elapsed={elapsed}
          onClick={() => void useVoice.getState().join(conversationId, selfInVoiceElsewhere)}
        />
        {isJoined && (
          <>
            <SoundboardPopover
              conversationId={conversationId}
              partnerName={friend?.display_name ?? "Partner"}
              triggerClassName="v2-icon-button"
              dropdownTrigger
            />
            <button
              type="button"
              className={cn("v2-icon-button", sharingScreen && "v2-stream-button-active")}
              aria-label={sharingScreen ? "Stop sharing screen" : "Share screen"}
              onPointerEnter={() => playAppSound("ui_hover")}
              onClick={() => {
                playAppSound("ui_click");
                if (sharingScreen) void useVoice.getState().stopScreenShare();
                else void useVoice.getState().startScreenShare();
              }}
            >
              {sharingScreen ? <MonitorXIcon /> : <MonitorUpIcon />}
            </button>
            <V2VoiceDropdown kind="mute" pressed={muted} disabled={false} />
            <V2VoiceDropdown kind="deafen" pressed={deafened} disabled={false} />
          </>
        )}
      </div>

      <div className="window-controls-reveal absolute top-[9px] right-[9px] z-[70] h-10 w-[108px]">
        <WindowControls />
      </div>

      <div className="relative min-h-0 flex-1">
        <MessageList conversationId={conversationId} topInset={72} bottomInset={composerInset} design="v2" />

        <div
          ref={composerRef}
          className="pointer-events-none absolute right-5 bottom-5 left-5 z-30 flex items-end justify-center gap-2"
        >
          <V2AccountButton profile={ownProfile} />
          <div className="pointer-events-auto w-full max-w-[640px]">
            <Composer
              conversationId={conversationId}
              placeholder={friend ? "Message @" + friend.username : "Message"}
              showAccount={false}
              className="v2-input-bar"
            />
          </div>
        </div>

        <ScreenSharePreview source="local" />
        <ScreenSharePreview source="remote" />
      </div>
    </div>
  );
}

function useV2VoiceElapsed(startedAt: string | undefined): string {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!startedAt) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [startedAt]);
  return formatVoiceElapsed(startedAt, now);
}

function V2VoiceStatus({
  joined,
  partnerInVoice,
  takeover,
  status,
  elapsed,
  onClick,
}: {
  joined: boolean;
  partnerInVoice: boolean;
  takeover: boolean;
  status: ReturnType<typeof useVoice.getState>["status"];
  elapsed: string;
  onClick: () => void;
}) {
  const label = joined
    ? status === "solo" ? `In voice \u00b7 ${elapsed}` : status === "connected" ? `Voice connected \u00b7 ${elapsed}` : `Voice ${status} \u00b7 ${elapsed}`
    : partnerInVoice ? `In voice \u00b7 Join \u00b7 ${elapsed}`
    : takeover ? "Voice active \u00b7 Take over"
    : "Voicechat";
  return (
    <button
      type="button"
      className={cn("v2-voice-status", (joined || partnerInVoice) && "is-active", joined && "is-joined")}
      aria-label={label}
      onPointerEnter={(event) => { if (event.pointerType === "mouse") playAppSound("ui_hover"); }}
      onClick={() => { playAppSound("ui_click"); if (joined) void useVoice.getState().leave(); else onClick(); }}
    >
      <HeadphonesIcon />
      <span>{label}</span>
    </button>
  );
}
function V2AccountButton({
  profile,
}: {
  profile: ReturnType<typeof useAuth.getState>["profile"];
}) {
  return (
    <Popover onOpenChange={(open) => playAppSound(open ? "ui_menu_open" : "ui_menu_close")}>
      <PopoverTrigger asChild>
        <button type="button" className="v2-account-button pointer-events-auto" aria-label="Your account and settings">
          <UserAvatar profile={profile} size="lg" className="v2-avatar-surface" animated />
        </button>
      </PopoverTrigger>
      <PopoverContent side="top" align="start" bare className="v2-control-menu v2-account-menu w-56 p-2.5">
        <div className="flex items-center gap-2 px-1 py-1.5">
          <UserAvatar profile={profile} animated />
          <div className="min-w-0">
            <p className="truncate text-sm font-medium">{profile?.display_name ?? "..."}</p>
            <p className="truncate text-xs text-muted-foreground">@{profile?.username ?? ""}</p>
          </div>
        </div>
        <div className="mt-1 flex flex-col gap-0.5 border-t border-white/[0.11] pt-2">
          <SettingsDialog buttonLabel="Settings" />
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start text-destructive hover:text-destructive"
            onClick={() => void useAuth.getState().signOut()}
          >
            Sign out
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function V2VoiceDropdown({
  kind,
  pressed,
  disabled,
}: {
  kind: "mute" | "deafen";
  pressed: boolean;
  disabled: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const isMute = kind === "mute";
  const icon = isMute
    ? (pressed ? <MicOffIcon /> : <MicIcon />)
    : (pressed ? <HeadphoneOffIcon /> : <HeadphonesIcon />);
  const label = isMute
    ? (pressed ? "Unmute microphone" : "Mute microphone")
    : (pressed ? "Undeafen audio" : "Deafen audio");
  const toggle = () => {
    const nextPressed = !pressed;
    playAppSound(isMute ? (nextPressed ? "mute_on" : "mute_off") : (nextPressed ? "deafen_on" : "deafen_off"));
    if (isMute) useVoice.getState().toggleMute();
    else useVoice.getState().toggleDeafen();
    setOpen(false);
  };
  const deviceKind = isMute ? "audioinput" : "audiooutput";
  const deviceLabel = isMute ? "Input device" : "Output device";
  const selectedDeviceId = usePreferences((state) => isMute ? state.inputDeviceId : state.outputDeviceId);
  const [loadingDevices, setLoadingDevices] = useState(false);
  const refreshDevices = useCallback(async () => {
    if (!navigator.mediaDevices?.enumerateDevices) {
      setDevices([]);
      return;
    }
    setLoadingDevices(true);
    try {
      let next = await navigator.mediaDevices.enumerateDevices();
      let scoped = next.filter((device) => device.kind === deviceKind && device.deviceId !== "default");

      // WebView2 commonly exposes only the default route or blank labels until an
      // audio permission grant has occurred. Voice is already joined here, but
      // this short probe also covers a fresh device change without persisting a stream.
      if ((scoped.length === 0 || scoped.some((device) => !device.label)) && navigator.mediaDevices.getUserMedia) {
        try {
          const probe = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
          for (const track of probe.getTracks()) track.stop();
          next = await navigator.mediaDevices.enumerateDevices();
          scoped = next.filter((device) => device.kind === deviceKind && device.deviceId !== "default");
        } catch {
          // Keep available anonymous entries; the fallback label below remains usable.
        }
      }
      setDevices(scoped);
    } catch {
      setDevices([]);
    } finally {
      setLoadingDevices(false);
    }
  }, [deviceKind]);

  useEffect(() => {
    if (!open) return;
    void refreshDevices();
    const onDeviceChange = () => void refreshDevices();
    navigator.mediaDevices?.addEventListener?.("devicechange", onDeviceChange);
    return () => navigator.mediaDevices?.removeEventListener?.("devicechange", onDeviceChange);
  }, [open, refreshDevices]);

  const setDevice = (deviceId: string) => {
    usePreferences.getState().setPreference(isMute ? "inputDeviceId" : "outputDeviceId", deviceId);
    playAppSound("ui_click");
  };

  return (
    <Popover open={open} onOpenChange={(next) => { setOpen(next); playAppSound(next ? "ui_menu_open" : "ui_menu_close"); }}>
      <PopoverAnchor asChild>
        <button
          type="button"
          aria-label={label}
          aria-pressed={pressed || undefined}
          aria-haspopup="menu"
          aria-expanded={open}
          disabled={disabled}
          className={cn("v2-audio-dropdown-button", pressed && "v2-icon-button-active")}
          onPointerEnter={(event) => { if (event.pointerType === "mouse") playAppSound("ui_hover"); }}
          onClick={(event) => {
            const bounds = event.currentTarget.getBoundingClientRect();
            if (event.clientX >= bounds.right - 22) {
              setOpen((current) => !current);
              return;
            }
            toggle();
          }}
        >
          {icon}
          <ChevronDownIcon className={cn("v2-dropdown-chevron", open && "is-open")} />
        </button>
      </PopoverAnchor>
      <PopoverContent side="bottom" align="start" sideOffset={10} bare className="v2-control-menu v2-device-menu w-60 p-2">
        <Button variant="ghost" size="sm" className="w-full justify-start" onClick={toggle}>
          {icon}
          {label}
        </Button>
        <div className="mt-1 border-t border-white/[0.11] pt-2">
          <p className="flex items-center gap-1.5 px-2 pb-1.5 text-[10px] font-semibold tracking-[0.1em] text-muted-foreground uppercase">
            {isMute ? <MicIcon /> : <Volume2Icon />}
            {deviceLabel}
          </p>
          <div className="max-h-44 space-y-0.5 overflow-y-auto pr-0.5">
            <button type="button" className={cn("v2-device-option", selectedDeviceId === "default" && "is-selected")} onClick={() => setDevice("default")}>Default {isMute ? "microphone" : "speakers"}</button>
            {loadingDevices ? (
              <p className="px-2 py-2 text-[11px] text-muted-foreground">Finding available devices...</p>
            ) : devices.length ? devices.map((device, index) => (
              <button key={device.deviceId} type="button" className={cn("v2-device-option", selectedDeviceId === device.deviceId && "is-selected")} onClick={() => setDevice(device.deviceId)}>
                {device.label || `${isMute ? "Microphone" : "Output"} ${index + 1}`}
              </button>
            )) : (
              <p className="px-2 py-2 text-[11px] text-muted-foreground">No additional {isMute ? "microphones" : "outputs"} detected.</p>
            )}
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}
