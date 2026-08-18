import { useRef } from "react";
import { CameraIcon, Loader2Icon } from "lucide-react";
import { UserAvatar } from "@/components/user-avatar";
import type { Profile } from "@/lib/types";

export function AvatarStep({
  previewProfile,
  avatarBusy,
  onUpload,
}: {
  previewProfile: Profile | null;
  avatarBusy: boolean;
  onUpload: (file: File | undefined) => void;
}) {
  const input = useRef<HTMLInputElement>(null);

  return (
    <div className="v3-onboarding__avatar-picker">
      <button type="button" className="v3-settings__profile-avatar v3-onboarding__avatar-button" aria-label="Upload profile picture" disabled={avatarBusy} onClick={() => input.current?.click()}>
        <UserAvatar profile={previewProfile} size="lg" animated playOnHover={false} />
        <span className={"v3-settings__avatar-overlay" + (avatarBusy ? " is-busy" : "")}>
          {avatarBusy ? <Loader2Icon aria-hidden="true" className="v3-settings__spin" /> : <CameraIcon aria-hidden="true" />}
        </span>
      </button>
      <input
        ref={input}
        type="file"
        accept="image/*"
        className="v3-settings__hidden-input"
        onChange={(event) => onUpload(event.target.files?.[0])}
      />
      <button type="button" className="v3-settings__ghost-button" disabled={avatarBusy} onClick={() => input.current?.click()}>
        {avatarBusy ? "Uploading…" : previewProfile?.avatar_path ? "Change picture" : "Upload picture"}
      </button>
    </div>
  );
}
