import { create } from "zustand";
import { supabase } from "@/lib/supabase";
import type { Profile } from "@/lib/types";

interface ProfilesState {
  byId: Record<string, Profile>;
  put: (profiles: Profile[]) => void;
  /** Fetch any profiles not yet cached. */
  ensure: (ids: string[]) => Promise<void>;
}

export const useProfiles = create<ProfilesState>()((set, get) => ({
  byId: {},

  put: (profiles) =>
    set((state) => {
      const byId = { ...state.byId };
      for (const p of profiles) byId[p.id] = p;
      return { byId };
    }),

  ensure: async (ids) => {
    const missing = [...new Set(ids)].filter((id) => id && !get().byId[id]);
    if (missing.length === 0) return;
    const { data } = await supabase.from("profiles").select("*").in("id", missing);
    if (data?.length) get().put(data as Profile[]);
  },
}));
