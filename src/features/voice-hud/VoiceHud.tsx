import { useEffect, useRef, useState, type CSSProperties } from "react";
import { listen } from "@tauri-apps/api/event";
import { motion, useReducedMotion } from "framer-motion";
import { MESSAGE_BODY_INITIAL, MESSAGE_BODY_ANIMATE, MESSAGE_BODY_TRANSITION } from "@/lib/message-motion";
import { DecoratedText } from "@/components/decorated-text";
import type { VoiceHudMessage, VoiceHudParticipant, VoiceHudUpdate } from "@/lib/voice-hud";
import type { NameColor } from "@/lib/types";
import type { VoiceHudFrame } from "@/lib/voice-hud-stream";

// Tailwind isn't loaded in this window's bundle (see vite.config.ts - it's a
// separate minimal entry), so name colors are resolved to real hex values
// instead of the Tailwind utility classes name-colors.ts normally returns.
const NAME_COLOR_HEX: Record<NameColor, string> = {
  default: "#f5f5f4",
  slate: "#cbd5e1",
  red: "#f87171",
  orange: "#fb923c",
  amber: "#fbbf24",
  green: "#34d399",
  cyan: "#22d3ee",
  blue: "#60a5fa",
  violet: "#a78bfa",
  pink: "#f472b6",
};

export function VoiceHud() {
  const [participants, setParticipants] = useState<VoiceHudParticipant[]>([]);
  const [scale, setScale] = useState(70);
  const [showNames, setShowNames] = useState(false);
  const lastUpdate = useRef(0);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<VoiceHudUpdate>("voice-hud:update", (event) => {
      if (disposed) return;
      lastUpdate.current = Date.now();
      setParticipants(event.payload.participants);
      setScale(event.payload.scale);
      setShowNames(event.payload.showNames);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    const watchdog = setInterval(() => {
      if (Date.now() - lastUpdate.current > 8_000) setParticipants((current) => current.length ? [] : current);
    }, 1_000);
    return () => {
      disposed = true;
      unlisten?.();
      clearInterval(watchdog);
    };
  }, []);

  return (
    <div className="voice-hud-viewport">
      <div className={"voice-hud" + (participants.some((participant) => participant.screenId || participant.message) ? " has-content" : "")} style={{ transform: `scale(${scale / 100})`, transformOrigin: "left center" }}>
        {participants.map((participant) => (
          <ParticipantRow key={participant.id} participant={participant} showName={showNames} />
        ))}
      </div>
    </div>
  );
}

function ParticipantRow({
  participant,
  showName,
}: {
  participant: VoiceHudParticipant;
  showName: boolean;
}) {
  return (
    <div className="voice-hud__participant-group">
    <div className="voice-hud__participant">
      <span
        className={
          "voice-hud__avatar" +
          (participant.muted ? " is-muted" : participant.speaking ? " is-speaking" : "")
        }
        style={{ "--intensity": participant.speaking && !participant.muted ? participant.level : 0 } as CSSProperties}
      >
        <span className="voice-hud__avatar-photo">
          {participant.avatar ? (
            <img src={participant.avatar} alt="" />
          ) : (
            <span className="voice-hud__fallback">{participant.name.slice(0, 1).toUpperCase()}</span>
          )}
        </span>
      </span>
      {showName && (
        <span
          className="voice-hud__name"
          style={{ color: NAME_COLOR_HEX[participant.nameColor ?? "default"] ?? NAME_COLOR_HEX.default }}
        >
          <DecoratedText effect={participant.nameDecoration} font={participant.nameFont} weight={participant.nameWeight}>
            {participant.name}
          </DecoratedText>
        </span>
      )}
    </div>
    {participant.message && <MessageBubble key={participant.message.id} message={participant.message} />}
    {participant.screenId && <StreamPreview key={participant.screenId} participantId={participant.id} streamId={participant.screenId} />}
    </div>
  );
}

function MessageBubble({ message }: { message: VoiceHudMessage }) {
  const [visible, setVisible] = useState(message.expiresAt > Date.now());
  const reducedMotion = useReducedMotion();
  useEffect(() => {
    const timer = setTimeout(() => setVisible(false), Math.max(0, message.expiresAt - Date.now()));
    return () => clearTimeout(timer);
  }, [message.expiresAt]);
  if (!visible) return null;
  return <motion.div className="voice-hud__message" role="status"
    initial={reducedMotion ? false : MESSAGE_BODY_INITIAL}
    animate={reducedMotion ? { opacity: 1, x: 0, scale: 1 } : MESSAGE_BODY_ANIMATE}
    transition={MESSAGE_BODY_TRANSITION}>{message.text}</motion.div>;
}

function StreamPreview({ participantId, streamId }: { participantId: string; streamId: string }) {
  const [image, setImage] = useState<string | null>(null);
  const receivedAt = useRef(0);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<VoiceHudFrame>("voice-hud:frame", ({ payload }) => {
      if (disposed || payload.participantId !== participantId || payload.streamId !== streamId || !payload.image.startsWith("data:image/jpeg;") || payload.image.length > 200_000) return;
      receivedAt.current = Date.now();
      setImage(payload.image);
    }).then((fn) => { if (disposed) fn(); else unlisten = fn; });
    const watchdog = setInterval(() => {
      if (Date.now() - receivedAt.current > 3_000) setImage(null);
    }, 1_000);
    return () => { disposed = true; unlisten?.(); clearInterval(watchdog); };
  }, [participantId, streamId]);
  return <div className="voice-hud__stream" aria-label="Live screen share">
    {image ? <img src={image} alt="Live screen share" /> : <span>Waiting for stream…</span>}
  </div>;
}
