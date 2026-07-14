import { cn } from "@/lib/utils";
import { useLayoutEffect, useMemo, useRef, type CSSProperties } from "react";

export type TextDecoration = "fuzzy" | "sparkles" | "resize" | "bouncy" | "wavy" | "gradient" | "glitch" | "particle";
export type NameFont = "sans" | "rounded" | "serif" | "mono";
export type NameWeight = "regular" | "medium" | "bold" | "black";

const fontClasses: Record<NameFont, string> = {
  sans: "name-font-sans",
  rounded: "name-font-rounded",
  serif: "name-font-serif",
  mono: "name-font-mono",
};

const weightClasses: Record<NameWeight, string> = {
  regular: "name-weight-regular",
  medium: "name-weight-medium",
  bold: "name-weight-bold",
  black: "name-weight-black",
};

type DecorationProps = {
  children: string;
  effect?: TextDecoration | null;
  font?: NameFont | null;
  weight?: NameWeight | null;
  active?: boolean;
  className?: string;
};

function Letters({ children, effect }: { children: string; effect: "bouncy" | "wavy" }) {
  return (
    <span className={"text-effect-letters text-effect-" + effect + "-letters"} aria-label={children}>
      {Array.from(children).map((letter, index) => (
        <span
          aria-hidden="true"
          className="text-effect-letter"
          key={letter + "-" + index}
          style={{ "--letter-index": index } as CSSProperties}
        >
          {letter === " " ? "\u00a0" : letter}
        </span>
      ))}
    </span>
  );
}

function Sparkles({ children }: { children: string }) {
  const sparkles = useMemo(
    () => Array.from({ length: 6 }, (_, index) => ({
      id: index,
      left: (8 + ((index * 31) % 82)) + "%",
      top: (4 + ((index * 47) % 91)) + "%",
      delay: (index * 0.37).toFixed(2) + "s",
      size: 5 + (index % 3) * 1.5,
    })),
    [],
  );

  return (
    <span className="text-effect-sparkles-wrap">
      {sparkles.map((sparkle) => (
        <svg
          className="text-effect-sparkle"
          key={sparkle.id}
          viewBox="0 0 200 200"
          aria-hidden="true"
          style={{
            left: sparkle.left,
            top: sparkle.top,
            width: sparkle.size,
            height: sparkle.size,
            animationDelay: sparkle.delay,
          }}
        >
          <path d="M120 80 100 0 80 80 0 100l80 20 20 80 20-80 80-20-80-20Z" />
        </svg>
      ))}
      <span className="text-effect-content">{children}</span>
    </span>
  );
}

function Particles({ children }: { children: string }) {
  const particles = useMemo(
    () => Array.from({ length: 9 }, (_, index) => ({
      id: index,
      left: (((index * 37) % 110) - 5) + "%",
      top: (((index * 53) % 120) - 10) + "%",
      delay: (index * 0.19).toFixed(2) + "s",
    })),
    [],
  );
  return (
    <span className="text-effect-particles-wrap">
      {particles.map((particle) => (
        <i
          className="text-effect-particle"
          key={particle.id}
          aria-hidden="true"
          style={{ left: particle.left, top: particle.top, animationDelay: particle.delay }}
        />
      ))}
      <span className="text-effect-content">{children}</span>
    </span>
  );
}

// Adapted from Sera UI's Fuzzy Text canvas renderer. It only runs for an active
// name, keeping message lists free from dozens of animation loops.
function FuzzyText({ children }: { children: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return;

    const style = getComputedStyle(canvas);
    const font = style.fontWeight + " " + style.fontSize + " " + style.fontFamily;
    const color = style.color;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const measure = document.createElement("canvas").getContext("2d");
    if (!measure) return;
    measure.font = font;
    const metrics = measure.measureText(children);
    const width = Math.max(1, Math.ceil(metrics.width + 8));
    const fallbackHeight = Number.parseFloat(style.fontSize) * 1.2;
    const height = Math.max(1, Math.ceil((metrics.actualBoundingBoxAscent + metrics.actualBoundingBoxDescent || fallbackHeight) + 6));
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = width + "px";
    canvas.style.height = height + "px";
    context.scale(dpr, dpr);

    const source = document.createElement("canvas");
    source.width = width;
    source.height = height;
    const sourceContext = source.getContext("2d", { willReadFrequently: true });
    if (!sourceContext) return;
    sourceContext.font = font;
    sourceContext.fillStyle = color;
    sourceContext.textBaseline = "middle";
    sourceContext.fillText(children, 4, height / 2);
    const sourcePixels = sourceContext.getImageData(0, 0, width, height);
    let frame = 0;
    const render = () => {
      const image = context.createImageData(width, height);
      for (let y = 0; y < height; y += 1) {
        const offset = Math.round((Math.random() - 0.5) * 2.4);
        for (let x = 0; x < width; x += 1) {
          const fromX = x + offset;
          if (fromX < 0 || fromX >= width) continue;
          const from = (y * width + fromX) * 4;
          const to = (y * width + x) * 4;
          image.data[to] = sourcePixels.data[from];
          image.data[to + 1] = sourcePixels.data[from + 1];
          image.data[to + 2] = sourcePixels.data[from + 2];
          image.data[to + 3] = sourcePixels.data[from + 3];
        }
      }
      context.clearRect(0, 0, width, height);
      context.putImageData(image, 0, 0);
      frame = window.requestAnimationFrame(render);
    };
    render();
    return () => window.cancelAnimationFrame(frame);
  }, [children]);

  return <canvas className="text-effect-fuzzy-canvas" ref={canvasRef} aria-label={children} />;
}

function ActiveEffect({ effect, children }: { effect: TextDecoration; children: string }) {
  switch (effect) {
    case "fuzzy":
      return <FuzzyText>{children}</FuzzyText>;
    case "sparkles":
      return <Sparkles>{children}</Sparkles>;
    case "resize":
      return <span className="text-effect-resize"><span>{children}</span><i /><i /><i /><i /></span>;
    case "bouncy":
    case "wavy":
      return <Letters effect={effect}>{children}</Letters>;
    case "gradient":
      return <span className="text-effect-gradient">{children}</span>;
    case "glitch":
      return <span className="text-effect-glitch" data-text={children}>{children}</span>;
    case "particle":
      return <Particles>{children}</Particles>;
  }
}

export function DecoratedText({ children, effect, font = "sans", weight = "medium", active: _active = true, className }: DecorationProps) {
  // Kept for source compatibility with existing callers; name effects are intentionally always-on.
  void _active;
  const safeFont = font && font in fontClasses ? font : "sans";
  const safeWeight = weight && weight in weightClasses ? weight : "medium";
  const canAnimate = Boolean(effect);

  return (
    <span
      className={cn(
        "text-decoration",
        fontClasses[safeFont],
        weightClasses[safeWeight],
        effect && "text-decoration-" + effect,
        canAnimate && "text-decoration-active",
        className,
      )}
    >
      {canAnimate && effect ? <ActiveEffect effect={effect} children={children} /> : children}
    </span>
  );
}