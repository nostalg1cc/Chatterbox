import { usePreferences } from "@/stores/preferences";

export type AppSound =
  | "call_join"
  | "call_leave"
  | "deafen_off"
  | "deafen_on"
  | "mute_off"
  | "mute_on"
  | "notification_single";

type SinkCapableAudio = HTMLAudioElement & {
  setSinkId?: (sinkId: string) => Promise<void>;
};

const fileBySound: Record<AppSound, string> = {
  call_join: "joinvoice.mp3",
  call_leave: "voicechatleave.mp3",
  deafen_off: "mutedeafenabletoggle.mp3",
  deafen_on: "mutedeafdisabletoggle.mp3",
  mute_off: "mutedeafenabletoggle.mp3",
  mute_on: "mutedeafdisabletoggle.mp3",
  notification_single: "mesage.mp3",
};

const sources = new Map<AppSound, string>();

function sourceFor(sound: AppSound): string {
  return "/sounds/system/" + fileBySound[sound];
}

export function preloadAppSounds(): void {
  for (const sound of ["call_join", "call_leave", "deafen_off", "deafen_on", "mute_off", "mute_on", "notification_single"] as AppSound[]) {
    const source = sourceFor(sound);
    sources.set(sound, source);
    const audio = new Audio(source);
    audio.preload = "auto";
    audio.load();
  }
}

export function playAppSound(sound: AppSound, force = false): void {
  const preferences = usePreferences.getState();
  if (!force && !preferences.interfaceSounds) return;

  const audio = new Audio(sources.get(sound) ?? sourceFor(sound));
  audio.preload = "auto";
  audio.volume = Math.min(1, Math.max(0, preferences.interfaceSoundVolume / 100));
  void routeAndPlay(audio, preferences.outputDeviceId);
}

async function routeAndPlay(audio: HTMLAudioElement, outputDeviceId: string): Promise<void> {
  const sinkAudio = audio as SinkCapableAudio;
  if (sinkAudio.setSinkId) {
    try {
      await sinkAudio.setSinkId(outputDeviceId === "default" ? "" : outputDeviceId);
    } catch {
      await sinkAudio.setSinkId("").catch(() => undefined);
    }
  }
  await audio.play().catch(() => undefined);
}