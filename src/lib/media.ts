import type { MediaKind } from "@/lib/types";

const MEBIBYTE = 1024 * 1024;
const CHAT_IMAGE_TARGET_BYTES = 3 * MEBIBYTE;
const AVATAR_TARGET_BYTES = 512 * 1024;
export const CHAT_MEDIA_MAX_BYTES = 50 * MEBIBYTE;
export const CHAT_VIDEO_MAX_SECONDS = 120;

export interface PreparedMedia {
  kind: MediaKind;
  blob: Blob;
  mimeType: "image/webp" | "video/webm";
  extension: "webp" | "webm";
  width: number;
  height: number;
  durationSeconds: number | null;
  originalName: string;
}

function fitWithin(width: number, height: number, maxLong: number, maxShort: number) {
  const long = Math.max(width, height);
  const short = Math.min(width, height);
  const scale = Math.min(1, maxLong / long, maxShort / short);
  return {
    width: Math.max(2, Math.round((width * scale) / 2) * 2),
    height: Math.max(2, Math.round((height * scale) / 2) * 2),
  };
}

function canvasBlob(canvas: HTMLCanvasElement, quality: number): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => blob ? resolve(blob) : reject(new Error("This image could not be compressed.")),
      "image/webp",
      quality
    );
  });
}

async function encodeCanvas(
  source: CanvasImageSource,
  width: number,
  height: number,
  targetBytes: number
) {
  let outputWidth = width;
  let outputHeight = height;

  for (let resizePass = 0; resizePass < 4; resizePass += 1) {
    const canvas = document.createElement("canvas");
    canvas.width = outputWidth;
    canvas.height = outputHeight;
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) throw new Error("Image compression is unavailable.");
    context.imageSmoothingEnabled = true;
    context.imageSmoothingQuality = "high";
    context.drawImage(source, 0, 0, outputWidth, outputHeight);

    for (const quality of [0.88, 0.82, 0.76, 0.7, 0.64]) {
      const blob = await canvasBlob(canvas, quality);
      if (blob.size <= targetBytes || quality === 0.64) {
        if (blob.size <= targetBytes || resizePass === 3) {
          return { blob, width: outputWidth, height: outputHeight };
        }
        break;
      }
    }

    outputWidth = Math.max(2, Math.round((outputWidth * 0.82) / 2) * 2);
    outputHeight = Math.max(2, Math.round((outputHeight * 0.82) / 2) * 2);
  }

  throw new Error("This image could not be compressed enough.");
}

async function imageBitmap(file: File) {
  try {
    return await createImageBitmap(file, { imageOrientation: "from-image" });
  } catch {
    return await createImageBitmap(file);
  }
}

export async function prepareChatImage(file: File): Promise<PreparedMedia> {
  if (!file.type.startsWith("image/")) throw new Error("Choose an image file.");
  const bitmap = await imageBitmap(file);
  try {
    const size = fitWithin(bitmap.width, bitmap.height, 1920, 1920);
    const result = await encodeCanvas(bitmap, size.width, size.height, CHAT_IMAGE_TARGET_BYTES);
    if (result.blob.size > CHAT_MEDIA_MAX_BYTES) throw new Error("The compressed image is too large.");
    return {
      kind: "image",
      blob: result.blob,
      mimeType: "image/webp",
      extension: "webp",
      width: result.width,
      height: result.height,
      durationSeconds: null,
      originalName: file.name,
    };
  } finally {
    bitmap.close();
  }
}

export async function prepareAvatar(file: File): Promise<Blob> {
  if (!file.type.startsWith("image/")) throw new Error("Choose an image file.");
  const bitmap = await imageBitmap(file);
  try {
    const side = Math.min(bitmap.width, bitmap.height);
    const sourceX = Math.round((bitmap.width - side) / 2);
    const sourceY = Math.round((bitmap.height - side) / 2);
    const crop = document.createElement("canvas");
    crop.width = 512;
    crop.height = 512;
    const context = crop.getContext("2d", { alpha: false });
    if (!context) throw new Error("Image compression is unavailable.");
    context.imageSmoothingEnabled = true;
    context.imageSmoothingQuality = "high";
    context.drawImage(bitmap, sourceX, sourceY, side, side, 0, 0, 512, 512);

    for (const quality of [0.86, 0.78, 0.7, 0.62]) {
      const blob = await canvasBlob(crop, quality);
      if (blob.size <= AVATAR_TARGET_BYTES || quality === 0.62) return blob;
    }
    throw new Error("This avatar could not be compressed enough.");
  } finally {
    bitmap.close();
  }
}

function loadVideo(file: File): Promise<{ video: HTMLVideoElement; url: string }> {
  const url = URL.createObjectURL(file);
  const video = document.createElement("video");
  video.preload = "auto";
  video.playsInline = true;
  video.muted = true;
  video.src = url;

  return new Promise((resolve, reject) => {
    video.onloadedmetadata = () => resolve({ video, url });
    video.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("This video format is not supported."));
    };
    video.load();
  });
}

