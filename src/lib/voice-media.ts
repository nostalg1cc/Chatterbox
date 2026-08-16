// Shared by both screen-share send paths (direct P2P in stores/voice.ts and
// the Cloudflare relay in lib/cloudflare-realtime.ts). The P2P path adds
// screen video onto the same RTCPeerConnection that carries live voice
// audio, so this ceiling isn't just about screen-share quality - too high
// and it can saturate a typical home upload connection on its own once
// maintain-framerate actually lets it use that much, starving voice audio
// behind it (multi-second delay as the jitter buffer tries to smooth over
// the resulting gaps).
export const SCREEN_SHARE_MAX_BITRATE = 2_500_000;

export interface MicrophonePipeline {
  rawStream: MediaStream;
  outputStream: MediaStream;
  context: AudioContext | null;
  source: MediaStreamAudioSourceNode | null;
  gain: GainNode | null;
  destination: MediaStreamAudioDestinationNode | null;
  fellBackToDefault: boolean;
  usesAudioGraph: boolean;
}

type SinkCapableAudio = HTMLAudioElement & {
  setSinkId?: (sinkId: string) => Promise<void>;
};

interface RemoteAudioProcessor {
  sourceStream: MediaStream;
  context: AudioContext;
  source: MediaStreamAudioSourceNode;
  gain: GainNode;
  destination: MediaStreamAudioDestinationNode;
}

// Keep remote processing separate from the sender's microphone pipeline. A
// listener can safely boost their partner without changing the call's outgoing
// stream or the partner's local volume.
const remoteAudioProcessors = new WeakMap<HTMLAudioElement, RemoteAudioProcessor>();

// A slider drag fires many rapid preference updates, each triggering an async
// configure/dispose call for the same element. Without serialising them, two
// in-flight calls can both see no processor yet, each create their own
// AudioContext, and race to set element.srcObject last — the audible
// break/mute. Chaining every call for an element through one queue makes them
// run one at a time so each sees the previous call's finished state.
const remoteAudioQueues = new WeakMap<HTMLAudioElement, Promise<void>>();

function queueRemoteAudioTask(
  element: HTMLAudioElement,
  task: () => Promise<void>
): Promise<void> {
  const previous = remoteAudioQueues.get(element) ?? Promise.resolve();
  const next = previous.then(task, task);
  remoteAudioQueues.set(
    element,
    next.catch(() => undefined)
  );
  return next;
}

function audioConstraints(deviceId: string, noiseSuppression: boolean): MediaTrackConstraints {
  return {
    deviceId: deviceId === "default" ? undefined : { exact: deviceId },
    echoCancellation: false,
    noiseSuppression,
    autoGainControl: false,
    channelCount: 1,
  };
}

async function captureMicrophone(deviceId: string, noiseSuppression: boolean): Promise<{
  stream: MediaStream;
  fellBackToDefault: boolean;
}> {
  try {
    return {
      stream: await navigator.mediaDevices.getUserMedia({
        audio: audioConstraints(deviceId, noiseSuppression),
        video: false,
      }),
      fellBackToDefault: false,
    };
  } catch (error) {
    if (deviceId === "default") throw error;
    return {
      stream: await navigator.mediaDevices.getUserMedia({
        audio: audioConstraints("default", noiseSuppression),
        video: false,
      }),
      fellBackToDefault: true,
    };
  }
}

export async function createMicrophonePipeline(
  deviceId: string,
  inputVolume: number,
  noiseSuppression = false
): Promise<MicrophonePipeline> {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new Error("Microphone capture is unavailable in this WebView.");
  }

  const { stream, fellBackToDefault } = await captureMicrophone(deviceId, noiseSuppression);
  const rawTrack = stream.getAudioTracks()[0];
  if (!rawTrack) {
    for (const track of stream.getTracks()) track.stop();
    throw new Error("The microphone did not provide an audio track.");
  }

  let context: AudioContext | null = null;
  try {
    context = new AudioContext({ latencyHint: "interactive" });
    await context.resume();
    if (context.state !== "running") {
      await context.close().catch(() => undefined);
      return rawMicrophonePipeline(stream, fellBackToDefault);
    }

    const source = context.createMediaStreamSource(stream);
    const gain = context.createGain();
    const destination = context.createMediaStreamDestination();
    gain.gain.value = Math.max(0, Math.min(1, inputVolume / 100));
    source.connect(gain);
    gain.connect(destination);

    const outputTrack = destination.stream.getAudioTracks()[0];
    if (!outputTrack) throw new Error("The audio processor did not provide an output track.");

    return {
      rawStream: stream,
      outputStream: destination.stream,
      context,
      source,
      gain,
      destination,
      fellBackToDefault,
      usesAudioGraph: true,
    };
  } catch {
    await context?.close().catch(() => undefined);
    return rawMicrophonePipeline(stream, fellBackToDefault);
  }
}

