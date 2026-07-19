import { toast } from "sonner";
import { create } from "zustand";
import { prepareSoundboardAudio, playSoundboardUrl, preloadSoundboardClips, type SoundboardPlayback } from "@/lib/soundboard-audio";
import { supabase } from "@/lib/supabase";
import { usePreferences } from "./preferences";
import { broadcastVoiceSoundboard, useVoice } from "./voice";

export interface SoundboardSound {
  id: string;
  owner_id: string;
  name: string;
  storage_path?: string;
  size_bytes: number;
  duration_ms: number;
  created_at: string;
}

interface SoundboardState {
  sounds: SoundboardSound[];
  availableSounds: SoundboardSound[];
  loading: boolean;
  uploading: boolean;
  cooldownUntil: number;
  signedUrls: Record<string, string>;
  playingSoundId: string | null;
  playbackProgress: number;
  playbackPaused: boolean;
  load: () => Promise<void>;
  loadAvailable: (conversationId: string) => Promise<void>;
  upload: (file: File, name: string) => Promise<void>;
  rename: (soundId: string, name: string) => Promise<void>;
  remove: (soundId: string) => Promise<void>;
  play: (soundId: string) => Promise<void>;
  preview: (soundId: string) => Promise<void>;
}

let localPlayback: SoundboardPlayback | null = null;

async function invoke<T>(body: Record<string, unknown>): Promise<T> {
  const { data, error } = await supabase.functions.invoke("soundboard-storage", { body });
  if (error) throw new Error(error.message);
  if (data?.error) throw new Error(data.error);
  return data as T;
}

