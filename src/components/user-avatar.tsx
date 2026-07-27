import { useState, type CSSProperties } from "react";
import { Avatar, AvatarBadge, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { decorationUrl } from "@/lib/avatar-decorations";
import { supabase } from "@/lib/supabase";
import type { Profile } from "@/lib/types";

function fallbackInitial(profile: Profile | undefined | null): string {
  const source = profile?.display_name?.trim() || profile?.username?.trim() || "?";
  return Array.from(source)[0]?.toLocaleUpperCase() ?? "?";
}

function fallbackStyle(profile: Profile | undefined | null): CSSProperties {
  const seed = profile?.id ?? profile?.username ?? profile?.display_name ?? "dislight";
  let hash = 0;
  for (let index = 0; index < seed.length; index += 1) {
    hash = (hash * 31 + seed.charCodeAt(index)) | 0;
  }
  const hue = Math.abs(hash) % 360;
  return {
    backgroundColor: `hsl(${hue} 42% 29%)`,
    color: `hsl(${hue} 88% 86%)`,
  };
}

export function UserAvatar({
  profile,
  size = "default",
  online,
  animated = false,
  playOnHover = true,
  decorationActive = false,
  className,
}: {
  profile: Profile | undefined | null;
  size?: "sm" | "default" | "lg";
  online?: boolean;
  animated?: boolean;
  playOnHover?: boolean;
  decorationActive?: boolean;
  className?: string;
}) {
  const [hovered, setHovered] = useState(false);
  const active = animated || decorationActive || (playOnHover && hovered);
  const avatarPath = active && profile?.avatar_animated_path ? profile.avatar_animated_path : profile?.avatar_path;
  const publicUrl = avatarPath
    ? supabase.storage.from("avatars").getPublicUrl(avatarPath).data.publicUrl
    : null;
  const avatarUrl = publicUrl && profile?.avatar_updated_at
    ? publicUrl + "?v=" + encodeURIComponent(profile.avatar_updated_at)
    : publicUrl;
  const decoration = decorationUrl(profile?.avatar_decoration, active);
  const wrapperSize = size === "lg" ? "h-10 w-10" : size === "sm" ? "h-6 w-6" : "h-8 w-8";

  return (
    <div
      className={`relative shrink-0 ${wrapperSize} ${className ?? ""}`}
      onPointerEnter={playOnHover ? () => setHovered(true) : undefined}
      onPointerLeave={playOnHover ? () => setHovered(false) : undefined}
    >
      <Avatar size={size}>
        {avatarUrl && <AvatarImage src={avatarUrl} alt="" />}
        <AvatarFallback
          className="font-bold tracking-[-0.02em] text-foreground"
          style={fallbackStyle(profile)}
        >
          {fallbackInitial(profile)}
        </AvatarFallback>
        {online !== undefined && (
          <AvatarBadge className={online ? "!z-30 bg-emerald-500" : "!z-30 bg-muted-foreground"} />
        )}
      </Avatar>
      {decoration && (
        <img
          src={decoration}
          aria-hidden
          className="pointer-events-none absolute z-20 max-w-none object-contain"
          style={{
            left: "50%",
            top: "50%",
            width: "118%",
            aspectRatio: "1 / 1",
            height: "auto",
            transform: "translate(-50%, -50%)",
          }}
        />
      )}
    </div>
  );
}
