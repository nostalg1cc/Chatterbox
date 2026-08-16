import { useEffect, useRef, useState } from "react";
import { Maximize2Icon, MonitorUpIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useVoice } from "@/stores/voice";
import { usePreferences } from "@/stores/preferences";
import { configureMediaOutput } from "@/lib/voice-media";

type ScreenShareSource = "local" | "remote";

export function ScreenSharePreview({ source = "remote" }: { source?: ScreenShareSource }) {
  const stream = useVoice((state) =>
    source === "local" ? state.localScreenStream : state.remoteScreenStream
  );
  const videoRef = useRef<HTMLVideoElement>(null);
  const [expanded, setExpanded] = useState(false);
  const isLocal = source === "local";
  const label = isLocal ? "Your screen" : "Partner screen";
  const outputDeviceId = usePreferences((state) => state.outputDeviceId);
  const mediaVolume = usePreferences((state) => state.mediaVolume);
  const outputVolume = usePreferences((state) => state.outputVolume);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.srcObject = stream;
    // muted must be settled before play() is ever called, not after - an
    // unmuted autoplay() request is what browsers actually gate behind a
    // user gesture, so setting it in a later effect (after play() already
    // ran unmuted-by-default) can get the very first play silently blocked.
    // isLocal stays muted regardless - playing your own captured system
    // audio back through your own speakers is an instant feedback loop,
    // since the OS is already outputting it.
    video.muted = isLocal;
    if (stream) void video.play().catch(() => undefined);
    return () => {
      video.srcObject = null;
    };
  }, [isLocal, stream]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !stream) return;
    // Same output routing as everything else in the app (voice, soundboard,
    // chat media) - the remote side is the one that actually needs to be
    // heard, so it follows the user's chosen output device/volume.
    void configureMediaOutput(video, {
      volume: isLocal ? 0 : (mediaVolume * outputVolume) / 100,
      outputDeviceId,
      muted: isLocal,
    });
  }, [isLocal, mediaVolume, outputDeviceId, outputVolume, stream]);

  useEffect(() => {
    if (!stream) setExpanded(false);
  }, [stream]);

  if (!stream) return null;

  const openFullscreen = async (event: React.MouseEvent) => {
    event.stopPropagation();
    await videoRef.current?.requestFullscreen().catch(() => undefined);
  };

  return (
    <div
      role="button"
      tabIndex={0}
      aria-label={expanded ? `Return ${label.toLowerCase()} to compact view` : `Expand ${label.toLowerCase()}`}
      aria-expanded={expanded}
      className={
        expanded
          ? "group absolute top-[77px] right-[21px] bottom-[104px] left-[21px] z-40 overflow-hidden rounded-[10px] border-[1.25px] border-solid border-white/[0.18] bg-black shadow-2xl"
          : "group absolute top-[77px] " +
            (isLocal ? "left-[21px]" : "right-[21px]") +
            " z-20 w-[min(360px,42vw)] aspect-video overflow-hidden rounded-[5px] border-[1.25px] border-solid border-white/[0.18] bg-black shadow-xl"
      }
      onClick={() => setExpanded((value) => !value)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          setExpanded((value) => !value);
        }
      }}
    >
      <video
        ref={videoRef}
        autoPlay
        playsInline
        aria-label={label}
        className="pointer-events-none size-full object-contain"
      />
      <div className="pointer-events-none absolute inset-x-0 top-0 flex items-center gap-1.5 bg-gradient-to-b from-black/75 to-transparent px-2 py-1.5 text-[10px] text-white/85 opacity-0 transition-opacity group-hover:opacity-100">
        <MonitorUpIcon className="size-3" />
        {label}

      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`View ${label.toLowerCase()} fullscreen`}
            className={
              expanded
                ? "absolute top-2 right-2 bg-black/55 text-white/80 hover:bg-black/75 hover:text-white"
                : "absolute top-1.5 right-1.5 bg-black/55 text-white/80 opacity-0 hover:bg-black/75 hover:text-white group-hover:opacity-100"
            }
            onClick={(event) => void openFullscreen(event)}
          >
            <Maximize2Icon />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Fullscreen</TooltipContent>
      </Tooltip>

    </div>
  );
}