function rawMicrophonePipeline(
  stream: MediaStream,
  fellBackToDefault: boolean
): MicrophonePipeline {
  return {
    rawStream: stream,
    outputStream: stream,
    context: null,
    source: null,
    gain: null,
    destination: null,
    fellBackToDefault,
    usesAudioGraph: false,
  };
}

export function setMicrophoneGain(
  pipeline: MicrophonePipeline | null,
  inputVolume: number
): void {
  if (!pipeline?.gain || !pipeline.context) return;
  const value = Math.max(0, Math.min(1, inputVolume / 100));
  pipeline.gain.gain.setTargetAtTime(value, pipeline.context.currentTime, 0.015);
}

export async function stopMicrophonePipeline(
  pipeline: MicrophonePipeline | null
): Promise<void> {
  if (!pipeline) return;
  pipeline.source?.disconnect();
  pipeline.gain?.disconnect();
  const tracks = new Set([
    ...pipeline.outputStream.getTracks(),
    ...pipeline.rawStream.getTracks(),
  ]);
  for (const track of tracks) track.stop();
  await pipeline.context?.close().catch(() => undefined);
}

export function createRemoteAudioElement(): HTMLAudioElement {
  const element = document.createElement("audio");
  element.autoplay = true;
  element.preload = "auto";
  element.setAttribute("aria-hidden", "true");
  element.style.display = "none";
  document.body.appendChild(element);
  return element;
}

export async function configureMediaOutput(
  element: HTMLMediaElement,
  {
    volume,
    outputDeviceId,
    muted = false,
  }: {
    volume: number;
    outputDeviceId: string;
    muted?: boolean;
  }
): Promise<void> {
  element.volume = Math.max(0, Math.min(1, volume / 100));
  element.muted = muted;
  const sinkMedia = element as HTMLMediaElement & SinkCapableAudio;
  if (sinkMedia.setSinkId) {
    try {
      await sinkMedia.setSinkId(outputDeviceId === "default" ? "" : outputDeviceId);
    } catch {
      await sinkMedia.setSinkId("").catch(() => undefined);
    }
  }
}
export function configureRemoteAudio(
  element: HTMLAudioElement,
  options: {
    stream?: MediaStream;
    outputVolume: number;
    outputDeviceId: string;
    deafened: boolean;
    partnerVoiceBoost?: number;
  }
): Promise<void> {
  return queueRemoteAudioTask(element, () => applyRemoteAudioConfig(element, options));
}

async function applyRemoteAudioConfig(
  element: HTMLAudioElement,
  {
    stream,
    outputVolume,
    outputDeviceId,
    deafened,
    partnerVoiceBoost = 100,
  }: {
    stream?: MediaStream;
    outputVolume: number;
    outputDeviceId: string;
    deafened: boolean;
    partnerVoiceBoost?: number;
  }
): Promise<void> {
  const existingProcessor = remoteAudioProcessors.get(element);
  const sourceStream =
    stream ??
    existingProcessor?.sourceStream ??
    (element.srcObject instanceof MediaStream ? element.srcObject : undefined);
  const boost = Math.max(100, Math.min(200, partnerVoiceBoost));

  if (sourceStream && boost > 100) {
    try {
      const processor = await ensureRemoteAudioProcessor(element, sourceStream);
      processor.gain.gain.setTargetAtTime(
        boost / 100,
        processor.context.currentTime,
        0.015
      );
      element.srcObject = processor.destination.stream;
    } catch (error) {
      // Remote playback must never depend on the optional boost graph. Falling
      // back to the original stream preserves the known-good voice path.
      console.warn("Partner voice boost is unavailable; using direct audio.", error);
      await disposeRemoteAudioProcessor(element);
      element.srcObject = sourceStream;
    }
  } else {
    await disposeRemoteAudioProcessor(element);
    if (sourceStream) element.srcObject = sourceStream;
  }
  await configureMediaOutput(element, { volume: outputVolume, outputDeviceId, muted: deafened });

  if (element.srcObject && !deafened) {
    try {
      await element.play();
    } catch {
      armPlaybackRetry(element);
    }
  }
}

