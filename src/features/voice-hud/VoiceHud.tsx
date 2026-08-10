import { useEffect, useState, type CSSProperties } from "react";
import { listen } from "@tauri-apps/api/event";
import { DecoratedText } from "@/components/decorated-text";
import type { VoiceHudParticipant, VoiceHudUpdate } from "@/lib/voice-hud";
import type { NameColor } from "@/lib/types";

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

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<VoiceHudUpdate>("voice-hud:update", (event) => {
      if (disposed) return;
      setParticipants(event.payload.participants);
      setScale(event.payload.scale);
      setShowNames(event.payload.showNames);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div className="voice-hud-viewport">
      <div className="voice-hud" style={{ transform: `scale(${scale / 100})`, transformOrigin: "left center" }}>
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
    <div className="voice-hud__participant">
      <span
        className={"voice-hud__avatar" + (participant.speaking ? " is-speaking" : "")}
        style={{ "--intensity": participant.speaking ? participant.level : 0 } as CSSProperties}
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
  );
}
