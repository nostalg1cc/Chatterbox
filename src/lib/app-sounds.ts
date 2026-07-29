import { usePreferences } from "@/stores/preferences";

export type AppSound =
  | "voice_join"
  | "voice_leave"
  | "deafen_off"
  | "deafen_on"
  | "mute_off"
  | "mute_on"
  | "notification_single"
  | "ui_click"
  | "ui_hover"
  | "ui_input_focus"
  | "ui_menu_open"
  | "ui_menu_close";

type SinkCapableAudio = HTMLAudioElement & {
  setSinkId?: (sinkId: string) => Promise<void>;
};

type Tone = { frequency: number; offset: number; duration: number; volume: number; waveform?: OscillatorType };

const fileBySound: Record<AppSound, string> = {
  voice_join: "ui/pop_open.mp3",
  voice_leave: "ui/pop_close.mp3",
  deafen_off: "ui/toggle_on.mp3",
  deafen_on: "ui/toggle_off.mp3",
  mute_off: "ui/toggle_on.mp3",
  mute_on: "ui/toggle_off.mp3",
  notification_single: "ui/button_soft.mp3",
  ui_click: "ui/button_soft.mp3",
  ui_hover: "ui/button_squishy.mp3",
  ui_input_focus: "ui/input_focus.mp3",
  ui_menu_open: "ui/pop_open.mp3",
  ui_menu_close: "ui/pop_close.mp3",
};

const sources = new Map<AppSound, string>();
const lastPlayedAt = new Map<AppSound, number>();
let uiAudioContext: AudioContext | null = null;

function sourceFor(sound: AppSound): string {
  return "/sounds/" + fileBySound[sound];
}

function getUiAudioContext(): AudioContext | null {
  if (typeof window === "undefined" || !window.AudioContext) return null;
  uiAudioContext ??= new window.AudioContext();
  return uiAudioContext;
}

function playToneSequence(tones: Tone[], volumeScale: number): boolean {
  const context = getUiAudioContext();
  if (!context) return false;

  void context.resume().catch(() => undefined);
  const startTime = context.currentTime + 0.01;
  for (const { frequency, offset, duration, volume, waveform } of tones) {
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    const toneStart = startTime + offset;
    const toneEnd = toneStart + duration;
    oscillator.type = waveform ?? "sine";
    oscillator.frequency.setValueAtTime(frequency, toneStart);
    gain.gain.setValueAtTime(0.0001, toneStart);
    gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, volume * volumeScale), toneStart + 0.008);
    gain.gain.exponentialRampToValueAtTime(0.0001, toneEnd);
    oscillator.connect(gain);
    gain.connect(context.destination);
    oscillator.start(toneStart);
    oscillator.stop(toneEnd + 0.01);
  }
  return true;
}

function studyTones(sound: AppSound): Tone[] {
  const action = [
    { frequency: 466.16, offset: 0, duration: 0.035, volume: 0.013 },
    { frequency: 554.37, offset: 0.04, duration: 0.052, volume: 0.018 },
  ];
  const reverse = [
    { frequency: 554.37, offset: 0, duration: 0.052, volume: 0.018 },
    { frequency: 466.16, offset: 0.04, duration: 0.035, volume: 0.013 },
  ];
  const enabled = [
    { frequency: 523.25, offset: 0, duration: 0.052, volume: 0.024 },
    { frequency: 659.25, offset: 0.058, duration: 0.075, volume: 0.032 },
  ];
  const disabled = [
    { frequency: 392, offset: 0, duration: 0.06, volume: 0.028 },
    { frequency: 293.66, offset: 0.064, duration: 0.082, volume: 0.022 },
  ];
  // Voice state changes deserve a clearer cue than ordinary interaction sounds.
  const voiceJoin = [
    { frequency: 392, offset: 0, duration: 0.06, volume: 0.030, waveform: "triangle" as const },
    { frequency: 523.25, offset: 0.055, duration: 0.075, volume: 0.040, waveform: "triangle" as const },
    { frequency: 783.99, offset: 0.120, duration: 0.120, volume: 0.052, waveform: "sine" as const },
  ];
  const voiceLeave = [
    { frequency: 659.25, offset: 0, duration: 0.070, volume: 0.040, waveform: "triangle" as const },
    { frequency: 493.88, offset: 0.062, duration: 0.078, volume: 0.044, waveform: "triangle" as const },
    { frequency: 329.63, offset: 0.132, duration: 0.125, volume: 0.048, waveform: "sine" as const },
  ];

  switch (sound) {
    case "ui_hover": return [{ frequency: 739.99, offset: 0, duration: 0.028, volume: 0.006 }];
    case "voice_join": return voiceJoin;
    case "voice_leave": return voiceLeave;
    case "ui_menu_close": return reverse;
    case "mute_on":
    case "deafen_on": return disabled;
    case "mute_off":
    case "deafen_off": return enabled;
    default: return action;
  }
}

export function preloadAppSounds(): void {
  for (const sound of Object.keys(fileBySound) as AppSound[]) {
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

  const now = performance.now();
  const cooldown = sound === "ui_hover" ? 90 : sound === "ui_input_focus" ? 140 : 0;
  if (cooldown && now - (lastPlayedAt.get(sound) ?? 0) < cooldown) return;
  lastPlayedAt.set(sound, now);

  const volumeScale = Math.min(1, Math.max(0, preferences.interfaceSoundVolume / 100));
  if (playToneSequence(studyTones(sound), volumeScale)) return;

  const audio = new Audio(sources.get(sound) ?? sourceFor(sound));
  audio.preload = "auto";
  audio.volume = volumeScale;
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
