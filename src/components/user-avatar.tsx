import { Avatar, AvatarBadge, AvatarFallback } from "@/components/ui/avatar";
import { initials } from "@/lib/format";
import { cn } from "@/lib/utils";
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
  return (
    <Avatar size={size}>
      <AvatarFallback>{initials(profile?.display_name ?? "?")}</AvatarFallback>
      {online !== undefined && (
        <AvatarBadge
          className={cn(online ? "bg-emerald-500" : "bg-muted-foreground/50")}
        />
      )}
    </Avatar>
  );
}
