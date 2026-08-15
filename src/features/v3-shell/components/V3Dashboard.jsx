import { useEffect, useMemo, useState } from "react";
import { Check, MessageCircle, Search, UserPlus, X } from "lucide-react";
import { decorationUrl } from "@/lib/avatar-decorations";
import { supabase } from "@/lib/supabase";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useFriends } from "@/stores/friends";
import { usePresenceStatus } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";

function nameFor(profile) {
  return profile?.display_name?.trim() || profile?.username?.trim() || "Unknown";
}

function avatarFor(profile) {
  const path = profile?.avatar_animated_path || profile?.avatar_path;
  if (!path) return null;
  const publicUrl = supabase.storage.from("avatars").getPublicUrl(path).data.publicUrl;
  return profile.avatar_updated_at ? `${publicUrl}?v=${encodeURIComponent(profile.avatar_updated_at)}` : publicUrl;
}

function excerptFor(overview) {
  if (!overview?.last_message_id) return "No messages yet";
  if (overview.last_message_deleted) return "Message deleted";
  return overview.last_message_content || "Attachment";
}

function Avatar({ profile, presence }) {
  const avatar = avatarFor(profile);
  const decoration = decorationUrl(profile?.avatar_decoration, false);
  return (
    <span className="v3-dashboard__avatar">
      {avatar ? <img src={avatar} alt="" /> : <span className="v3-dashboard__avatar-fallback">{nameFor(profile).slice(0, 1).toUpperCase()}</span>}
      {decoration && <img className="v3-dashboard__avatar-decoration" src={decoration} alt="" />}
      {presence && <span className={"v3-dashboard__presence is-" + presence} aria-hidden="true" />}
    </span>
  );
}

function AddFriend() {
  const userId = useAuth((state) => state.userId);
  const username = useAuth((state) => state.profile?.username);
  const [value, setValue] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState(null);

  async function submit(event) {
    event.preventDefault();
    if (!value.trim() || !userId || !username) return;
    setSending(true);
    setError(null);
    try {
      await useFriends.getState().sendRequest(value, userId, username);
      setValue("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Couldn't send the request.");
    } finally {
      setSending(false);
    }
  }

  return (
    <form className="v3-dashboard__add-friend" onSubmit={submit}>
      <div className="v3-dashboard__add-friend-input">
        <Search aria-hidden="true" />
        <input value={value} onChange={(event) => { setValue(event.target.value); setError(null); }} placeholder="Add a friend by username" autoComplete="off" />
      </div>
      <button type="submit" disabled={sending || !value.trim()}>
        <UserPlus aria-hidden="true" />
        Send
      </button>
      {error && <p className="v3-dashboard__add-friend-error">{error}</p>}
    </form>
  );
}

