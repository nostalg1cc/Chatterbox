import { create } from "zustand";
import { toast } from "sonner";
import { playAppSound } from "@/lib/app-sounds";
import type { RealtimeChannel } from "@supabase/supabase-js";
import { supabase } from "@/lib/supabase";
import { deleteCachedMedia, putCachedMedia } from "@/lib/media-cache";
import type { PreparedMedia } from "@/lib/media";
import type {
  Conversation,
  ConversationOverview,
  Message,
  Reaction,
} from "@/lib/types";
import { useAuth } from "./auth";
import { useProfiles } from "./profiles";

const PAGE_SIZE = 50;

interface OverviewEntry {
  content: string | null;
  senderId: string | null;
  deleted: boolean;
  at: string | null;
}

export type ConversationChannel = "chat" | "media";

interface ChatState {
  view: "chat" | "friends";
  channel: ConversationChannel;
  activeId: string | null;
  conversations: Conversation[];
  overviews: Record<string, OverviewEntry>;
  unread: Record<string, number>;
  /** Messages per conversation, ascending by created_at. Absent key = not loaded yet. */
  messages: Record<string, Message[]>;
  /** Reactions keyed by message id. */
  reactions: Record<string, Reaction[]>;
  /** Referenced messages needed to render reply previews. */
  replyTargets: Record<string, Message>;
  /** The message currently selected as the composer reply target. */
  replyTo: Message | null;
  hasMore: Record<string, boolean>;
  loadingOlder: boolean;
  /** User id currently typing, per conversation. */
  typing: Record<string, string | null>;
  loaded: boolean;

  setView: (view: "chat" | "friends") => void;
  openConversation: (id: string) => void;
  openConversationChannel: (id: string, channel: ConversationChannel) => void;
  loadConversations: () => Promise<void>;
  loadMessages: (convId: string) => Promise<void>;
  loadOlder: (convId: string) => Promise<void>;
  sendMessage: (
    convId: string,
    content: string,
    media?: PreparedMedia,
    replyToMessageId?: string | null
  ) => Promise<boolean>;
  setReplyTo: (message: Message | null) => void;
  editMessage: (messageId: string, content: string) => Promise<void>;
  deleteMessage: (messageId: string) => Promise<void>;
  toggleReaction: (message: Message, emoji: string) => Promise<void>;
  markRead: (convId: string) => void;
  subscribe: () => () => void;
  joinTyping: (convId: string) => () => void;
  notifyTyping: (convId: string) => void;
  reset: () => void;
}

function counterpartId(conv: Conversation, myId: string): string {
  return conv.user1_id === myId ? conv.user2_id : conv.user1_id;
}

function sortConversations(list: Conversation[]): Conversation[] {
  return [...list].sort(
    (a, b) => new Date(b.last_message_at).getTime() - new Date(a.last_message_at).getTime()
  );
}

