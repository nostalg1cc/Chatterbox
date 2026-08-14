import { useEffect, useRef, useState } from "react";
import { supabase } from "@/lib/supabase";
import { getCachedMedia, putCachedMedia } from "@/lib/media-cache";
import { remoteMediaUrl } from "@/lib/media";
import { useAuth } from "@/stores/auth";
import { usePreferences } from "@/stores/preferences";
import { useLightbox } from "@/stores/lightbox";
import { configureMediaOutput } from "@/lib/voice-media";

export function V3MediaAttachment({ message }) {
  const userId = useAuth((state) => state.userId);
  const [url, setUrl] = useState(null);
  const [local, setLocal] = useState(false);
  const [failed, setFailed] = useState(false);
  const videoRef = useRef(null);
  const outputDeviceId = usePreferences((state) => state.outputDeviceId);
  const mediaVolume = usePreferences((state) => state.mediaVolume);
  const outputVolume = usePreferences((state) => state.outputVolume);
  const setPreference = usePreferences((state) => state.setPreference);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || message.media_kind !== "video") return;
    video.defaultMuted = false;
    void configureMediaOutput(video, {
      volume: (mediaVolume * outputVolume) / 100,
      outputDeviceId,
      muted: false,
    });
  }, [mediaVolume, message.media_kind, outputDeviceId, outputVolume, url]);

  useEffect(() => {
    if (!userId) return undefined;
    let disposed = false;
    let objectUrl = null;
    const abortController = new AbortController();
    setUrl(null);
    setLocal(false);
    setFailed(false);

    const load = async () => {
      try {
        const cached = await getCachedMedia(userId, message.id);
        if (disposed) return;
        if (cached) {
          objectUrl = URL.createObjectURL(cached.blob);
          setUrl(objectUrl);
          setLocal(true);
          return;
        }
        if (!message.media_path) {
          setFailed(true);
          return;
        }
        const cloudinaryUrl = remoteMediaUrl(message.media_path);
        const signed = cloudinaryUrl ? null : await supabase.storage.from("chat-media").createSignedUrl(message.media_path, 60 * 60);
        if (disposed) return;
        const remoteUrl = cloudinaryUrl ?? signed?.data?.signedUrl;
        if (!remoteUrl) {
          setFailed(true);
          return;
        }
        setUrl(remoteUrl);
        void fetch(remoteUrl, { signal: abortController.signal })
          .then(async (response) => {
            if (!response.ok) return;
            const blob = await response.blob();
            await putCachedMedia({ userId, messageId: message.id, blob, mimeType: message.media_mime_type ?? blob.type, createdAt: message.created_at });
          })
          .catch(() => undefined);
      } catch {
        if (!disposed) setFailed(true);
      }
    };
    void load();
    return () => {
      disposed = true;
      abortController.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [message.created_at, message.id, message.media_mime_type, message.media_path, userId]);

  if (failed) return <span className="v3-media-unavailable">Media is unavailable</span>;
  if (!url) return <span className="v3-media-loading">Loading media…</span>;
  return (
    <div className="v3-media-wrap">
      {message.media_kind === "video" ? (
        <video ref={videoRef} className="v3-media" src={url} controls playsInline preload="metadata" onVolumeChange={(event) => {
          const nextVolume = Math.round((event.currentTarget.volume * 10000) / Math.max(1, outputVolume));
          if (usePreferences.getState().mediaVolume !== nextVolume) setPreference("mediaVolume", nextVolume);
        }} />
      ) : (
        <img
          className="v3-media"
          src={url}
          alt="Chat attachment"
          loading="lazy"
          onClick={() => useLightbox.getState().show(url)}
        />
      )}
      {local && message.media_deleted_at && <span className="v3-media-saved">Saved locally</span>}
    </div>
  );
}
