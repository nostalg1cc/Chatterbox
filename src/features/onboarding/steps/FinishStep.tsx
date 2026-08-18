import { DecoratedText, type TextDecoration } from "@/components/decorated-text";
import { UserAvatar } from "@/components/user-avatar";
import { nameColorClass } from "@/lib/name-colors";
import type { Profile } from "@/lib/types";

export function FinishStep({ previewProfile }: { previewProfile: Profile | null }) {
  return (
    <div className="v3-onboarding__step-heading">
      <h1>You're all set</h1>
      <p>Here's how you'll show up. You can fine-tune any of this later in Settings.</p>
      <div className="v3-settings__panel v3-settings__profile-card" style={{ marginTop: 20 }}>
        <UserAvatar profile={previewProfile} size="lg" animated playOnHover={false} />
        <div className="v3-settings__row-copy">
          <p className={"v3-settings__profile-name " + nameColorClass(previewProfile?.name_color)}>
            <DecoratedText effect={previewProfile?.name_decoration as TextDecoration | null} font={previewProfile?.name_font} weight={previewProfile?.name_weight}>
              {previewProfile?.display_name || "Your display name"}
            </DecoratedText>
          </p>
          <p className="v3-settings__row-desc">@{previewProfile?.username ?? "username"}</p>
        </div>
      </div>
    </div>
  );
}
