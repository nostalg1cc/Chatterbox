import { useState } from "react";
import { Avatar, AvatarBadge, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { decorationUrl } from "@/lib/avatar-decorations";
import { initials } from "@/lib/format";
import { supabase } from "@/lib/supabase";
import type { Profile } from "@/lib/types";

export function UserAvatar({ profile, size = "default", online, animated = false, playOnHover = true, decorationActive = false }: { profile: Profile | undefined | null; size?: "sm" | "default" | "lg"; online?: boolean; animated?: boolean; playOnHover?: boolean; decorationActive?: boolean }) {
  const [hovered, setHovered] = useState(false);
  const active = animated || decorationActive || (playOnHover && hovered);
  const avatarPath = active && profile?.avatar_animated_path ? profile.avatar_animated_path : profile?.avatar_path;
  const publicUrl = avatarPath ? supabase.storage.from("avatars").getPublicUrl(avatarPath).data.publicUrl : null;
  const avatarUrl = publicUrl && profile?.avatar_updated_at ? publicUrl + "?v=" + encodeURIComponent(profile.avatar_updated_at) : publicUrl;
  const decoration = decorationUrl(profile?.avatar_decoration, active);
  const wrapperSize = size === "lg" ? "h-10 w-10" : size === "sm" ? "h-6 w-6" : "h-8 w-8";

  return <div className={`relative shrink-0 ${wrapperSize}`} onPointerEnter={playOnHover ? () => setHovered(true) : undefined} onPointerLeave={playOnHover ? () => setHovered(false) : undefined}><Avatar size={size}>{avatarUrl && <AvatarImage src={avatarUrl} alt="" />}<AvatarFallback>{initials(profile?.display_name ?? "?")}</AvatarFallback>{online !== undefined && <AvatarBadge className={online ? "!z-30 bg-emerald-500" : "!z-30 bg-muted-foreground"} />}</Avatar>{decoration && <img src={decoration} aria-hidden className="pointer-events-none absolute z-20 max-w-none object-contain" style={{ left: "50%", top: "50%", width: "118%", aspectRatio: "1 / 1", height: "auto", transform: "translate(-50%, -50%)" }} />}</div>;
}