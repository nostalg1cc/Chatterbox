// Shared by the live call timer (VoiceCallButton) and the "Call lasted"
// chat history label (v3-shell) so both switch to hh:mm:ss at the same
// point instead of drifting out of sync with each other.
export function formatCallDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`
    : `${minutes}:${String(remainder).padStart(2, "0")}`;
}
