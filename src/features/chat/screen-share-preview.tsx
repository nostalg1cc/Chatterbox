import { useEffect, useRef, useState } from "react";
import { Maximize2Icon, MonitorUpIcon, ShrinkIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useVoice } from "@/stores/voice";

export function ScreenSharePreview() {
  const stream = useVoice((state) => state.remoteScreenStream);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.srcObject = stream;
    if (stream) void video.play().catch(() => undefined);
    return () => { video.srcObject = null; };
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
      aria-label={expanded ? "Return to compact screen share" : "Expand screen share"}
      aria-expanded={expanded}
      className={expanded
        ? "group absolute top-[77px] right-[21px] bottom-[104px] left-[21px] z-40 overflow-hidden rounded-[10px] border-[1.25px] border-solid border-white/[0.18] bg-black shadow-2xl"
        : "group absolute top-[77px] right-[21px] z-20 w-[min(360px,42vw)] aspect-video overflow-hidden rounded-[5px] border-[1.25px] border-solid border-white/[0.18] bg-black shadow-xl"}
      onClick={() => setExpanded((value) => !value)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          setExpanded((value) => !value);
        }
      }}
    >
      <video ref={videoRef} autoPlay playsInline muted aria-label="Partner screen share" className="pointer-events-none size-full object-contain" />
      <div className="pointer-events-none absolute inset-x-0 top-0 flex items-center gap-1.5 bg-gradient-to-b from-black/75 to-transparent px-2 py-1.5 text-[10px] text-white/85 opacity-0 transition-opacity group-hover:opacity-100">
        <MonitorUpIcon className="size-3" />
        Live screen
        <span className="ml-auto">{expanded ? "Click to return to compact view" : "Click to expand"}</span>
      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="View screen share fullscreen"
            className={expanded ? "absolute top-2 right-2 bg-black/55 text-white/80 hover:bg-black/75 hover:text-white" : "absolute top-1.5 right-1.5 bg-black/55 text-white/80 opacity-0 hover:bg-black/75 hover:text-white group-hover:opacity-100"}
            onClick={(event) => void openFullscreen(event)}
          >
            <Maximize2Icon />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Fullscreen</TooltipContent>
      </Tooltip>
      {expanded && (
        <div className="pointer-events-none absolute right-2 bottom-2 rounded-md bg-black/60 px-2 py-1 text-[10px] text-white/80">
          <ShrinkIcon className="mr-1 inline size-3" />
          Click anywhere to return
        </div>
      )}
    </div>
  );
}