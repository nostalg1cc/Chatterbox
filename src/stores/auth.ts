import { create } from "zustand";
import { supabase } from "@/lib/supabase";
import type { Profile } from "@/lib/types";

type AuthStatus = "booting" | "signedOut" | "signedIn";

interface AuthState {
  status: AuthStatus;
  userId: string | null;
  email: string | null;
  profile: Profile | null;
  init: () => void;
  updateDisplayName: (name: string) => Promise<void>;
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
          // Token refresh for the already-loaded user; nothing to do.
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

  updateDisplayName: async (name) => {
    const { userId } = get();
    if (!userId) return;
    const { data, error } = await supabase
      .from("profiles")
      .update({ display_name: name })
      .eq("id", userId)
      .select()
      .single();
    if (error) throw new Error("Couldn't update your display name.");
    set({ profile: data as Profile });
  },

  signOut: async () => {
    await supabase.auth.signOut();
  },
}));
