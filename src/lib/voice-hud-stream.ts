import { emitTo } from "@tauri-apps/api/event";
import { isTauri } from "@/lib/tauri";
import { shouldFillScreenPreview } from "@/lib/screen-share-layout";

export interface VoiceHudFrame {
  participantId: string;
  streamId: string;
  image: string;
}

/** A bounded, video-only local preview relay between the two webviews.
 * MediaStream cannot be serialized through Tauri events. No extra capture,
 * server upload, audio playback, or second network stream is involved. */
export function relayVoiceHudStream(participantId: string, stream: MediaStream): () => void {
  if (!isTauri) return () => undefined;
  const video = document.createElement("video");
  video.muted = true;
  video.playsInline = true;
  video.srcObject = new MediaStream(stream.getVideoTracks());
  const canvas = document.createElement("canvas");
  canvas.width = 320;
  canvas.height = 180;
  const context = canvas.getContext("2d", { alpha: false });
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let lastTime = -1;

  const pump = async () => {
    if (disposed) return;
    const track = stream.getVideoTracks()[0];
    if (context && track?.readyState === "live" && !track.muted && video.readyState >= 2 && video.currentTime !== lastTime) {
      lastTime = video.currentTime;
      try {
        // Crop wide captures to fill; keep other aspect ratios undistorted.
        const scale = shouldFillScreenPreview(video.videoWidth, video.videoHeight) ? Math.max : Math.min;
        const ratio = scale(canvas.width / video.videoWidth, canvas.height / video.videoHeight);
        const width = video.videoWidth * ratio;
        const height = video.videoHeight * ratio;
        context.fillStyle = "#000";
        context.fillRect(0, 0, canvas.width, canvas.height);
        context.drawImage(video, (canvas.width - width) / 2, (canvas.height - height) / 2, width, height);
        const frame: VoiceHudFrame = { participantId, streamId: stream.id, image: canvas.toDataURL("image/jpeg", 0.65) };
        await emitTo("voice-hud", "voice-hud:frame", frame);
      } catch {
        // The overlay falls back to its waiting state if frames stop arriving.
      }
    }
    // One in-flight event per source, at most eight small frames per second.
    if (!disposed) timer = setTimeout(() => void pump(), 125);
  };
  void video.play().catch(() => undefined);
  void pump();
  return () => {
    disposed = true;
    if (timer !== undefined) clearTimeout(timer);
    video.pause();
    video.srcObject = null;
    // The call owns the tracks; detaching this preview must never stop them.
  };
}