function insertSorted(list: Message[], msg: Message): Message[] {
  const idx = list.findIndex((m) => m.id === msg.id);
  if (idx !== -1) {
    const next = [...list];
    next[idx] = { ...next[idx], ...msg, pending: false };
    return next;
  }
  const next = [...list, msg];
  next.sort((a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime());
  return next;
}

// Module-level realtime bookkeeping (non-reactive).
const typingChannels = new Map<string, RealtimeChannel>();
const typingTimeouts = new Map<string, ReturnType<typeof setTimeout>>();
const typingLastSent = new Map<string, number>();

export const useChat = create<ChatState>()((set, get) => ({
  view: "chat",
  channel: "chat",
  activeId: null,
  conversations: [],
  overviews: {},
  unread: {},
  messages: {},
  reactions: {},
  replyTargets: {},
  replyTo: null,
  hasMore: {},
  loadingOlder: false,
  typing: {},
  loaded: false,

  setView: (view) => set({ view }),

  openConversation: (id) => {
    set({ activeId: id, view: "chat", channel: "chat", replyTo: null });
    void get().loadMessages(id);
    get().markRead(id);
  },

  openConversationChannel: (id, channel) => {
    set({ activeId: id, view: "chat", channel, replyTo: null });
    void get().loadMessages(id);
    get().markRead(id);
  },

  loadConversations: async () => {
    const [convRes, overviewRes] = await Promise.all([
      supabase.from("conversations").select("*"),
      supabase.rpc("conversation_overview"),
    ]);
    if (convRes.error || overviewRes.error) {
      toast.error("Couldn't load conversations.");
      return;
    }
    const conversations = sortConversations((convRes.data ?? []) as Conversation[]);
    const myId = useAuth.getState().userId ?? "";
    await useProfiles.getState().ensure(conversations.map((c) => counterpartId(c, myId)));

    const overviews: Record<string, OverviewEntry> = {};
    const unread: Record<string, number> = {};
    for (const o of (overviewRes.data ?? []) as ConversationOverview[]) {
      overviews[o.conversation_id] = {
        content: o.last_message_content,
        senderId: o.last_message_sender_id,
        deleted: Boolean(o.last_message_deleted),
        at: o.last_message_at,
      };
      unread[o.conversation_id] = Number(o.unread_count) || 0;
    }
    set({ conversations, overviews, unread, loaded: true });
  },

  loadMessages: async (convId) => {
    if (get().messages[convId]) return;
    const { data, error } = await supabase
      .from("messages")
      .select("*")
      .eq("conversation_id", convId)
      .order("created_at", { ascending: false })
      .limit(PAGE_SIZE);
    if (error) {
      toast.error("Couldn't load messages.");
      return;
    }
    const page = (data ?? []) as Message[];
    const list = [...page].reverse();
    set((s) => ({
      messages: { ...s.messages, [convId]: list },
      hasMore: { ...s.hasMore, [convId]: page.length === PAGE_SIZE },
    }));
    await loadReactionsFor(list.map((m) => m.id), set);
    await loadReplyTargets(list.map((m) => m.reply_to_message_id), set);
  },

  loadOlder: async (convId) => {
    const current = get().messages[convId];
    if (!current?.length || get().loadingOlder || !get().hasMore[convId]) return;
    set({ loadingOlder: true });
    const { data, error } = await supabase
      .from("messages")
      .select("*")
      .eq("conversation_id", convId)
      .lt("created_at", current[0].created_at)
      .order("created_at", { ascending: false })
      .limit(PAGE_SIZE);
    set({ loadingOlder: false });
    if (error || !data) return;
    const page = (data as Message[]).reverse();
    set((s) => ({
      messages: { ...s.messages, [convId]: [...page, ...(s.messages[convId] ?? [])] },
      hasMore: { ...s.hasMore, [convId]: data.length === PAGE_SIZE },
    }));
    await loadReactionsFor(page.map((m) => m.id), set);
    await loadReplyTargets(page.map((m) => m.reply_to_message_id), set);
  },

  sendMessage: async (convId, content, media, replyToMessageId = null) => {
    const myId = useAuth.getState().userId;
    const trimmed = content.trim();
    if (!myId || (!trimmed && !media) || trimmed.length > 4000) return false;

    const id = crypto.randomUUID();
    let mediaPath: string | null = null;
    const removeOptimistic = () => {
      set((s) => ({
        messages: {
          ...s.messages,
          [convId]: (s.messages[convId] ?? []).filter((message) => message.id !== id),
        },
      }));
    };

    const optimistic: Message = {
      id,
      conversation_id: convId,
      sender_id: myId,
      content: trimmed,
      created_at: new Date().toISOString(),
      edited_at: null,
      deleted_at: null,
      media_kind: media?.kind ?? null,
      media_path: null,
      media_mime_type: media?.mimeType ?? null,
      media_size_bytes: media?.blob.size ?? null,
      media_width: media?.width ?? null,
      media_height: media?.height ?? null,
      media_duration_seconds: media?.durationSeconds ?? null,
      media_expires_at: media ? new Date(Date.now() + 3 * 24 * 60 * 60 * 1000).toISOString() : null,
      media_deleted_at: null,
      reply_to_message_id: replyToMessageId,
      pending: true,
    };

    try {
      if (media) {
        const { data: reservation, error: reservationError } = await supabase.functions.invoke(
          "purge-chat-media",
          {
            body: {
              mode: "reserve",
              conversationId: convId,
              messageId: id,
              kind: media.kind,
              mimeType: media.mimeType,
              sizeBytes: media.blob.size,
            },
          }
        );
        if (
          reservationError ||
          typeof reservation?.path !== "string" ||
          typeof reservation?.token !== "string"
        ) {
          throw new Error(reservationError?.message ?? reservation?.error ?? "Upload could not start.");
        }

        mediaPath = reservation.path;
        const { error: uploadError } = await supabase.storage
          .from("chat-media")
          .uploadToSignedUrl(reservation.path, reservation.token, media.blob, {
            contentType: media.mimeType,
            cacheControl: "3600",
          });
        if (uploadError) throw new Error(uploadError.message);
        optimistic.media_path = mediaPath;
      }

      applyMessage(optimistic, set);

      const { data, error } = await supabase
        .from("messages")
        .insert({
          id,
          conversation_id: convId,
          sender_id: myId,
          content: trimmed,
          media_kind: media?.kind ?? null,
          media_path: mediaPath,
          media_mime_type: media?.mimeType ?? null,
          media_size_bytes: media?.blob.size ?? null,
          media_width: media?.width ?? null,
          media_height: media?.height ?? null,
          media_duration_seconds: media?.durationSeconds ?? null,
          reply_to_message_id: replyToMessageId,
        })
        .select()
        .single();
      if (error) throw new Error(error.message);

      const sentMessage = data as Message;
      applyMessage(sentMessage, set);
      if (media) {
        void putCachedMedia({
          userId: myId,
          messageId: id,
          blob: media.blob,
          mimeType: media.mimeType,
          createdAt: sentMessage.created_at,
        }).catch((error) => console.warn("Local media cache write failed", error));
      }
      if (mediaPath) {
        void supabase.functions.invoke("purge-chat-media", {
          body: { mode: "finalize", path: mediaPath },
        });
      }
      return true;
    } catch (error) {
      removeOptimistic();
      if (mediaPath) {
        void supabase.functions.invoke("purge-chat-media", {
          body: { mode: "discard", path: mediaPath },
        });
      }
      toast.error(error instanceof Error ? error.message : "Message didn't send.");
      return false;
    }
  },

  setReplyTo: (message) => set({ replyTo: message }),

  editMessage: async (messageId, content) => {
    const trimmed = content.trim();
    if (!trimmed || trimmed.length > 4000) return;
    const { data, error } = await supabase
      .from("messages")
      .update({ content: trimmed, edited_at: new Date().toISOString() })
      .eq("id", messageId)
      .select()
      .single();
    if (error) {
      toast.error("Couldn't edit the message.");
      return;
    }
    applyMessage(data as Message, set);
  },

  deleteMessage: async (messageId) => {
    const { data, error } = await supabase
      .from("messages")
      .update({ deleted_at: new Date().toISOString() })
      .eq("id", messageId)
      .select()
      .single();
    if (error) {
      toast.error("Couldn't delete the message.");
      return;
    }
    applyMessage(data as Message, set);
  },

  toggleReaction: async (message, emoji) => {
    const myId = useAuth.getState().userId;
    if (!myId) return;
    const existing = (get().reactions[message.id] ?? []).find(
      (r) => r.user_id === myId && r.emoji === emoji
    );

    if (existing) {
      set((s) => ({
        reactions: {
          ...s.reactions,
          [message.id]: (s.reactions[message.id] ?? []).filter((r) => r.id !== existing.id),
        },
      }));
      const { error } = await supabase.from("reactions").delete().eq("id", existing.id);
      if (error) {
        applyReaction(existing, set);
        toast.error("Couldn't remove the reaction.");
      }
      return;
    }

    const optimistic: Reaction = {
      id: crypto.randomUUID(),
      message_id: message.id,
      user_id: myId,
      emoji,
      created_at: new Date().toISOString(),
    };
    applyReaction(optimistic, set);
    const { error } = await supabase.from("reactions").insert({
      id: optimistic.id,
      message_id: message.id,
      user_id: myId,
      emoji,
    });
    if (error && error.code !== "23505") {
      removeReaction(optimistic.id, message.id, set);
      toast.error("Couldn't add the reaction.");
    }
  },

  markRead: (convId) => {
    const myId = useAuth.getState().userId;
    if (!myId) return;
    set((s) => ({ unread: { ...s.unread, [convId]: 0 } }));
    void supabase
      .from("conversation_reads")
      .upsert(
        { conversation_id: convId, user_id: myId, last_read_at: new Date().toISOString() },
        { onConflict: "conversation_id,user_id" }
      )
      .then(({ error }) => {
        if (error) console.error("markRead failed", error);
      });
  },

  subscribe: () => {
    const channel = supabase
      .channel("chat-changes")
      .on(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "messages" },
        (payload) => {
          const msg = payload.new as Message;
          const myId = useAuth.getState().userId;
          applyMessage(msg, set);
          if (msg.reply_to_message_id) void loadReplyTargets([msg.reply_to_message_id], set);
          const { activeId, view } = get();
          const isViewing =
            activeId === msg.conversation_id && view === "chat" && document.hasFocus();
          if (msg.sender_id !== myId) {
            if (isViewing) {
              get().markRead(msg.conversation_id);
            } else {
              playAppSound("notification_single");
              set((s) => ({
                unread: {
                  ...s.unread,
                  [msg.conversation_id]: (s.unread[msg.conversation_id] ?? 0) + 1,
                },
              }));
            }
          }
        }
      )
      .on(
        "postgres_changes",
        { event: "UPDATE", schema: "public", table: "messages" },
        (payload) => {
          applyMessage(payload.new as Message, set, { bump: false });
        }
      )
      .on(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "reactions" },
        (payload) => {
          applyReaction(payload.new as Reaction, set);
        }
      )
      .on(
        "postgres_changes",
        { event: "DELETE", schema: "public", table: "reactions" },
        (payload) => {
          const old = payload.old as Partial<Reaction>;
          if (old.id && old.message_id) removeReaction(old.id, old.message_id, set);
        }
      )
      .on(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "conversations" },
        async (payload) => {
          const conv = payload.new as Conversation;
          const myId = useAuth.getState().userId ?? "";
          await useProfiles.getState().ensure([counterpartId(conv, myId)]);
          set((s) => ({
            conversations: sortConversations([
              ...s.conversations.filter((c) => c.id !== conv.id),
              conv,
            ]),
          }));
        }
      )
      .on(
        "postgres_changes",
        { event: "UPDATE", schema: "public", table: "conversations" },
        (payload) => {
          const conv = payload.new as Conversation;
          set((s) => ({
            conversations: sortConversations([
              ...s.conversations.filter((c) => c.id !== conv.id),
              conv,
            ]),
          }));
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  },

  joinTyping: (convId) => {
    const myId = useAuth.getState().userId;
    if (typingChannels.has(convId)) {
      // Already joined (e.g. StrictMode double-mount); reuse it.
    } else {
      const channel = supabase.channel(`typing:${convId}`);
      channel
        .on("broadcast", { event: "typing" }, (payload) => {
          const userId = (payload.payload as { userId?: string })?.userId;
          if (!userId || userId === myId) return;
          set((s) => ({ typing: { ...s.typing, [convId]: userId } }));
          clearTimeout(typingTimeouts.get(convId));
          typingTimeouts.set(
            convId,
            setTimeout(() => {
              set((s) => ({ typing: { ...s.typing, [convId]: null } }));
            }, 3000)
          );
        })
        .subscribe();
      typingChannels.set(convId, channel);
    }

    return () => {
      const channel = typingChannels.get(convId);
      if (channel) {
        supabase.removeChannel(channel);
        typingChannels.delete(convId);
      }
      clearTimeout(typingTimeouts.get(convId));
      typingTimeouts.delete(convId);
      typingLastSent.delete(convId);
      set((s) => ({ typing: { ...s.typing, [convId]: null } }));
    };
  },

  notifyTyping: (convId) => {
    const myId = useAuth.getState().userId;
    const channel = typingChannels.get(convId);
    if (!myId || !channel) return;
    const last = typingLastSent.get(convId) ?? 0;
    if (Date.now() - last < 2000) return;
    typingLastSent.set(convId, Date.now());
    void channel.send({
      type: "broadcast",
      event: "typing",
      payload: { userId: myId },
    });
  },

  reset: () => {
    for (const channel of typingChannels.values()) supabase.removeChannel(channel);
    typingChannels.clear();
    for (const t of typingTimeouts.values()) clearTimeout(t);
    typingTimeouts.clear();
    typingLastSent.clear();
    set({
      view: "chat",
      channel: "chat",
      activeId: null,
      conversations: [],
      overviews: {},
      unread: {},
      messages: {},
      reactions: {},
      replyTargets: {},
      replyTo: null,
      hasMore: {},
      typing: {},
      loaded: false,
    });
  },
}));

