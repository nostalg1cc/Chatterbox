import { create } from "zustand";
import { toast } from "sonner";
import { supabase } from "@/lib/supabase";
import type { Friendship } from "@/lib/types";
import { useProfiles } from "./profiles";
import { useChat } from "./chat";

interface FriendsState {
  friendships: Friendship[];
  loaded: boolean;
  load: () => Promise<void>;
  /** Throws an Error with a user-facing message on failure. */
  sendRequest: (usernameRaw: string, myId: string, myUsername: string) => Promise<void>;
  accept: (id: string) => Promise<void>;
  /** Decline (incoming), cancel (outgoing), or unfriend — all delete the row. */
  removeFriendship: (id: string) => Promise<void>;
  subscribe: (myId: string) => () => void;
}

function upsertLocal(list: Friendship[], f: Friendship): Friendship[] {
  const idx = list.findIndex((x) => x.id === f.id);
  if (idx === -1) return [...list, f];
  const next = [...list];
  next[idx] = f;
  return next;
}

export const useFriends = create<FriendsState>()((set, get) => ({
  friendships: [],
  loaded: false,

  load: async () => {
    const { data, error } = await supabase
      .from("friendships")
      .select("*")
      .order("created_at", { ascending: false });
    if (error) {
      toast.error("Couldn't load your friends.");
      return;
    }
    const friendships = (data ?? []) as Friendship[];
    await useProfiles
      .getState()
      .ensure(friendships.flatMap((f) => [f.requester_id, f.addressee_id]));
    set({ friendships, loaded: true });
  },

  sendRequest: async (usernameRaw, myId, myUsername) => {
    const username = usernameRaw.trim().toLowerCase();
    if (!/^[a-z0-9_]{3,20}$/.test(username)) {
      throw new Error("Usernames are 3–20 characters: a–z, 0–9, underscores.");
    }
    if (username === myUsername) {
      throw new Error("That's your own username.");
    }

    const { data: profile, error: lookupError } = await supabase
      .from("profiles")
      .select("*")
      .eq("username", username)
      .maybeSingle();
    if (lookupError) throw new Error("Lookup failed. Try again.");
    if (!profile) throw new Error(`No one goes by @${username}.`);

    const existing = get().friendships.find(
      (f) =>
        (f.requester_id === profile.id && f.addressee_id === myId) ||
        (f.requester_id === myId && f.addressee_id === profile.id)
    );
    if (existing) {
      if (existing.status === "accepted") throw new Error("You're already friends.");
      if (existing.status === "pending") {
        throw new Error(
          existing.requester_id === myId
            ? "Request already sent — it's pending."
            : `@${username} already sent you a request. Check Pending.`
        );
      }
      throw new Error("You can't send a request to this user.");
    }

    const { data, error } = await supabase
      .from("friendships")
      .insert({ requester_id: myId, addressee_id: profile.id })
      .select()
      .single();
    if (error) {
      if (error.code === "23505") throw new Error("A request between you already exists.");
      throw new Error("Couldn't send the request. Try again.");
    }
    useProfiles.getState().put([profile]);
    set((s) => ({ friendships: upsertLocal(s.friendships, data as Friendship) }));
  },

  accept: async (id) => {
    const { data, error } = await supabase
      .from("friendships")
      .update({ status: "accepted" })
      .eq("id", id)
      .select()
      .single();
    if (error) {
      toast.error("Couldn't accept the request.");
      return;
    }
    set((s) => ({ friendships: upsertLocal(s.friendships, data as Friendship) }));
    // The accept trigger just created the conversation; pull it in.
    void useChat.getState().loadConversations();
  },

  removeFriendship: async (id) => {
    const { error } = await supabase.from("friendships").delete().eq("id", id);
    if (error) {
      toast.error("Couldn't remove this request.");
      return;
    }
    set((s) => ({ friendships: s.friendships.filter((f) => f.id !== id) }));
  },

  subscribe: (myId) => {
    const channel = supabase
      .channel("friendships-changes")
      .on(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "friendships" },
        async (payload) => {
          const f = payload.new as Friendship;
          set((s) => ({ friendships: upsertLocal(s.friendships, f) }));
          if (f.addressee_id === myId && f.status === "pending") {
            await useProfiles.getState().ensure([f.requester_id]);
            const requester = useProfiles.getState().byId[f.requester_id];
            toast(`${requester?.display_name ?? "Someone"} sent you a friend request`);
          }
        }
      )
      .on(
        "postgres_changes",
        { event: "UPDATE", schema: "public", table: "friendships" },
        async (payload) => {
          const f = payload.new as Friendship;
          const before = get().friendships.find((x) => x.id === f.id);
          set((s) => ({ friendships: upsertLocal(s.friendships, f) }));
          if (
            f.status === "accepted" &&
            before?.status !== "accepted" &&
            f.requester_id === myId
          ) {
            await useProfiles.getState().ensure([f.addressee_id]);
            const friend = useProfiles.getState().byId[f.addressee_id];
            toast(`${friend?.display_name ?? "Someone"} accepted your friend request`);
            void useChat.getState().loadConversations();
          }
        }
      )
      .on(
        "postgres_changes",
        { event: "DELETE", schema: "public", table: "friendships" },
        (payload) => {
          const old = payload.old as Partial<Friendship>;
          if (!old.id) return;
          set((s) => ({ friendships: s.friendships.filter((f) => f.id !== old.id) }));
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  },
}));
