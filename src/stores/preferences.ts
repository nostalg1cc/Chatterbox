import { create } from "zustand";

export type WindowMaterial = "mica" | "acrylic";

export interface KeybindPreferences {
  toggleMute: string;
  toggleDeafen: string;
  leaveVoice: string;
  toggleScreenShare: string;
  openSoundboard: string;
}

interface PreferencesState {
  enterToSend: boolean;
  composerHintVisible: boolean;
  compactMessages: boolean;
  showMediaPreviews: boolean;
  inputDeviceId: string;
  outputDeviceId: string;
  inputVolume: number;
  outputVolume: number;
  interfaceSounds: boolean;
  interfaceSoundVolume: number;
  soundboardVolume: number;
  windowMaterial: WindowMaterial;
  acrylicDim: number;
  keybinds: KeybindPreferences;
  setPreference: <K extends keyof PreferencesData>(key: K, value: PreferencesData[K]) => void;
}

type PreferencesData = Omit<PreferencesState, "setPreference">;

const STORAGE_KEY = "dislight-preferences-v1";

const defaults: PreferencesData = {
  enterToSend: true,
  composerHintVisible: true,
  compactMessages: false,
  showMediaPreviews: true,
  inputDeviceId: "default",
  outputDeviceId: "default",
  inputVolume: 100,
  outputVolume: 100,
  interfaceSounds: true,
  interfaceSoundVolume: 65,
  soundboardVolume: 80,
  windowMaterial: "mica",
  acrylicDim: 55,
  keybinds: {
    toggleMute: "Ctrl+Shift+KeyM",
    toggleDeafen: "Ctrl+Shift+KeyD",
    leaveVoice: "Ctrl+Shift+KeyL",
    toggleScreenShare: "Ctrl+Shift+KeyS",
    openSoundboard: "Ctrl+Shift+KeyB",
  },
};

function loadPreferences(): PreferencesData {
  if (typeof window === "undefined") return defaults;
  try {
    const stored = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? "{}") as Partial<PreferencesData>;
    return {
      ...defaults,
      ...stored,
      keybinds: { ...defaults.keybinds, ...(stored.keybinds ?? {}) },
    };
  } catch {
    return defaults;
  }
}

export const usePreferences = create<PreferencesState>()((set, get) => ({
  ...loadPreferences(),
  setPreference: (key, value) => {
    set({ [key]: value } as Pick<PreferencesState, typeof key>);
    const { setPreference: _setPreference, ...data } = get();
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
  },
}));