type SetChat = (
  updater: (state: ChatState) => Partial<ChatState>
) => void;

/** Insert/replace a message, and (optionally) bump the conversation + overview. */
function applyMessage(msg: Message, set: SetChat, opts: { bump?: boolean } = {}) {
  const bump = opts.bump ?? true;
  const userId = useAuth.getState().userId;
  if (msg.deleted_at && userId && msg.media_kind) {
    void deleteCachedMedia(userId, msg.id).catch((error) =>
      console.warn("Local media cache delete failed", error)
    );
  }
  set((s) => {
    const patch: Partial<ChatState> = {};
    if (s.messages[msg.conversation_id]) {
      patch.messages = {
        ...s.messages,
        [msg.conversation_id]: insertSorted(s.messages[msg.conversation_id], msg),
      };
    }
    const existingOverview = s.overviews[msg.conversation_id];
    const isNewer =
      !existingOverview?.at || new Date(msg.created_at) >= new Date(existingOverview.at);
    if (isNewer) {
      patch.overviews = {
        ...s.overviews,
        [msg.conversation_id]: {
          content: msg.content || (msg.media_kind === "image" ? "[Image]" : msg.media_kind === "video" ? "[Video]" : ""),
          senderId: msg.sender_id,
          deleted: msg.deleted_at !== null,
          at: msg.created_at,
        },
      };
    }
    if (bump && isNewer) {
      patch.conversations = sortConversations(
        s.conversations.map((c) =>
          c.id === msg.conversation_id ? { ...c, last_message_at: msg.created_at } : c
        )
      );
    }
    if (s.replyTargets[msg.id]) {
      patch.replyTargets = { ...s.replyTargets, [msg.id]: msg };
    }
    return patch;
  });
}

