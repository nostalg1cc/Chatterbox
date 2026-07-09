import { create } from "zustand";
import type { RealtimeChannel } from "@supabase/supabase-js";
import { supabase } from "@/lib/supabase";

interface PresenceState {
  online: Record<string, true>;
  join: (userId: string) => () => void;
}

let channel: RealtimeChannel | null = null;

export const usePresence = create<PresenceState>()((set) => ({
  online: {},

  join: (userId) => {
    if (channel) supabase.removeChannel(channel);
    channel = supabase.channel("online", {
      config: { presence: { key: userId } },
    });

    channel
      .on("presence", { event: "sync" }, () => {
        const state = channel?.presenceState() ?? {};
        const online: Record<string, true> = {};
        for (const key of Object.keys(state)) online[key] = true;
        set({ online });
      })
      .subscribe(async (status) => {
        if (status === "SUBSCRIBED") {
          await channel?.track({ online_at: new Date().toISOString() });
        }
      });

    return () => {
      if (channel) {
        supabase.removeChannel(channel);
        channel = null;
      }
      set({ online: {} });
    };
  },
}));

export function useIsOnline(userId: string | undefined): boolean {
  return usePresence((s) => (userId ? Boolean(s.online[userId]) : false));
}
