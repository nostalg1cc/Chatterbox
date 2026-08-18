import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { prepareAnimatedAvatar, prepareAvatar } from "@/lib/media";
import { supabase } from "@/lib/supabase";
import type { NameColor, NameFont, NameWeight, Profile } from "@/lib/types";
import { useAlerts } from "@/stores/alerts";
import { useAuth } from "@/stores/auth";
import { useProfiles } from "@/stores/profiles";

// Local draft state for the General/My Account tab, plus the avatar upload
// pipeline. Kept as one hook (rather than folded into GeneralTab directly)
// since it's a decent chunk of state/logic on its own.
export function useGeneralAccountForm() {
  const profile = useAuth((state) => state.profile);
  const userId = useAuth((state) => state.userId);
  const [displayName, setDisplayName] = useState(profile?.display_name ?? "");
  const [nameColor, setNameColor] = useState<NameColor>(profile?.name_color ?? "default");
  const [decoration, setDecoration] = useState<string | null>(profile?.avatar_decoration ?? null);
  const [nameDecoration, setNameDecoration] = useState<string | null>(profile?.name_decoration ?? null);
  const [nameFont, setNameFont] = useState<NameFont>(profile?.name_font ?? "sans");
  const [nameWeight, setNameWeight] = useState<NameWeight>(profile?.name_weight ?? "medium");
  const [saving, setSaving] = useState(false);
  const [avatarBusy, setAvatarBusy] = useState(false);
  const avatarInput = useRef<HTMLInputElement>(null);

  const profileChanged =
    displayName.trim() !== (profile?.display_name ?? "") ||
    nameColor !== (profile?.name_color ?? "default") ||
    decoration !== (profile?.avatar_decoration ?? null) ||
    nameDecoration !== (profile?.name_decoration ?? null) ||
    nameFont !== (profile?.name_font ?? "sans") ||
    nameWeight !== (profile?.name_weight ?? "medium");

  const previewProfile = useMemo<Profile | null>(() => {
    if (!profile) return null;
    return { ...profile, display_name: displayName.trim() || profile.display_name, name_color: nameColor, avatar_decoration: decoration, name_decoration: nameDecoration, name_font: nameFont, name_weight: nameWeight };
  }, [decoration, displayName, nameColor, nameDecoration, nameFont, nameWeight, profile]);

  const reset = () => {
    setDisplayName(profile?.display_name ?? "");
    setNameColor(profile?.name_color ?? "default");
    setDecoration(profile?.avatar_decoration ?? null);
    setNameDecoration(profile?.name_decoration ?? null);
    setNameFont(profile?.name_font ?? "sans");
    setNameWeight(profile?.name_weight ?? "medium");
  };

  useEffect(() => {
    reset();
    // Only meant to resync when the underlying profile identity changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profile?.id]);

  const save = async (event?: { preventDefault?: () => void }) => {
    event?.preventDefault?.();
    const name = displayName.trim();
    if (!name) return;
    setSaving(true);
    try {
      await useAuth.getState().updateGeneralSettings(name, nameColor, decoration, nameDecoration, nameFont, nameWeight);
      const updated = useAuth.getState().profile;
      if (updated) useProfiles.getState().put([updated]);
      toast.success("Profile updated.");
    } catch (error) {
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "Couldn't save." });
    } finally {
      setSaving(false);
    }
  };

  const uploadAvatar = async (file: File | undefined) => {
    if (!file || !userId) return;
    setAvatarBusy(true);
    try {
      const isGif = file.type === "image/gif";
      const animatedGif = isGif ? await prepareAnimatedAvatar(file) : null;
      const cover = await prepareAvatar(file);
      const path = userId + "/avatar.webp";
      const animatedPath = userId + "/avatar.gif";
      const { error: coverError } = await supabase.storage.from("avatars").upload(path, cover, {
        upsert: true,
        contentType: "image/webp",
        cacheControl: "3600",
      });
      if (coverError) throw new Error(coverError.message);
      if (isGif) {
        const { error: gifError } = await supabase.storage.from("avatars").upload(animatedPath, animatedGif!, {
          upsert: true,
          contentType: "image/gif",
          cacheControl: "3600",
        });
        if (gifError) throw new Error(gifError.message);
      } else {
        await supabase.storage.from("avatars").remove([animatedPath]);
      }
      await useAuth.getState().updateAvatar(path, isGif ? animatedPath : null);
      const updated = useAuth.getState().profile;
      if (updated) useProfiles.getState().put([updated]);
      toast.success(isGif ? "Animated avatar updated." : "Avatar updated.");
    } catch (error) {
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "Couldn't update your avatar." });
    } finally {
      setAvatarBusy(false);
      if (avatarInput.current) avatarInput.current.value = "";
    }
  };

  return {
    profile,
    displayName, setDisplayName,
    nameColor, setNameColor,
    decoration, setDecoration,
    nameDecoration, setNameDecoration,
    nameFont, setNameFont,
    nameWeight, setNameWeight,
    saving, avatarBusy,
    avatarInput,
    profileChanged,
    previewProfile,
    save,
    reset,
    uploadAvatar,
  };
}
