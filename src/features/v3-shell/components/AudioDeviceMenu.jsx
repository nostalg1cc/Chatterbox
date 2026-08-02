import { useCallback, useEffect, useState } from "react";
import { Mic, Volume2 } from "lucide-react";
import { usePreferences } from "@/stores/preferences";

export function AudioDeviceMenu({ kind, open, onSelect }) {
  const [devices, setDevices] = useState([]); const [loading, setLoading] = useState(false);
  const input = kind === "input"; const selected = usePreferences((state) => input ? state.inputDeviceId : state.outputDeviceId);
  const refresh = useCallback(async () => {
    if (!navigator.mediaDevices?.enumerateDevices) return setDevices([]);
    setLoading(true);
    try {
      let found = await navigator.mediaDevices.enumerateDevices();
      let filtered = found.filter((device) => device.kind === (input ? "audioinput" : "audiooutput") && device.deviceId !== "default");
      if ((filtered.length === 0 || filtered.some((device) => !device.label)) && navigator.mediaDevices.getUserMedia) {
        try { const probe = await navigator.mediaDevices.getUserMedia({ audio: true }); probe.getTracks().forEach((track) => track.stop()); found = await navigator.mediaDevices.enumerateDevices(); filtered = found.filter((device) => device.kind === (input ? "audioinput" : "audiooutput") && device.deviceId !== "default"); } catch { /* default route remains usable */ }
      }
      setDevices(filtered);
    } finally { setLoading(false); }
  }, [input]);
  useEffect(() => { if (!open) return; void refresh(); navigator.mediaDevices?.addEventListener?.("devicechange", refresh); return () => navigator.mediaDevices?.removeEventListener?.("devicechange", refresh); }, [open, refresh]);
  const choose = (id) => { usePreferences.getState().setPreference(input ? "inputDeviceId" : "outputDeviceId", id); onSelect?.(); };
  return <div className="audio-device-menu"><p>{input ? <Mic /> : <Volume2 />}{input ? "Input device" : "Output device"}</p><button className={selected === "default" ? "is-selected" : ""} type="button" onClick={() => choose("default")}>Default {input ? "microphone" : "speakers"}</button>{loading ? <small>Finding devices…</small> : devices.map((device,index) => <button key={device.deviceId} className={selected === device.deviceId ? "is-selected" : ""} type="button" onClick={() => choose(device.deviceId)}>{device.label || `${input ? "Microphone" : "Output"} ${index + 1}`}</button>)}</div>;
}
