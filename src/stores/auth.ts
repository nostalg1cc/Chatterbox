import { create } from "zustand";
import { supabase } from "@/lib/supabase";
import type { NameColor, Profile } from "@/lib/types";

type AuthStatus = "booting" | "signedOut" | "signedIn";

interface AuthState {
  status: AuthStatus;
  userId: string | null;
  email: string | null;
  profile: Profile | null;
  init: () => void;
  updateGeneralSettings: (name: string, nameColor: NameColor, decoration: string | null, nameDecoration: string | null) => Promise<void>;
  updateAvatar: (path: string) => Promise<void>;
  applyProfile: (profile: Profile) => void;
  signOut: () => Promise<void>;
}

let initialized = false;

export const useAuth = create<AuthState>()((set, get) => ({
  status: "booting",
  userId: null,
  email: null,
  profile: null,

  init: () => {
    if (initialized) return;
    initialized = true;

    supabase.auth.onAuthStateChange((_event, session) => {
      // Supabase warns against awaiting client calls inside this callback;
      // defer the profile fetch to the next tick.
      setTimeout(async () => {
        if (!session?.user) {
          set({ status: "signedOut", userId: null, email: null, profile: null });
          return;
        }
        const { userId, profile } = get();
        if (userId === session.user.id && profile) {
          set({ email: session.user.email ?? null });
          return;
        }
        const { data } = await supabase
          .from("profiles")
          .select("*")
          .eq("id", session.user.id)
          .maybeSingle();
        set({
          status: "signedIn",
          userId: session.user.id,
          email: session.user.email ?? null,
          profile: (data as Profile | null) ?? null,
        });
      }, 0);
    });
  },

  updateGeneralSettings: async (name, nameColor, decoration, nameDecoration) => {
    const { userId } = get();
    if (!userId) return;
    const { data, error } = await supabase
      .from("profiles")
      .update({ display_name: name, name_color: nameColor, avatar_decoration: decoration, name_decoration: nameDecoration })
      .eq("id", userId)
      .select()
      .single();
    if (error) throw new Error("Couldn't update your profile.");
    set({ profile: data as Profile });
  },

  updateAvatar: async (path) => {
    const { userId } = get();
    if (!userId) return;
    const { data, error } = await supabase
      .from("profiles")
      .update({
        avatar_path: path,
        avatar_updated_at: new Date().toISOString(),
      })
      .eq("id", userId)
      .select()
      .single();
    if (error) throw new Error("Couldn't update your avatar.");
    set({ profile: data as Profile });
  },

  applyProfile: (profile) => {
    if (profile.id === get().userId) set({ profile });
  },

  signOut: async () => {
    await supabase.auth.signOut();
  },
}));