export function V3Dashboard() {
  const userId = useAuth((state) => state.userId);
  const friendships = useFriends((state) => state.friendships);
  const conversations = useChat((state) => state.conversations);
  const overviews = useChat((state) => state.overviews);
  const unread = useChat((state) => state.unread);
  const profiles = useProfiles((state) => state.byId);
  const activeId = useChat((state) => state.activeId);

  const incoming = useMemo(() => friendships.filter((f) => f.status === "pending" && f.addressee_id === userId), [friendships, userId]);
  const outgoing = useMemo(() => friendships.filter((f) => f.status === "pending" && f.requester_id === userId), [friendships, userId]);
  const friends = useMemo(() => friendships
    .filter((f) => f.status === "accepted")
    .map((f) => (f.requester_id === userId ? f.addressee_id : f.requester_id)), [friendships, userId]);

  const conversationItems = useMemo(() => conversations.map((conversation) => ({
    conversation,
    partnerId: conversation.user1_id === userId ? conversation.user2_id : conversation.user1_id,
  })), [conversations, userId]);

  useEffect(() => {
    const ids = new Set([
      ...incoming.map((f) => f.requester_id),
      ...outgoing.map((f) => f.addressee_id),
      ...friends,
      ...conversationItems.map((item) => item.partnerId),
    ]);
    const missing = [...ids].filter((id) => id && !profiles[id]);
    if (missing.length) void useProfiles.getState().ensure(missing);
  }, [incoming, outgoing, friends, conversationItems, profiles]);

  function openConversationWith(friendId) {
    const item = conversationItems.find((entry) => entry.partnerId === friendId);
    if (!item) return;
    useChat.getState().openConversation(item.conversation.id);
  }

  return (
    <div className="v3-dashboard">
      <div className="v3-dashboard__inner">
        <header className="v3-dashboard__header">
          <h1>Dashboard</h1>
          {activeId && (
            <button type="button" className="v3-dashboard__back" onClick={() => useChat.getState().setView("chat")}>
              <MessageCircle aria-hidden="true" />
              Back to chat
            </button>
          )}
        </header>

        <section className="v3-dashboard__section">
          <p className="v3-dashboard__section-title">Add a friend</p>
          <AddFriend />
        </section>

        {incoming.length > 0 && (
          <section className="v3-dashboard__section">
            <p className="v3-dashboard__section-title">Friend requests</p>
            <div className="v3-dashboard__list">
              {incoming.map((request) => {
                const profile = profiles[request.requester_id];
                return (
                  <div key={request.id} className="v3-dashboard__row">
                    <Avatar profile={profile} />
                    <span className="v3-dashboard__row-name">{nameFor(profile)}</span>
                    <span className="v3-dashboard__row-actions">
                      <button type="button" aria-label="Accept" className="v3-dashboard__accept" onClick={() => void useFriends.getState().accept(request.id)}><Check aria-hidden="true" /></button>
                      <button type="button" aria-label="Decline" onClick={() => void useFriends.getState().removeFriendship(request.id)}><X aria-hidden="true" /></button>
                    </span>
                  </div>
                );
              })}
            </div>
          </section>
        )}

        {outgoing.length > 0 && (
          <section className="v3-dashboard__section">
            <p className="v3-dashboard__section-title">Pending</p>
            <div className="v3-dashboard__list">
              {outgoing.map((request) => {
                const profile = profiles[request.addressee_id];
                return (
                  <div key={request.id} className="v3-dashboard__row">
                    <Avatar profile={profile} />
                    <span className="v3-dashboard__row-name">{nameFor(profile)}</span>
                    <span className="v3-dashboard__row-actions">
                      <button type="button" aria-label="Cancel request" onClick={() => void useFriends.getState().removeFriendship(request.id)}><X aria-hidden="true" /></button>
                    </span>
                  </div>
                );
              })}
            </div>
          </section>
        )}

        <section className="v3-dashboard__section">
          <p className="v3-dashboard__section-title">Friends {friends.length > 0 && `— ${friends.length}`}</p>
          <div className="v3-dashboard__list">
            {friends.map((friendId) => {
              const profile = profiles[friendId];
              return (
                <FriendListRow key={friendId} profile={profile} onClick={() => openConversationWith(friendId)} />
              );
            })}
            {friends.length === 0 && <span className="v3-dashboard__empty">No friends yet — add one above.</span>}
          </div>
        </section>

        <section className="v3-dashboard__section">
          <p className="v3-dashboard__section-title">All chats</p>
          <div className="v3-dashboard__list">
            {conversationItems.map(({ conversation, partnerId }) => {
              const profile = profiles[partnerId];
              const overview = overviews[conversation.id];
              const unreadCount = unread[conversation.id] ?? 0;
              return (
                <button
                  key={conversation.id}
                  type="button"
                  className={"v3-dashboard__row v3-dashboard__row--chat" + (conversation.id === activeId ? " is-active" : "")}
                  onClick={() => useChat.getState().openConversation(conversation.id)}
                >
                  <Avatar profile={profile} />
                  <span className="v3-dashboard__row-copy">
                    <strong>{nameFor(profile)}</strong>
                    <span>{excerptFor(overview)}</span>
                  </span>
                  {unreadCount > 0 && <span className="v3-dashboard__unread">{unreadCount}</span>}
                </button>
              );
            })}
            {conversationItems.length === 0 && <span className="v3-dashboard__empty">No conversations yet.</span>}
          </div>
        </section>
      </div>
    </div>
  );
}

function FriendListRow({ profile, onClick }) {
  const presence = usePresenceStatus(profile?.id);
  return (
    <button type="button" className="v3-dashboard__row" onClick={onClick}>
      <Avatar profile={profile} presence={presence} />
      <span className="v3-dashboard__row-name">{nameFor(profile)}</span>
    </button>
  );
}
