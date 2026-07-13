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

function audioConstraints(deviceId: string): MediaTrackConstraints {
  return {
    deviceId: deviceId === "default" ? undefined : { exact: deviceId },
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
    channelCount: 1,
  };
}

async function captureMicrophone(deviceId: string): Promise<{
  stream: MediaStream;
  fellBackToDefault: boolean;
}> {
  try {
    return {
      stream: await navigator.mediaDevices.getUserMedia({
        audio: audioConstraints(deviceId),
        video: false,
      }),
      fellBackToDefault: false,
    };
  } catch (error) {
    if (deviceId === "default") throw error;
    return {
      stream: await navigator.mediaDevices.getUserMedia({
        audio: audioConstraints("default"),
        video: false,
      }),
      fellBackToDefault: true,
    };
  }
}

export async function createMicrophonePipeline(
  deviceId: string,
  inputVolume: number
): Promise<MicrophonePipeline> {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new Error("Microphone capture is unavailable in this WebView.");
  }

  const { stream, fellBackToDefault } = await captureMicrophone(deviceId);
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

export async function configureRemoteAudio(
  element: HTMLAudioElement,
  {
    stream,
    outputVolume,
    outputDeviceId,
    deafened,
  }: {
    stream?: MediaStream;
    outputVolume: number;
    outputDeviceId: string;
    deafened: boolean;
  }
): Promise<void> {
  if (stream) element.srcObject = stream;
  element.volume = Math.max(0, Math.min(1, outputVolume / 100));
  element.muted = deafened;

  const sinkAudio = element as SinkCapableAudio;
  if (sinkAudio.setSinkId) {
    try {
      await sinkAudio.setSinkId(outputDeviceId === "default" ? "" : outputDeviceId);
    } catch {
      await sinkAudio.setSinkId("").catch(() => undefined);
    }
  }

  if (element.srcObject && !deafened) {
    try {
      await element.play();
    } catch {
      armPlaybackRetry(element);
    }
  }
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
      frameRate: { ideal: 60, max: 60 },
    },
    audio: false,
  });
}