function applyReaction(reaction: Reaction, set: SetChat) {
  set((s) => {
    const list = s.reactions[reaction.message_id] ?? [];
    if (list.some((r) => r.id === reaction.id)) return {};
    // A locally-toggled reaction may exist under an optimistic id with the
    // same (user, emoji); replace it instead of duplicating.
    const filtered = list.filter(
      (r) => !(r.user_id === reaction.user_id && r.emoji === reaction.emoji)
    );
    return {
      reactions: { ...s.reactions, [reaction.message_id]: [...filtered, reaction] },
    };
  });
}

function removeReaction(id: string, messageId: string, set: SetChat) {
  set((s) => ({
    reactions: {
      ...s.reactions,
      [messageId]: (s.reactions[messageId] ?? []).filter((r) => r.id !== id),
    },
  }));
}

async function loadReactionsFor(messageIds: string[], set: SetChat) {
  if (messageIds.length === 0) return;
  const { data } = await supabase.from("reactions").select("*").in("message_id", messageIds);
  if (!data) return;
  set((s) => {
    const reactions = { ...s.reactions };
    for (const id of messageIds) reactions[id] = [];
    for (const r of data as Reaction[]) {
      reactions[r.message_id] = [...(reactions[r.message_id] ?? []), r];
    }
    return { reactions };
  });
}

async function loadReplyTargets(replyIds: Array<string | null>, set: SetChat) {
  const ids = [...new Set(replyIds.filter((id): id is string => Boolean(id)))];
  if (ids.length === 0) return;
  const { data, error } = await supabase.from("messages").select("*").in("id", ids);
  if (error || !data) return;
  const targets = data as Message[];
  set((s) => ({
    replyTargets: {
      ...s.replyTargets,
      ...Object.fromEntries(targets.map((message) => [message.id, message])),
    },
  }));
}
