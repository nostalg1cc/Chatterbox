import { MessageTemplate } from "@/features/v3-shell/components/MessageTemplate";
import { supabase } from "@/lib/supabase";
import type { Profile } from "@/lib/types";

// Same URL-building logic as v3-shell.jsx's local avatarUrl() helper -
// MessageTemplate takes a resolved URL, not a profile object, matching how
// every other caller of it already works.
function avatarUrl(profile: Profile | null) {
  const path = profile?.avatar_animated_path || profile?.avatar_path;
  if (!path) return null;
  const url = supabase.storage.from("avatars").getPublicUrl(path).data.publicUrl;
  return profile?.avatar_updated_at ? `${url}?v=${encodeURIComponent(profile.avatar_updated_at)}` : url;
}

export function FinishStep({ previewProfile }: { previewProfile: Profile | null }) {
  return (
    <div className="v3-onboarding__finish-preview">
      {/* The real message component, not a lookalike - this is exactly how
          the message would render in an actual conversation, avatar
          decoration/name font/weight/color/effect and all. sourceMessage is
          left undefined so the action tray and reactions never mount - it's
          a preview, not something you can actually edit/react to. */}
      <MessageTemplate
        name={previewProfile?.display_name || "You"}
        avatar={avatarUrl(previewProfile)}
        avatarDecoration={previewProfile?.avatar_decoration}
        nameDecoration={previewProfile?.name_decoration}
        nameColor={previewProfile?.name_color}
        nameFont={previewProfile?.name_font}
        nameWeight={previewProfile?.name_weight}
        message="Welcome to Nitro!"
        timestamp="Just now"
        showMeta
      />
    </div>
  );
}
