import { useEffect, useRef, useState } from "react";
import { useLightbox } from "@/stores/lightbox";

const MIN_SCALE = 1;
const MAX_SCALE = 5;
const ZOOM_STEP = 0.0015;
const CLICK_DRAG_THRESHOLD = 4;

// How big the image renders at scale 1 (object-fit:contain against the
// current container), needed to know how far it can be panned before its
// edge would pull in from the container edge and leave empty space.
function containedSize(containerWidth, containerHeight, naturalWidth, naturalHeight) {
  if (!naturalWidth || !naturalHeight) return { width: containerWidth, height: containerHeight };
  const containerRatio = containerWidth / containerHeight;
  const naturalRatio = naturalWidth / naturalHeight;
  return naturalRatio > containerRatio
    ? { width: containerWidth, height: containerWidth / naturalRatio }
    : { width: containerHeight * naturalRatio, height: containerHeight };
}

function clampOffset(offset, scale, container, natural) {
  if (!container) return offset;
  const { width, height } = containedSize(container.width, container.height, natural.width, natural.height);
  const maxX = Math.max(0, (width * scale - container.width) / 2);
  const maxY = Math.max(0, (height * scale - container.height) / 2);
  return {
    x: Math.min(maxX, Math.max(-maxX, offset.x)),
    y: Math.min(maxY, Math.max(-maxY, offset.y)),
  };
}

export function V3Lightbox() {
  const src = useLightbox((state) => state.src);
  const hide = useLightbox((state) => state.hide);
  const containerRef = useRef(null);
  const naturalRef = useRef({ width: 0, height: 0 });
  const dragRef = useRef(null);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    setScale(1);
    setOffset({ x: 0, y: 0 });
    naturalRef.current = { width: 0, height: 0 };
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

  const containerRect = () => containerRef.current?.getBoundingClientRect();

  const onWheel = (event) => {
    event.preventDefault();
    setScale((current) => {
      const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, current - event.deltaY * ZOOM_STEP));
      setOffset((currentOffset) => clampOffset(currentOffset, next, containerRect(), naturalRef.current));
      return next;
    });
  };

  const onImageLoad = (event) => {
    naturalRef.current = { width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight };
  };

  const onMouseDown = (event) => {
    if (event.button !== 0) return;
    dragRef.current = { startX: event.clientX, startY: event.clientY, originX: offset.x, originY: offset.y, moved: false };
    setDragging(true);
  };

  const onMouseMove = (event) => {
    const drag = dragRef.current;
    if (!drag) return;
    const dx = event.clientX - drag.startX;
    const dy = event.clientY - drag.startY;
    if (!drag.moved && Math.hypot(dx, dy) > CLICK_DRAG_THRESHOLD) drag.moved = true;
    if (scale <= MIN_SCALE) return;
    setOffset(clampOffset({ x: drag.originX + dx, y: drag.originY + dy }, scale, containerRect(), naturalRef.current));
  };

  const endDrag = () => {
    setDragging(false);
    // Don't clear dragRef here - the click event fires right after mouseup,
    // and onClick below needs to still see whether this gesture moved.
  };

  const onClick = () => {
    // A drag that actually moved the image shouldn't also close the
    // lightbox - only a genuine click (no meaningful movement) does.
    const wasDrag = dragRef.current?.moved;
    dragRef.current = null;
    if (wasDrag) return;
    hide();
  };

  const cursor = scale > MIN_SCALE ? (dragging ? "grabbing" : "grab") : "zoom-out";

  return (
    <div
      ref={containerRef}
      className="v3-lightbox"
      style={{ cursor }}
      onClick={onClick}
      onWheel={onWheel}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={endDrag}
      onMouseLeave={endDrag}
    >
      <div className="v3-lightbox__drag-region" data-tauri-drag-region aria-hidden="true" />
      <img
        className="v3-lightbox__media"
        src={src}
        alt=""
        referrerPolicy="no-referrer"
        draggable={false}
        onLoad={onImageLoad}
        style={{
          transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
          transition: dragging ? "none" : "transform 60ms ease-out",
        }}
      />
    </div>
  );
}
