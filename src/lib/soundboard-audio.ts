const MAX_DURATION_SECONDS = 15;
const TARGET_SAMPLE_RATE = 48_000;
const TARGET_AUDIO_BITRATE = 96_000;

export interface PreparedSound {
  blob: Blob;
  durationMs: number;
}

export async function prepareSoundboardAudio(file: File): Promise<PreparedSound> {
  if (!file.type.startsWith("audio/")) throw new Error("Choose an audio file.");
  if (file.size > 25 * 1024 * 1024) throw new Error("Source audio can be up to 25 MiB.");
  const input = await file.arrayBuffer();
  const decodeContext = new AudioContext();
  let decoded: AudioBuffer;
  try {
    decoded = await decodeContext.decodeAudioData(input.slice(0));
  } catch {
    throw new Error("This audio format could not be decoded.");
  } finally {
    await decodeContext.close();
  }

  const trimmed = trimBounds(decoded);
  const duration = (trimmed.end - trimmed.start) / decoded.sampleRate;
  if (duration < 0.1) throw new Error("The sound is empty.");
  if (duration > MAX_DURATION_SECONDS) {
    throw new Error("Soundboard clips can be up to 15 seconds.");
  }

  const frames = Math.ceil(duration * TARGET_SAMPLE_RATE);
  const offline = new OfflineAudioContext(1, frames, TARGET_SAMPLE_RATE);
  const source = offline.createBufferSource();
  source.buffer = decoded;
  const gain = offline.createGain();
  gain.gain.value = normalizationGain(decoded, trimmed.start, trimmed.end);
  source.connect(gain).connect(offline.destination);
  source.start(0, trimmed.start / decoded.sampleRate, duration);
  const normalized = await offline.startRendering();
  const blob = await encodeOpus(normalized);
  if (blob.size > 512 * 1024) {
    throw new Error("The compressed clip is over 512 KiB. Shorten it and try again.");
  }
  return { blob, durationMs: Math.round(duration * 1000) };
}

function trimBounds(buffer: AudioBuffer): { start: number; end: number } {
  const threshold = 0.004;
  let start = 0;
  let end = buffer.length;
  const channels = Array.from({ length: buffer.numberOfChannels }, (_, index) =>
    buffer.getChannelData(index)
  );
  while (start < end && channels.every((channel) => Math.abs(channel[start]) < threshold)) start += 1;
  while (end > start && channels.every((channel) => Math.abs(channel[end - 1]) < threshold)) end -= 1;
  const padding = Math.round(buffer.sampleRate * 0.02);
  return { start: Math.max(0, start - padding), end: Math.min(buffer.length, end + padding) };
}

function normalizationGain(buffer: AudioBuffer, start: number, end: number): number {
  let peak = 0;
  let sum = 0;
  let count = 0;
  for (let channelIndex = 0; channelIndex < buffer.numberOfChannels; channelIndex += 1) {
    const channel = buffer.getChannelData(channelIndex);
    for (let index = start; index < end; index += 8) {
      const value = channel[index];
      peak = Math.max(peak, Math.abs(value));
      sum += value * value;
      count += 1;
    }
  }
  const rms = Math.sqrt(sum / Math.max(1, count));
  return Math.min(4, 0.92 / Math.max(peak, 0.001), 0.14 / Math.max(rms, 0.001));
}

async function encodeOpus(buffer: AudioBuffer): Promise<Blob> {
  const mimeType = ["audio/webm;codecs=opus", "audio/webm"].find((type) =>
    MediaRecorder.isTypeSupported(type)
  );
  if (!mimeType) throw new Error("Opus compression is unavailable on this device.");

  const context = new AudioContext({ sampleRate: TARGET_SAMPLE_RATE });
  const destination = context.createMediaStreamDestination();
  const source = context.createBufferSource();
  source.buffer = buffer;
  source.connect(destination);
  const chunks: Blob[] = [];
  const recorder = new MediaRecorder(destination.stream, {
    mimeType,
    audioBitsPerSecond: TARGET_AUDIO_BITRATE,
  });
  const completed = new Promise<Blob>((resolve, reject) => {
    recorder.ondataavailable = (event) => {
      if (event.data.size) chunks.push(event.data);
    };
    recorder.onerror = () => reject(new Error("Audio compression failed."));
    recorder.onstop = () => resolve(new Blob(chunks, { type: "audio/webm" }));
  });
  recorder.start(250);
  source.start();
  source.onended = () => recorder.stop();
  try {
    return await completed;
  } finally {
    for (const track of destination.stream.getTracks()) track.stop();
    await context.close();
  }
}

