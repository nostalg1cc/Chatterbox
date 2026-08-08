import { useEffect, useMemo } from "react";
import { AudioLines, Play, Square, Star, Volume2, VolumeX } from "lucide-react";
import { supabase } from "@/lib/supabase";
import { useAuth } from "@/stores/auth";
import { usePreferences } from "@/stores/preferences";
import { useProfiles } from "@/stores/profiles";
import { useSoundboard } from "@/stores/soundboard";
import { DropdownButton } from "./DropdownButton";

function avatarUrl(profile) {
  const path = profile?.avatar_animated_path || profile?.avatar_path;
  if (!path) return null;
  const url = supabase.storage.from("avatars").getPublicUrl(path).data.publicUrl;
  return profile.avatar_updated_at ? url + "?v=" + encodeURIComponent(profile.avatar_updated_at) : url;
}

export function SoundboardDropdown({ conversationId, isMenuOpen, onMenuOpenChange }) {
  const userId = useAuth((state) => state.userId);
  const selfProfile = useAuth((state) => state.profile);
  const profiles = useProfiles((state) => state.byId);
  const sounds = useSoundboard((state) => state.availableSounds);
  const playingSoundId = useSoundboard((state) => state.playingSoundId);
  const playbackProgress = useSoundboard((state) => state.playbackProgress);
  const pinnedSoundIds = usePreferences((state) => state.pinnedSoundIds);
  const soundboardVolume = usePreferences((state) => state.soundboardVolume);
  const setPreference = usePreferences((state) => state.setPreference);

  useEffect(() => {
    if (!isMenuOpen || !conversationId) return;
    void useSoundboard.getState().loadAvailable(conversationId);
  }, [conversationId, isMenuOpen]);

  useEffect(() => {
    if (isMenuOpen) void useProfiles.getState().ensure(sounds.map((sound) => sound.owner_id));
  }, [isMenuOpen, sounds]);

  const ordered = useMemo(() => [...sounds].sort((a, b) =>
    Number(pinnedSoundIds.includes(b.id)) - Number(pinnedSoundIds.includes(a.id)) ||
    a.created_at.localeCompare(b.created_at)
  ), [sounds, pinnedSoundIds]);

  const togglePin = (id) => setPreference(
    "pinnedSoundIds",
    pinnedSoundIds.includes(id) ? pinnedSoundIds.filter((entry) => entry !== id) : [...pinnedSoundIds, id]
  );

  return (
    <DropdownButton icon={AudioLines} label="Soundboard" isMenuOpen={isMenuOpen} onMenuOpenChange={onMenuOpenChange} className="soundboard-control">
      <div className="soundboard-volume">
        {soundboardVolume === 0 ? <VolumeX aria-hidden="true" /> : <Volume2 aria-hidden="true" />}
        <input type="range" min={0} max={100} value={soundboardVolume} aria-label="Your soundboard listening volume" onChange={(event) => setPreference("soundboardVolume", Number(event.target.value))} />
        <span>{soundboardVolume}%</span>
      </div>
      <div className="soundboard-menu" role="menu" aria-label="Shared soundboard">
        {ordered.length ? ordered.map((sound) => {
          const playing = playingSoundId === sound.id;
          const pinned = pinnedSoundIds.includes(sound.id);
          const owner = sound.owner_id === userId ? selfProfile : profiles[sound.owner_id];
          const ownerAvatar = avatarUrl(owner);
          const ownerName = owner?.display_name ?? owner?.username ?? "Uploader";
          return (
            <div key={sound.id} className="soundboard-widget" data-playing={playing}>
              <span className="soundboard-widget__progress" style={{ transform: "scaleX(" + (playing ? Math.max(0, playbackProgress) : 0) + ")" }} aria-hidden="true" />
              <button className="soundboard-widget__play" type="button" aria-label={(playing ? "Stop " : "Play ") + sound.name} onClick={() => void useSoundboard.getState().play(sound.id)}>
                {playing ? <Square fill="currentColor" strokeWidth={0} /> : <Play fill="currentColor" strokeWidth={0} />}
              </button>
              <button className="soundboard-widget__body" type="button" role="menuitem" aria-label={(playing ? "Stop " : "Play ") + sound.name} onClick={() => void useSoundboard.getState().play(sound.id)}>
                <span className="soundboard-widget__name">{sound.name}</span>
                <span className="soundboard-widget__meta">{(sound.duration_ms / 1_000).toFixed(1)}s</span>
              </button>
              <span className="soundboard-widget__owner" title={ownerName} aria-label={"Uploaded by " + ownerName}>{ownerAvatar ? <img src={ownerAvatar} alt="" /> : <span>{ownerName.slice(0, 1).toUpperCase()}</span>}</span>
              <button className={"soundboard-widget__pin" + (pinned ? " is-pinned" : "")} type="button" aria-label={(pinned ? "Unpin " : "Pin ") + sound.name} onClick={() => togglePin(sound.id)}>
                <Star fill={pinned ? "currentColor" : "none"} />
              </button>
            </div>
          );
        }) : <span className="soundboard-empty">No shared sounds yet</span>}
      </div>
    </DropdownButton>
  );
}
