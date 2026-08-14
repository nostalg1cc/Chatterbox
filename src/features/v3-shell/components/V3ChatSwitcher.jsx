import { useEffect, useMemo, useRef, useState } from "react";
import { LayoutDashboard } from "lucide-react";
import { decorationUrl } from "@/lib/avatar-decorations";
import { supabase } from "@/lib/supabase";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useProfiles } from "@/stores/profiles";
import { useUiSounds } from "../hooks/useUiSounds";

function nameFor(profile) {
  return profile?.display_name?.trim() || profile?.username?.trim() || "Unknown";
}

function avatarFor(profile) {
  const path = profile?.avatar_animated_path || profile?.avatar_path;
  if (!path) return null;
  const publicUrl = supabase.storage.from("avatars").getPublicUrl(path).data.publicUrl;
  return profile.avatar_updated_at ? `${publicUrl}?v=${encodeURIComponent(profile.avatar_updated_at)}` : publicUrl;
}

export function V3ChatSwitcher({ partnerProfile, partnerPresence }) {
  const userId = useAuth((state) => state.userId);
  const activeId = useChat((state) => state.activeId);
  const conversations = useChat((state) => state.conversations);
  const profiles = useProfiles((state) => state.byId);
  const [open, setOpen] = useState(false);
  const rootRef = useRef(null);
  const uiSounds = useUiSounds();
  const items = useMemo(() => conversations.map((conversation) => ({
    conversation,
    partnerId: conversation.user1_id === userId ? conversation.user2_id : conversation.user1_id,
  })), [conversations, userId]);

  useEffect(() => {
    const missing = items.map((item) => item.partnerId).filter((id) => id && !profiles[id]);
    if (missing.length) void useProfiles.getState().ensure(missing);
  }, [items, profiles]);

  useEffect(() => {
    const onPointerDown = (event) => {
      if (rootRef.current && !rootRef.current.contains(event.target)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, []);

  const currentAvatar = avatarFor(partnerProfile);
  const currentDecoration = decorationUrl(partnerProfile?.avatar_decoration, false);
  return (
    <div className="v3-chat-switcher" ref={rootRef}>
      <button type="button" className="icon-button partner-avatar-button v3-chat-switcher__trigger" aria-label="Switch conversation" aria-expanded={open} onPointerEnter={() => uiSounds.hover()} onClick={() => { setOpen((value) => !value); uiSounds.click(); }}>
        {currentAvatar ? <img className="partner-avatar-button__image" src={currentAvatar} alt="" /> : <span className="partner-avatar-button__fallback">{nameFor(partnerProfile).slice(0, 1).toUpperCase()}</span>}
        {currentDecoration && <img className="partner-avatar-button__decoration" src={currentDecoration} alt="" />}
        <span className={`partner-avatar-button__presence is-${partnerPresence}`} aria-label={partnerPresence} />
      </button>
      {open && <div className="v3-chat-switcher__menu" role="menu" aria-label="Direct messages">
        <p>Direct messages</p>
        <div className="v3-chat-switcher__list">
          {items.map(({ conversation, partnerId: itemPartnerId }) => {
            const profile = profiles[itemPartnerId];
            const avatar = avatarFor(profile);
            const decoration = decorationUrl(profile?.avatar_decoration, false);
            return <button key={conversation.id} type="button" role="menuitem" className={conversation.id === activeId ? "is-active" : ""} onClick={() => { useChat.getState().openConversation(conversation.id); setOpen(false); uiSounds.click(); }}>
              <span className="v3-chat-switcher__avatar">
                {avatar ? <img src={avatar} alt="" /> : <span>{nameFor(profile).slice(0, 1).toUpperCase()}</span>}
                {decoration && <img className="v3-chat-switcher__decoration" src={decoration} alt="" />}
              </span>
              <span>{nameFor(profile)}</span>
            </button>;
          })}
          {!items.length && <span className="v3-chat-switcher__empty">No conversations yet</span>}
        </div>
        <div className="v3-chat-switcher__divider" role="separator" />
        <button type="button" role="menuitem" className="v3-chat-switcher__dashboard-item" onClick={() => { useChat.getState().setView("friends"); setOpen(false); uiSounds.click(); }}>
          <LayoutDashboard aria-hidden="true" />
          <span>Dashboard</span>
        </button>
      </div>}
    </div>
  );
}