const MAX_PLAYBACK_CACHE_BYTES = 32 * 1024 * 1024;

interface CachedClip {
  blob: Blob;
  lastUsedAt: number;
}

const playbackCache = new Map<string, CachedClip>();
const pendingDownloads = new Map<string, Promise<Blob>>();
let playbackCacheBytes = 0;

function cacheClip(cacheKey: string, blob: Blob): Blob {
  const previous = playbackCache.get(cacheKey);
  if (previous) playbackCacheBytes -= previous.blob.size;
  playbackCache.set(cacheKey, { blob, lastUsedAt: Date.now() });
  playbackCacheBytes += blob.size;

  while (playbackCacheBytes > MAX_PLAYBACK_CACHE_BYTES && playbackCache.size > 1) {
    const oldest = [...playbackCache.entries()].reduce((candidate, entry) =>
      entry[1].lastUsedAt < candidate[1].lastUsedAt ? entry : candidate
    );
    playbackCache.delete(oldest[0]);
    playbackCacheBytes -= oldest[1].blob.size;
  }
  return blob;
}

async function loadClip(cacheKey: string, signedUrl: string): Promise<Blob> {
  const cached = playbackCache.get(cacheKey);
  if (cached) {
    cached.lastUsedAt = Date.now();
    return cached.blob;
  }

  const pending = pendingDownloads.get(cacheKey);
  if (pending) return pending;

  const download = fetch(signedUrl)
    .then(async (response) => {
      if (!response.ok) throw new Error("Soundboard clip could not be downloaded.");
      return cacheClip(cacheKey, await response.blob());
    })
    .finally(() => pendingDownloads.delete(cacheKey));
  pendingDownloads.set(cacheKey, download);
  return download;
}

export function preloadSoundboardClips(clips: Array<{ id: string; signedUrl: string }>): void {
  // Clips are tiny (max 512 KiB) and retained only for this app session. Warming
  // the shared library removes the download from the click-to-play path.
  const queue = [...clips];
  const worker = async () => {
    while (queue.length) {
      const clip = queue.shift();
      if (clip) await loadClip(clip.id, clip.signedUrl).catch(() => undefined);
    }
  };
  for (let index = 0; index < Math.min(4, queue.length); index += 1) void worker();
}

export interface SoundboardPlayback {
  stop: () => void;
}

export async function playSoundboardUrl(
  cacheKey: string,
  signedUrl: string,
  playAt: number,
  volume: number,
  outputDeviceId: string,
  callbacks?: {
    onProgress?: (progress: number) => void;
    onEnded?: () => void;
  }
): Promise<SoundboardPlayback> {
  const blob = await loadClip(cacheKey, signedUrl);
  const url = URL.createObjectURL(blob);
  const audio = new Audio(url);
  audio.preload = "auto";
  audio.volume = Math.min(1, Math.max(0, volume / 100));
  await routeSoundboardAudio(audio, outputDeviceId);

  let timer: number | null = null;
  let finished = false;
  const finish = () => {
    if (finished) return;
    finished = true;
    if (timer !== null) window.clearTimeout(timer);
    timer = null;
    URL.revokeObjectURL(url);
    callbacks?.onEnded?.();
  };
  const start = () => {
    timer = null;
    void audio.play().catch(() => finish());
  };

  audio.addEventListener("timeupdate", () => {
    if (audio.duration > 0 && Number.isFinite(audio.duration)) {
      callbacks?.onProgress?.(Math.max(0, Math.min(1, audio.currentTime / audio.duration)));
    }
  });
  audio.addEventListener("ended", finish, { once: true });
  timer = window.setTimeout(start, Math.max(0, playAt - Date.now()));

  return {
    stop: () => {
      if (finished) return;
      audio.pause();
      finish();
    },
  };
}
type SinkAudio = HTMLAudioElement & {
  setSinkId?: (sinkId: string) => Promise<void>;
};

async function routeSoundboardAudio(audio: HTMLAudioElement, outputDeviceId: string): Promise<void> {
  const sinkAudio = audio as SinkAudio;
  if (!sinkAudio.setSinkId) return;
  try {
    await sinkAudio.setSinkId(outputDeviceId === "default" ? "" : outputDeviceId);
  } catch {
    await sinkAudio.setSinkId("").catch(() => undefined);
  }
}