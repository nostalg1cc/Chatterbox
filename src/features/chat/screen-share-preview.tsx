import { useEffect, useRef } from "react";
import { Maximize2Icon, MonitorUpIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useVoice } from "@/stores/voice";

export function ScreenSharePreview() {
  const stream = useVoice((state) => state.remoteScreenStream);
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.srcObject = stream;
    if (stream) void video.play().catch(() => undefined);
    return () => {
      video.srcObject = null;
    };
  }, [stream]);

  if (!stream) return null;

  const openFullscreen = async () => {
    await videoRef.current?.requestFullscreen().catch(() => undefined);
  };

  return (
    <div className="group relative mx-[5px] aspect-video shrink-0 overflow-hidden rounded-[5px] border-[1.25px] border-solid border-white/[0.18] bg-black">
      <video
        ref={videoRef}
        autoPlay
        playsInline
        muted
        aria-label="Partner screen share"
        className="size-full object-contain"
      />
      <div className="pointer-events-none absolute inset-x-0 top-0 flex items-center gap-1.5 bg-gradient-to-b from-black/75 to-transparent px-2 py-1.5 text-[10px] text-white/85 opacity-0 transition-opacity group-hover:opacity-100">
        <MonitorUpIcon className="size-3" />
        Live screen
      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="View screen share fullscreen"
            className="absolute top-1.5 right-1.5 bg-black/55 text-white/80 opacity-0 hover:bg-black/75 hover:text-white group-hover:opacity-100"
            onClick={() => void openFullscreen()}
          >
            <Maximize2Icon />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Fullscreen</TooltipContent>
      </Tooltip>
    </div>
  );
}