export function disposeRemoteAudio(element: HTMLAudioElement): Promise<void> {
  return queueRemoteAudioTask(element, async () => {
    element.pause();
    await disposeRemoteAudioProcessor(element);
    element.srcObject = null;
    element.remove();
  });
}

async function ensureRemoteAudioProcessor(
  element: HTMLAudioElement,
  stream: MediaStream
): Promise<RemoteAudioProcessor> {
  const existing = remoteAudioProcessors.get(element);
  if (existing?.sourceStream === stream) return existing;
  await disposeRemoteAudioProcessor(element);

  const context = new AudioContext({ latencyHint: "interactive" });
  await context.resume();
  if (context.state !== "running") {
    await context.close().catch(() => undefined);
    throw new Error("Audio processing could not start.");
  }

  try {
    const source = context.createMediaStreamSource(stream);
    const gain = context.createGain();
    const destination = context.createMediaStreamDestination();
    source.connect(gain);
    gain.connect(destination);
    const processor = { sourceStream: stream, context, source, gain, destination };
    remoteAudioProcessors.set(element, processor);
    return processor;
  } catch (error) {
    await context.close().catch(() => undefined);
    throw error;
  }
}

async function disposeRemoteAudioProcessor(element: HTMLAudioElement): Promise<void> {
  const processor = remoteAudioProcessors.get(element);
  if (!processor) return;
  remoteAudioProcessors.delete(element);
  processor.source.disconnect();
  processor.gain.disconnect();
  await processor.context.close().catch(() => undefined);
}
function armPlaybackRetry(element: HTMLAudioElement): void {
  const retry = () => {
    void element.play().finally(() => {
      window.removeEventListener("pointerdown", retry, true);
      window.removeEventListener("keydown", retry, true);
    });
  };
  window.addEventListener("pointerdown", retry, { capture: true, once: true });
  window.addEventListener("keydown", retry, { capture: true, once: true });
}

export function supportsAudioOutputSelection(): boolean {
  return "setSinkId" in HTMLMediaElement.prototype;
}

export async function captureScreen(): Promise<MediaStream> {
  if (!navigator.mediaDevices?.getDisplayMedia) {
    throw new Error("Screen sharing requires a newer Microsoft Edge WebView2 runtime.");
  }

  return navigator.mediaDevices.getDisplayMedia({
    video: {
      width: { ideal: 1280, max: 1280 },
      height: { ideal: 720, max: 720 },
      frameRate: { ideal: 30, max: 30 },
    },
    audio: {
      echoCancellation: false,
      noiseSuppression: false,
      autoGainControl: false,
    },
  });
}

export interface ScreenAudioBoost {
  /** Same video track(s) as the input, plus a boosted copy of the audio
   * track (if any) in place of the raw one - use this for anything sent
   * out, not the original capture. */
  stream: MediaStream;
  /** Stops the boosted track and tears down the audio graph built on top
   * of the original capture. Does not touch the original capture's own
   * tracks - the caller still owns those. */
  cleanup: () => void;
}

// Windows' window/loopback audio capture (what getDisplayMedia's audio
// constraint pulls from) commonly comes in noticeably quieter than what's
// actually audible on the source machine - a well-known WASAPI loopback
// quirk. autoGainControl is deliberately off above (voice-oriented AGC
// mangles game/movie audio dynamics the same way it would over-process
// music), so nothing else compensates for that without this - boost it a
// fixed amount rather than adaptively, to keep it predictable.
const SCREEN_SHARE_AUDIO_GAIN = 2.5;

export function boostScreenShareAudio(stream: MediaStream): ScreenAudioBoost {
  const audioTrack = stream.getAudioTracks()[0];
  if (!audioTrack) return { stream, cleanup: () => undefined };
  try {
    const context = new AudioContext();
    const source = context.createMediaStreamSource(new MediaStream([audioTrack]));
    const gain = context.createGain();
    gain.gain.value = SCREEN_SHARE_AUDIO_GAIN;
    const destination = context.createMediaStreamDestination();
    source.connect(gain);
    gain.connect(destination);
    const boostedTrack = destination.stream.getAudioTracks()[0];
    if (!boostedTrack) {
      void context.close().catch(() => undefined);
      return { stream, cleanup: () => undefined };
    }
    return {
      stream: new MediaStream([...stream.getVideoTracks(), boostedTrack]),
      cleanup: () => {
        boostedTrack.stop();
        void context.close().catch(() => undefined);
      },
    };
  } catch {
    return { stream, cleanup: () => undefined };
  }
}
