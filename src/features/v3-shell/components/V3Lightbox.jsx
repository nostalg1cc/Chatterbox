import { useEffect, useState } from "react";
import { useLightbox } from "@/stores/lightbox";

const MIN_SCALE = 1;
const MAX_SCALE = 5;
const ZOOM_STEP = 0.0015;

export function V3Lightbox() {
  const src = useLightbox((state) => state.src);
  const hide = useLightbox((state) => state.hide);
  const [scale, setScale] = useState(1);

  useEffect(() => {
    setScale(1);
  }, [src]);

  useEffect(() => {
    if (!src) return;
    const onKeyDown = (event) => {
      if (event.key === "Escape") hide();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [src, hide]);

  if (!src) return null;

  const onWheel = (event) => {
    event.preventDefault();
    setScale((current) => {
      const next = current - event.deltaY * ZOOM_STEP;
      return Math.min(MAX_SCALE, Math.max(MIN_SCALE, next));
    });
  };

  return (
    <div className="v3-lightbox" onClick={hide} onWheel={onWheel}>
      <div className="v3-lightbox__drag-region" data-tauri-drag-region aria-hidden="true" />
      <img
        className="v3-lightbox__media"
        src={src}
        alt=""
        referrerPolicy="no-referrer"
        style={{ transform: `scale(${scale})` }}
      />
    </div>
  );
}
