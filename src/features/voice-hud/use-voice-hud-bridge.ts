import { useEffect, useMemo, useState } from "react";
import { supabase } from "@/lib/supabase";
import { isTauri } from "@/lib/tauri";
import { hideVoiceHud, resizeVoiceHud, showVoiceHud, updateVoiceHud, type VoiceHudParticipant, type VoiceHudMessage } from "@/lib/voice-hud";
import { relayVoiceHudStream } from "@/lib/voice-hud-stream";
import type { TextDecoration } from "@/components/decorated-text";
import { useAuth } from "@/stores/auth";
import { usePreferences } from "@/stores/preferences";
import { useProfiles } from "@/stores/profiles";
import { useVoice } from "@/stores/voice";
import { subscribeToLiveChatMessages } from "@/stores/chat";

const EMPTY_PARTICIPANTS: never[] = [];

/** Call-owned, independent of which conversation or settings view is open. */
export function useVoiceHudBridge(): void {
  const userId = useAuth((state) => state.userId);
  const self = useAuth((state) => state.profile);
  const conversationId = useVoice((state) => state.activeConversationId);
  const sessionId = useVoice((state) => state.sessionId);
  const messageScope = `${userId}:${conversationId}:${sessionId}`;
  const [messages, setMessages] = useState<Record<string, VoiceHudMessage & { scope: string }>>({});
  const roster = useVoice((state) => conversationId ? state.participants[conversationId] ?? EMPTY_PARTICIPANTS : EMPTY_PARTICIPANTS);
  const local = useVoice((state) => state.localScreenStream);
  const remote = useVoice((state) => state.remoteScreenStream);
  const remoteScreenSessionId = useVoice((state) => state.remoteScreenSessionId);
  const speaking = useVoice((state) => state.speaking);
  const levels = useVoice((state) => state.level);
  const muted = useVoice((state) => state.muted);
  const deafened = useVoice((state) => state.deafened);
  const remoteMuted = useVoice((state) => state.remoteMuted);
  const remoteDeafened = useVoice((state) => state.remoteDeafened);
  const profiles = useProfiles((state) => state.byId);
  const scale = usePreferences((state) => state.voiceHudScale);
  const showNames = usePreferences((state) => state.voiceHudShowNames);
  const remoteId = roster.find((participant) => participant.user_id !== userId)?.user_id;
  const joined = Boolean(userId && conversationId);

  useEffect(() => {
    setMessages({});
    if (!isTauri || !joined || !conversationId) return;
    const seen = new Set<string>();
    return subscribeToLiveChatMessages((event) => {
      if (useAuth.getState().userId !== userId || useVoice.getState().sessionId !== sessionId) return;
      const message = event.kind === "delete" ? null : event.message;
      if (message && (message.conversation_id !== conversationId || message.message_kind !== "chat")) return;
      const id = message?.id ?? (event.kind === "delete" ? event.id : "");
      if (event.kind === "delete" || message?.deleted_at) {
        setMessages((current) => Object.fromEntries(Object.entries(current).filter(([, bubble]) => bubble.id !== id)));
        return;
      }
      if (!message || !(useVoice.getState().participants[conversationId] ?? []).some((participant) => participant.user_id === message.sender_id)) return;
      const text = (message.content || (message.media_kind === "image" ? "[Image]" : message.media_kind === "video" ? "[Video]" : "")).trim().slice(0, 500);
      if (!text) return;
      if (event.kind === "update") {
        setMessages((current) => current[message.sender_id]?.id === id ? { ...current, [message.sender_id]: { ...current[message.sender_id], text } } : current);
        return;
      }
      if (seen.has(id)) return;
      seen.add(id);
      if (seen.size > 128) seen.delete(seen.values().next().value!);
      setMessages((current) => ({ ...current, [message.sender_id]: { id, text, expiresAt: Date.now() + 10_000, scope: messageScope } }));
    });
  }, [joined, conversationId, sessionId, userId, messageScope]);

  useEffect(() => {
    const expiries = Object.values(messages).map((message) => message.expiresAt);
    if (!expiries.length) return;
    const timer = setTimeout(() => setMessages((current) => Object.fromEntries(Object.entries(current).filter(([, message]) => message.expiresAt > Date.now()))), Math.max(0, Math.min(...expiries) - Date.now()));
    return () => clearTimeout(timer);
  }, [messages]);

  useEffect(() => {
    if (isTauri && joined) void useProfiles.getState().ensure(roster.map((participant) => participant.user_id));
  }, [joined, roster]);

  const participants = useMemo<VoiceHudParticipant[]>(() => roster.map(({ user_id: id }) => {
    const isSelf = id === userId;
    const profile = isSelf ? self : profiles[id];
    const avatarPath = profile?.avatar_animated_path || profile?.avatar_path;
    const avatar = avatarPath ? supabase.storage.from("avatars").getPublicUrl(avatarPath).data.publicUrl : null;
    return {
      id,
      name: profile?.display_name || profile?.username || (isSelf ? "You" : "Partner"),
      avatar: avatar && profile?.avatar_updated_at ? `${avatar}?v=${encodeURIComponent(profile.avatar_updated_at)}` : avatar,
      avatarDecoration: profile?.avatar_decoration ?? null,
      nameColor: profile?.name_color ?? null,
      nameDecoration: (profile?.name_decoration ?? null) as TextDecoration | null,
      nameFont: profile?.name_font ?? null,
      nameWeight: profile?.name_weight ?? null,
      speaking: Boolean(speaking[id]),
      level: levels[id] ?? 0,
      muted: isSelf ? muted || deafened : Boolean(remoteMuted[id] || remoteDeafened[id]),
      screenId: isSelf ? local?.id ?? null : remote?.id ?? remoteScreenSessionId,
      message: messages[id]?.scope === messageScope ? { id: messages[id].id, text: messages[id].text, expiresAt: messages[id].expiresAt } : null,
    };
  }), [roster, userId, self, profiles, speaking, levels, muted, deafened, remoteMuted, remoteDeafened, local, remote, remoteScreenSessionId, messages, messageScope]);

  useEffect(() => {
    if (!isTauri) return;
    if (joined) void showVoiceHud(usePreferences.getState().voiceHudScale);
    else void hideVoiceHud();
    return () => { void hideVoiceHud(); };
  }, [joined]);
  useEffect(() => {
    if (isTauri && joined) void resizeVoiceHud(scale);
  }, [joined, scale]);
  useEffect(() => {
    if (!isTauri || !joined) return;
    const update = () => void updateVoiceHud({ participants, scale, showNames });
    update();
    // Replay covers a HUD that was still installing its listener on startup;
    // its watchdog clears the roster if this owning page goes away.
    const timer = setInterval(update, 1_000);
    return () => clearInterval(timer);
  }, [joined, participants, scale, showNames]);
  useEffect(() => {
    if (joined && userId && local) return relayVoiceHudStream(userId, local);
  }, [joined, userId, local]);
  useEffect(() => {
    if (joined && remoteId && remote) return relayVoiceHudStream(remoteId, remote);
  }, [joined, remoteId, remote]);
}