function recorderMimeType() {
  const options = [
    "video/webm;codecs=vp9,opus",
    "video/webm;codecs=vp8,opus",
    "video/webm;codecs=vp9",
    "video/webm;codecs=vp8",
    "video/webm",
  ];
  return options.find((type) => MediaRecorder.isTypeSupported(type)) ?? null;
}

type CapturableVideo = HTMLVideoElement & {
  captureStream?: () => MediaStream;
  mozCaptureStream?: () => MediaStream;
};

export async function prepareChatVideo(
  file: File,
  onProgress?: (progress: number) => void
): Promise<PreparedMedia> {
  if (!file.type.startsWith("video/")) throw new Error("Choose a video file.");
  if (file.size > 300 * MEBIBYTE) {
    throw new Error("Choose a source video smaller than 300 MiB.");
  }
  if (typeof MediaRecorder === "undefined") {
    throw new Error("Video compression is unavailable on this device.");
  }

  const mimeType = recorderMimeType();
  if (!mimeType) throw new Error("WebM video compression is unavailable on this device.");

  const { video, url } = await loadVideo(file);
  let drawTimer: number | undefined;
  let outputStream: MediaStream | null = null;

  try {
    const duration = video.duration;
    if (!Number.isFinite(duration) || duration <= 0) throw new Error("The video duration is invalid.");
    if (duration > CHAT_VIDEO_MAX_SECONDS) {
      throw new Error("Videos must be " + CHAT_VIDEO_MAX_SECONDS + " seconds or shorter.");
    }

    const dimensions = fitWithin(video.videoWidth, video.videoHeight, 1280, 720);
    const canvas = document.createElement("canvas");
    canvas.width = dimensions.width;
    canvas.height = dimensions.height;
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) throw new Error("Video compression is unavailable.");

    const canvasStream = canvas.captureStream(30);
    const capturable = video as CapturableVideo;
    const sourceStream = capturable.captureStream?.() ?? capturable.mozCaptureStream?.();

    const videoBitrateByResolution = Math.max(
      1_200_000,
      Math.round(3_200_000 * Math.sqrt((dimensions.width * dimensions.height) / (1280 * 720)))
    );
    const maxVideoBitrateForSize = Math.floor(
      ((48 * MEBIBYTE * 8) / duration) - 128_000
    );
    const videoBitsPerSecond = Math.max(
      900_000,
      Math.min(3_200_000, videoBitrateByResolution, maxVideoBitrateForSize)
    );

    outputStream = new MediaStream(canvasStream.getVideoTracks());
    for (const track of sourceStream?.getAudioTracks() ?? []) outputStream.addTrack(track);

    const chunks: BlobPart[] = [];
    const recorder = new MediaRecorder(outputStream, {
      mimeType,
      videoBitsPerSecond,
      audioBitsPerSecond: 128_000,
    });
    const result = new Promise<Blob>((resolve, reject) => {
      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.push(event.data);
      };
      recorder.onerror = () => reject(new Error("Video compression failed."));
      recorder.onstop = () => resolve(new Blob(chunks, { type: "video/webm" }));
    });

    const draw = () => {
      context.drawImage(video, 0, 0, dimensions.width, dimensions.height);
      onProgress?.(Math.min(0.99, video.currentTime / duration));
      if (!video.ended && !video.paused) {
        drawTimer = window.setTimeout(draw, 1000 / 30);
      }
    };

    await video.play();
    recorder.start(1000);
    draw();

    await new Promise<void>((resolve, reject) => {
      video.onended = () => resolve();
      video.onerror = () => reject(new Error("Video compression failed during playback."));
    });

    if (drawTimer !== undefined) window.clearTimeout(drawTimer);
    context.drawImage(video, 0, 0, dimensions.width, dimensions.height);
    recorder.stop();
    const blob = await result;
    onProgress?.(1);

    if (blob.size > CHAT_MEDIA_MAX_BYTES) {
      throw new Error("The compressed video is larger than 50 MiB.");
    }

    return {
      kind: "video",
      blob,
      mimeType: "video/webm",
      extension: "webm",
      width: dimensions.width,
      height: dimensions.height,
      durationSeconds: Math.round(duration * 1000) / 1000,
      originalName: file.name,
    };
  } finally {
    if (drawTimer !== undefined) window.clearTimeout(drawTimer);
    outputStream?.getTracks().forEach((track) => track.stop());
    video.pause();
    video.removeAttribute("src");
    video.load();
    URL.revokeObjectURL(url);
  }
}

export async function prepareChatMedia(
  file: File,
  onProgress?: (progress: number) => void
): Promise<PreparedMedia> {
  if (file.type.startsWith("image/")) return prepareChatImage(file);
  if (file.type.startsWith("video/")) return prepareChatVideo(file, onProgress);
  throw new Error("Only images and videos can be attached.");
}

export function formattedBytes(bytes: number) {
  if (bytes < MEBIBYTE) return Math.max(1, Math.round(bytes / 1024)) + " KiB";
  return (bytes / MEBIBYTE).toFixed(1) + " MiB";
}