export const useSoundboard = create<SoundboardState>()((set, get) => ({
  sounds: [],
  availableSounds: [],
  loading: false,
  uploading: false,
  cooldownUntil: 0,
  signedUrls: {},
  playingSoundId: null,
  playbackProgress: 0,
  playbackPaused: false,

  load: async () => {
    set({ loading: true });
    try {
      const data = await invoke<{ sounds: SoundboardSound[] }>({ mode: "list" });
      set({ sounds: data.sounds });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Soundboard could not load.");
    } finally {
      set({ loading: false });
    }
  },

  loadAvailable: async (conversationId) => {
    set({ loading: true });
    try {
      const data = await invoke<{ sounds: SoundboardSound[]; preloadUrls?: Record<string, string> }>({
        mode: "list",
        conversationId,
      });
      set((state) => ({
        availableSounds: data.sounds,
        signedUrls: { ...state.signedUrls, ...(data.preloadUrls ?? {}) },
      }));
      if (data.preloadUrls) {
        preloadSoundboardClips(Object.entries(data.preloadUrls).map(([id, signedUrl]) => ({ id, signedUrl })));
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Shared soundboard could not load.");
    } finally {
      set({ loading: false });
    }
  },

  upload: async (file, requestedName) => {
    const name = requestedName.trim().slice(0, 32);
    if (!name) throw new Error("Give the sound a name.");
    set({ uploading: true });
    const soundId = crypto.randomUUID();
    let reserved = false;
    try {
      const prepared = await prepareSoundboardAudio(file);
      const reservation = await invoke<{ path: string; token: string }>({ mode: "reserve", soundId, name, sizeBytes: prepared.blob.size, durationMs: prepared.durationMs });
      reserved = true;
      const { error } = await supabase.storage.from("soundboard").uploadToSignedUrl(reservation.path, reservation.token, prepared.blob, { contentType: "audio/webm", cacheControl: "31536000" });
      if (error) throw new Error(error.message);
      const data = await invoke<{ sound: SoundboardSound }>({ mode: "finalize", soundId, name, sizeBytes: prepared.blob.size, durationMs: prepared.durationMs });
      set((state) => ({ sounds: [data.sound, ...state.sounds], availableSounds: [data.sound, ...state.availableSounds.filter((sound) => sound.id !== data.sound.id)] }));
      toast.success("Sound added.");
    } catch (error) {
      if (reserved) void invoke({ mode: "discard", soundId }).catch(() => undefined);
      if (error instanceof Error && error.message.includes("Soundboard quota reached")) {
        throw new Error("Soundboard storage is full (16 MiB per user, 96 MiB shared).");
      }
      throw error;
    } finally {
      set({ uploading: false });
    }
  },

  rename: async (soundId, requestedName) => {
    const name = requestedName.trim().slice(0, 32);
    if (!name) throw new Error("Give the sound a name.");
    const { data, error } = await supabase.rpc("rename_soundboard_sound", { p_sound_id: soundId, p_name: name });
    if (error) throw new Error(error.message);
    const renamed = data as SoundboardSound | null;
    if (!renamed) throw new Error("Sound rename returned no data.");
    set((state) => ({
      sounds: state.sounds.map((sound) => sound.id === soundId ? renamed : sound),
      availableSounds: state.availableSounds.map((sound) => sound.id === soundId ? renamed : sound),
    }));
    toast.success("Sound renamed.");
  },

  remove: async (soundId) => {
    await invoke({ mode: "delete", soundId });
    set((state) => ({ sounds: state.sounds.filter((sound) => sound.id !== soundId), availableSounds: state.availableSounds.filter((sound) => sound.id !== soundId) }));
    toast.success("Sound removed.");
  },

  preview: async (soundId) => {
    try {
      const data = await invoke<{ signedUrl: string }>({ mode: "preview", soundId });
      await playSoundboardUrl(soundId, data.signedUrl, Date.now() + 20, usePreferences.getState().soundboardVolume, usePreferences.getState().outputDeviceId);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Sound preview could not play.");
    }
  },

  play: async (soundId) => {
    if (get().playingSoundId === soundId && localPlayback) {
      if (get().playbackPaused) {
        localPlayback.resume();
        set({ playbackPaused: false });
      } else {
        localPlayback.pause();
        set({ playbackPaused: true });
      }
      return;
    }

    const voice = useVoice.getState();
    if (!voice.activeConversationId) {
      toast.info("Join voice to use the soundboard.");
      return;
    }
    if (Date.now() < get().cooldownUntil) return;
    set({ cooldownUntil: Date.now() + 600 });

    try {
      let signedUrl = get().signedUrls[soundId];
      let sound = get().availableSounds.find((entry) => entry.id === soundId);
      let playAt = Date.now() + 120;

      if (!signedUrl || !sound) {
        const data = await invoke<{
          sound: { id: string; owner_id: string; name: string; duration_ms: number };
          signedUrl: string;
          playAt: number;
        }>({ mode: "play", soundId, conversationId: voice.activeConversationId });
        signedUrl = data.signedUrl;
        playAt = data.playAt;
        sound = data.sound as SoundboardSound;
        set((state) => ({ signedUrls: { ...state.signedUrls, [soundId]: signedUrl! } }));
      }

      const payload = {
        version: 1 as const,
        id: sound.id,
        name: sound.name,
        signedUrl,
        playAt,
        nonce: crypto.randomUUID(),
      };
      broadcastVoiceSoundboard(payload);

      const previous = localPlayback;
      localPlayback = null;
      previous?.stop();
      set({ playingSoundId: payload.id, playbackProgress: 0, playbackPaused: false });
      localPlayback = await playSoundboardUrl(
        payload.id,
        payload.signedUrl,
        payload.playAt,
        usePreferences.getState().soundboardVolume,
        usePreferences.getState().outputDeviceId,
        {
          onProgress: (playbackProgress) => {
            if (get().playingSoundId === payload.id) set({ playbackProgress });
          },
          onEnded: () => {
            if (get().playingSoundId === payload.id) {
              localPlayback = null;
              set({ playingSoundId: null, playbackProgress: 0, playbackPaused: false });
            }
          },
        }
      );
    } catch (error) {
      if (get().playingSoundId === soundId) {
        localPlayback = null;
        set({ playingSoundId: null, playbackProgress: 0, playbackPaused: false });
      }
      toast.error(error instanceof Error ? error.message : "Sound could not play.");
    }
  },
}));
