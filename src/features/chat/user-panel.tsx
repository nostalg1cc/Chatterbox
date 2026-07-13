import {
  HeadphonesIcon,
  MicIcon,
  MicOffIcon,
  PhoneOffIcon,
  VolumeXIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UserAvatar } from "@/components/user-avatar";
import { SettingsDialog } from "@/features/settings/settings-dialog";
import { nameColorClass } from "@/lib/name-colors";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { usePresence } from "@/stores/presence";
import { useVoice } from "@/stores/voice";

export function UserPanel() {
  const profile = useAuth((state) => state.profile);
  const myId = useAuth((state) => state.userId);
  const online = usePresence((state) =>
    myId ? Boolean(state.online[myId]) : false
  );
  const muted = useVoice((state) => state.muted);
  const deafened = useVoice((state) => state.deafened);
  const voiceStatus = useVoice((state) => state.status);
  const inVoice = useVoice((state) => Boolean(state.activeConversationId));

  const voiceLabel =
    voiceStatus === "solo"
      ? "Voice - waiting"
      : voiceStatus === "reconnecting"
        ? "Voice - reconnecting"
        : voiceStatus === "failed"
          ? "Voice - connection failed"
          : "Voice connected";

  return (
    <div className="m-[5px] flex min-h-[47px] items-center gap-2 rounded-[5px] bg-transparent px-2.5 py-2">
      <UserAvatar profile={profile} online={online} animated />
      <div className="min-w-0 flex-1 leading-tight">
        <p
          className={cn(
            "truncate text-sm font-medium",
            nameColorClass(profile?.name_color)
          )}
        >
          {profile?.display_name ?? "..."}
        </p>
        <p
          className={cn(
            "truncate text-xs text-muted-foreground",
            inVoice && voiceStatus !== "failed" && "text-emerald-400/80",
            voiceStatus === "failed" && "text-destructive"
          )}
        >
          {inVoice ? voiceLabel : "@" + (profile?.username ?? "")}
        </p>
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        <PanelAction
          label={muted ? "Unmute" : "Mute"}
          pressed={muted}
          onClick={() => useVoice.getState().toggleMute()}
        >
          {muted ? <MicOffIcon /> : <MicIcon />}
        </PanelAction>
        <PanelAction
          label={deafened ? "Undeafen" : "Deafen"}
          pressed={deafened}
          onClick={() => useVoice.getState().toggleDeafen()}
        >
          {deafened ? <VolumeXIcon /> : <HeadphonesIcon />}
        </PanelAction>
        {inVoice && (
          <PanelAction
            label="Leave voice"
            pressed={false}
            danger
            onClick={() => void useVoice.getState().leave()}
          >
            <PhoneOffIcon />
          </PanelAction>
        )}
        <SettingsDialog />
      </div>
    </div>
  );
}

function PanelAction({
  label,
  pressed,
  danger = false,
  onClick,
  children,
}: {
  label: string;
  pressed: boolean;
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
            pressed &&
              "border-white/[0.35] bg-[#FF3333] text-white hover:bg-[#FF3333]/90 hover:text-white",
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

