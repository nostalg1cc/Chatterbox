import {
  Avatar,
  AvatarBadge,
  AvatarFallback,
  AvatarImage,
} from "@/components/ui/avatar";
import { initials } from "@/lib/format";
import { supabase } from "@/lib/supabase";
import type { Profile } from "@/lib/types";

export function UserAvatar({
  profile,
  size = "default",
  online,
}: {
  profile: Profile | undefined | null;
  size?: "sm" | "default" | "lg";
  /** Renders a status dot when provided (green = online, gray = offline). */
  online?: boolean;
}) {
  const publicUrl = profile?.avatar_path
    ? supabase.storage.from("avatars").getPublicUrl(profile.avatar_path).data.publicUrl
    : null;
  const avatarUrl = publicUrl && profile?.avatar_updated_at
    ? publicUrl + "?v=" + encodeURIComponent(profile.avatar_updated_at)
    : publicUrl;

  return (
    <Avatar size={size}>
      {avatarUrl && <AvatarImage src={avatarUrl} alt="" />}
      <AvatarFallback>{initials(profile?.display_name ?? "?")}</AvatarFallback>
      {online !== undefined && (
        <AvatarBadge
          className={online ? "bg-emerald-500" : "bg-muted-foreground"}
        />
      )}
    </Avatar>
  );
}

