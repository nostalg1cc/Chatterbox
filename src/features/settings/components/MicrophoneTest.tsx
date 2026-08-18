import { useCallback, useEffect, useRef, useState } from "react";
import { useAlerts } from "@/stores/alerts";
import {
  configureRemoteAudio,
  createMicrophonePipeline,
  createRemoteAudioElement,
  disposeRemoteAudio,
  stopMicrophonePipeline,
  type MicrophonePipeline,
} from "@/lib/voice-media";

export function MicrophoneTest({
  inputDeviceId,
  inputVolume,
  noiseSuppression,
  outputDeviceId,
  outputVolume,
}: {
  inputDeviceId: string;
  inputVolume: number;
  noiseSuppression: boolean;
  outputDeviceId: string;
  outputVolume: number;
}) {
  const monitorRef = useRef<{ pipeline: MicrophonePipeline; audio: HTMLAudioElement } | null>(null);
  const [testing, setTesting] = useState(false);

  const stopTest = useCallback(async () => {
    const monitor = monitorRef.current;
    if (!monitor) return;
    monitorRef.current = null;
    await disposeRemoteAudio(monitor.audio);
    await stopMicrophonePipeline(monitor.pipeline);
    setTesting(false);
  }, []);

  // Unmounts (tab switch, closing Settings) naturally stop any running test -
  // only the active tab's panel is ever rendered, so there's no separate
  // "am I still the visible tab" flag to track here.
  useEffect(() => () => {
    const monitor = monitorRef.current;
    if (!monitor) return;
    monitorRef.current = null;
    void disposeRemoteAudio(monitor.audio);
    void stopMicrophonePipeline(monitor.pipeline);
  }, []);

  const toggleTest = async () => {
    if (monitorRef.current) {
      await stopTest();
      return;
    }

    try {
      const pipeline = await createMicrophonePipeline(inputDeviceId, inputVolume, noiseSuppression);
      const audio = createRemoteAudioElement();
      monitorRef.current = { pipeline, audio };
      await configureRemoteAudio(audio, {
        stream: pipeline.outputStream,
        outputVolume,
        outputDeviceId,
        deafened: false,
      });
      setTesting(true);
    } catch (error) {
      const monitor = monitorRef.current;
      monitorRef.current = null;
      if (monitor) {
        await disposeRemoteAudio(monitor.audio);
        await stopMicrophonePipeline(monitor.pipeline);
      }
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "Couldn't start the microphone test." });
    }
  };

  return (
    <div className="v3-settings__row v3-settings__row--tight">
      <div className="v3-settings__row-copy">
        <p className="v3-settings__row-title">Microphone test</p>
        <p className="v3-settings__row-desc">
          {testing
            ? "You are hearing your current input settings. Use headphones to prevent feedback."
            : "Listen to your microphone with the selected input volume and suppression setting."}
        </p>
      </div>
      <button type="button" className={"v3-settings__ghost-button" + (testing ? " is-danger" : "")} onClick={() => void toggleTest()}>
        {testing ? "Stop test" : "Test microphone"}
      </button>
    </div>
  );
}
