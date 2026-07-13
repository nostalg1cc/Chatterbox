import { create } from "zustand";
import { supabase } from "@/lib/supabase";
import type { Profile } from "@/lib/types";

interface ProfilesState {
  byId: Record<string, Profile>;
  put: (profiles: Profile[]) => void;
  /** Fetch any profiles not yet cached. */
  ensure: (ids: string[]) => Promise<void>;
  subscribe: (onProfile: (profile: Profile) => void) => () => void;
  reset: () => void;
}

export const useProfiles = create<ProfilesState>()((set, get) => ({
  byId: {},

  put: (profiles) =>
    set((state) => {
      const byId = { ...state.byId };
      for (const profile of profiles) byId[profile.id] = profile;
      return { byId };
    }),

  ensure: async (ids) => {
    const missing = [...new Set(ids)].filter((id) => id && !get().byId[id]);
    if (missing.length === 0) return;
    const { data } = await supabase.from("profiles").select("*").in("id", missing);
    if (data?.length) get().put(data as Profile[]);
  },

  subscribe: (onProfile) => {
    const channel = supabase
      .channel("profile-changes")
      .on(
        "postgres_changes",
        { event: "UPDATE", schema: "public", table: "profiles" },
        (payload) => {
          const profile = payload.new as Profile;
          if (get().byId[profile.id]) get().put([profile]);
          onProfile(profile);
        }
      )
      .subscribe();

    return () => {
      void supabase.removeChannel(channel);
    };
  },

  reset: () => set({ byId: {} }),
}));
