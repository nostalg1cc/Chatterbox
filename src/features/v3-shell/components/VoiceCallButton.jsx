import { Phone } from "lucide-react";
import { useEffect, useState } from "react";
import { ActionButton } from "./ActionButton";

function elapsedTime(startedAt, now) {
  if (!startedAt) return "0:00";
  const seconds = Math.max(0, Math.floor((now - new Date(startedAt).getTime()) / 1_000));
  return String(Math.floor(seconds / 60)) + ":" + String(seconds % 60).padStart(2, "0");
}

export function VoiceCallButton({ active, roomStartedAt, participantAvatars = [], participantCount = 0, hasParticipants = false, onJoin, onLeave }) {
  const [hovered, setHovered] = useState(false);
  const [now, setNow] = useState(Date.now());
  const occupied = hasParticipants || participantCount > 0 || participantAvatars.length > 0;
  const visibleParticipants = Math.min(2, Math.max(participantCount, participantAvatars.length));
  const leaving = active && hovered;
  const participantClass = visibleParticipants > 1 ? " is-two-participants" : " is-one-participant";

  useEffect(() => {
    if (!roomStartedAt) return;
    const id = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(id);
  }, [roomStartedAt]);

  const participants = <span className="voice-call-button__avatars">{participantAvatars.slice(0, 2).map((avatar, index) => <img key={avatar + "-" + index} src={avatar} alt="" />)}</span>;

  if (!active && !occupied) {
    return <ActionButton icon={Phone} label="Start voice call" text="Voice" className="voice-call-button" onClick={() => void onJoin?.()} />;
  }

  if (!active) {
    return <ActionButton icon={Phone} label="Join voice call" text="Join" className={"voice-call-button is-joinable" + participantClass} onClick={() => void onJoin?.()} />;
  }

  return (
    <ActionButton
      label={leaving ? "Leave voice call" : "Voice call active"}
      className={"voice-call-button icon-button--label is-connected" + participantClass + (leaving ? " is-leaving" : "")}
      onClick={() => void onLeave?.()}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
    >
      <span className="voice-call-button__presence">
        {participants}
        {leaving ? <span className="voice-call-button__leave-label">Leave</span> : <time className="voice-call-button__time">{elapsedTime(roomStartedAt, now)}</time>}
      </span>
    </ActionButton>
  );
}
