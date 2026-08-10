// A fixed peak threshold. This app deliberately runs the mic with
// autoGainControl off (see voice-media.ts), so raw levels run low - keep
// this well below normal speech, closer to the actual silence/noise floor,
// so quiet talking still registers.
const SPEAKING_THRESHOLD = 0.02;
// Keep "speaking" true briefly after the level drops, so natural pauses
// between words don't flicker the indicator off and on.
const SPEAKING_HOLD_MS = 300;
const SAMPLE_INTERVAL_MS = 50;
// Peak amplitude that maps to a full-intensity (1.0) level report. Set above
// normal talking peaks so intensity has headroom without needing a shout to
// visibly move.
const LEVEL_CEILING = 0.14;
// Curve applied to the normalized 0-1 ratio before reporting it. >1 pulls
// down the low end a bit, so normal speaking volume sits mid-range rather
// than pinning near-max immediately.
const LEVEL_CURVE = 1.25;

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
  onChange: (speaking: boolean) => void,
  // Continuous 0-1 loudness for the current tick, scaled against the same
  // threshold used for the speaking gate. Purely additive - never affects
  // the speaking/hold logic above.
  onLevel?: (level: number) => void
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

        if (onLevel) {
          const ratio = Math.min(
            1,
            Math.max(0, (peak - SPEAKING_THRESHOLD) / (LEVEL_CEILING - SPEAKING_THRESHOLD))
          );
          onLevel(next ? ratio ** LEVEL_CURVE : 0);
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
      if (onLevel) onLevel(0);
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
