import { toast } from "sonner";
import { create } from "zustand";
import { prepareSoundboardAudio, playSoundboardUrl } from "@/lib/soundboard-audio";
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
  load: () => Promise<void>;
  loadAvailable: (conversationId: string) => Promise<void>;
  upload: (file: File, name: string) => Promise<void>;
  remove: (soundId: string) => Promise<void>;
  play: (soundId: string) => Promise<void>;
}

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
      const data = await invoke<{ sounds: SoundboardSound[] }>({
        mode: "list",
        conversationId,
      });
      set({ availableSounds: data.sounds });
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
      const reservation = await invoke<{ path: string; token: string }>({
        mode: "reserve",
        soundId,
        name,
        sizeBytes: prepared.blob.size,
        durationMs: prepared.durationMs,
      });
      reserved = true;
      const { error } = await supabase.storage
        .from("soundboard")
        .uploadToSignedUrl(reservation.path, reservation.token, prepared.blob, {
          contentType: "audio/webm",
          cacheControl: "31536000",
        });
      if (error) throw new Error(error.message);
      const data = await invoke<{ sound: SoundboardSound }>({
        mode: "finalize",
        soundId,
        name,
        sizeBytes: prepared.blob.size,
        durationMs: prepared.durationMs,
      });
      set((state) => ({
        sounds: [data.sound, ...state.sounds],
        availableSounds: [
          data.sound,
          ...state.availableSounds.filter((sound) => sound.id !== data.sound.id),
        ],
      }));
      toast.success("Sound added.");
    } catch (error) {
      if (reserved) {
        void invoke({ mode: "discard", soundId }).catch(() => undefined);
      }
      throw error;
    } finally {
      set({ uploading: false });
    }
  },

  remove: async (soundId) => {
    await invoke({ mode: "delete", soundId });
    set((state) => ({
      sounds: state.sounds.filter((sound) => sound.id !== soundId),
      availableSounds: state.availableSounds.filter((sound) => sound.id !== soundId),
    }));
    toast.success("Sound removed.");
  },

  play: async (soundId) => {
    const voice = useVoice.getState();
    if (!voice.activeConversationId) {
      toast.info("Join voice to use the soundboard.");
      return;
    }
    if (Date.now() < get().cooldownUntil) return;
    set({ cooldownUntil: Date.now() + 1200 });
    try {
      const data = await invoke<{
        sound: { id: string; owner_id: string; name: string; duration_ms: number };
        signedUrl: string;
        playAt: number;
      }>({
        mode: "play",
        soundId,
        conversationId: voice.activeConversationId,
      });
      const payload = {
        version: 1 as const,
        id: data.sound.id,
        name: data.sound.name,
        signedUrl: data.signedUrl,
        playAt: data.playAt,
        nonce: crypto.randomUUID(),
      };
      broadcastVoiceSoundboard(payload);
      await playSoundboardUrl(
        payload.id,
        payload.signedUrl,
        payload.playAt,
        usePreferences.getState().soundboardVolume
      );
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Sound could not play.");
    }
  },
}));