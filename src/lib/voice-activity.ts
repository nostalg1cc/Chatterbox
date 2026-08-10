// A fixed peak threshold. This app deliberately runs the mic with
// autoGainControl off (see voice-media.ts), so raw levels run low - keep
// this well below normal speech, closer to the actual silence/noise floor,
// so quiet talking still registers.
const SPEAKING_THRESHOLD = 0.02;
// Keep "speaking" true briefly after the level drops, so natural pauses
// between words don't flicker the indicator off and on.
const SPEAKING_HOLD_MS = 300;
const SAMPLE_INTERVAL_MS = 50;

export interface VoiceActivityMonitor {
  stop: () => void;
}

export type VoiceActivitySource =
  | MediaStream
  // An existing, already-running AudioContext + node to tap (e.g. the
  // microphone pipeline's own gain node). Preferred when available: it reads
  // the exact signal already proven to flow (the one actually being sent),
  // instead of asking a second AudioContext to independently consume the
  // same MediaStreamTrack.
  | { context: AudioContext; node: AudioNode };

function isMediaStream(source: VoiceActivitySource): source is MediaStream {
  return typeof MediaStream !== "undefined" && source instanceof MediaStream;
}

/**
 * Passive volume metering only - the analyser is a dead end (never connected
 * to a destination), so this has no effect on the actual audio graph or on
 * what either side hears.
 */
export function monitorVoiceActivity(
  source: VoiceActivitySource,
  onChange: (speaking: boolean) => void
): VoiceActivityMonitor {
  let stopped = false;
  let speaking = false;
  let lastLoudAt = 0;
  let ownedContext: AudioContext | null = null;
  let tappedNode: AudioNode | null = null;
  let analyserNode: AnalyserNode | null = null;
  let interval: ReturnType<typeof setInterval> | null = null;

  void (async () => {
    try {
      let context: AudioContext;
      let node: AudioNode;

      if (isMediaStream(source)) {
        context = new AudioContext({ latencyHint: "interactive" });
        ownedContext = context;
        await context.resume();
        if (stopped || context.state !== "running") return;
        node = context.createMediaStreamSource(source);
      } else {
        context = source.context;
        node = source.node;
        if (stopped || context.state !== "running") return;
      }

      const analyser = context.createAnalyser();
      analyser.fftSize = 512;
      node.connect(analyser);
      tappedNode = node;
      analyserNode = analyser;
      if (stopped) {
        // stop() ran while we were still spinning up - tear down what we
        // just connected instead of leaving it attached indefinitely.
        node.disconnect(analyser);
        return;
      }
      const data = new Uint8Array(analyser.fftSize);

      interval = setInterval(() => {
        analyser.getByteTimeDomainData(data);
        let peak = 0;
        for (let i = 0; i < data.length; i += 1) {
          const normalized = Math.abs(data[i] - 128) / 128;
          if (normalized > peak) peak = normalized;
        }

        const now = Date.now();
        if (peak > SPEAKING_THRESHOLD) lastLoudAt = now;

        const next = now - lastLoudAt < SPEAKING_HOLD_MS;
        if (next !== speaking) {
          speaking = next;
          onChange(speaking);
        }
      }, SAMPLE_INTERVAL_MS);
    } catch (error) {
      // Voice-activity display is optional polish and must never affect the
      // call, but stay loud about failures instead of swallowing them.
      console.warn("Voice activity monitoring failed to start", error);
    }
  })();

  return {
    stop: () => {
      stopped = true;
      if (interval) clearInterval(interval);
      if (speaking) onChange(false);
      // Detach from the tapped node so a shared, longer-lived context (the
      // microphone pipeline's own) isn't left with a dangling connection.
      if (tappedNode && analyserNode) {
        try {
          tappedNode.disconnect(analyserNode);
        } catch {
          // Already disconnected (e.g. the node's context closed first) - fine.
        }
      }
      // Only close a context this monitor created itself - never close the
      // shared microphone-pipeline context out from under the actual call.
      if (ownedContext) void ownedContext.close().catch(() => undefined);
    },
  };
}
