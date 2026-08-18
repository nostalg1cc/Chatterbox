import { useState } from "react";
import { CameraIcon, LogOutIcon } from "lucide-react";
import { DecoratedText, type TextDecoration } from "@/components/decorated-text";
import { UserAvatar } from "@/components/user-avatar";
import { AvatarDecorationPicker } from "../components/AvatarDecorationPicker";
import { AvatarDecorationStrip } from "../components/AvatarDecorationStrip";
import { useGeneralAccountForm } from "../hooks/useGeneralAccountForm";
import { NAME_COLOR_OPTIONS, nameColorClass } from "@/lib/name-colors";
import type { NameFont, NameWeight } from "@/lib/types";
import { useAuth } from "@/stores/auth";
import { useVoice } from "@/stores/voice";

const NAME_FONTS: NameFont[] = ["sans", "rounded", "serif", "mono"];
const NAME_WEIGHTS: NameWeight[] = ["regular", "medium", "bold", "black"];
const NAME_EFFECTS = ["fuzzy", "sparkles", "resize", "bouncy", "wavy", "gradient", "glitch", "particle"];

export function GeneralTab() {
  const email = useAuth((state) => state.email);
  const [decorationPickerOpen, setDecorationPickerOpen] = useState(false);
  const {
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
  } = useGeneralAccountForm();

  const signOut = () => {
    void (async () => {
      await useVoice.getState().leave();
      await useAuth.getState().signOut();
    })();
  };

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>My Account</h2>
        <p>This is how your profile shows up to your partner and in chat.</p>
      </div>

      <div className="v3-settings__panel v3-settings__profile-card">
        <button
          type="button"
          className="v3-settings__profile-avatar"
          aria-label="Change avatar"
          disabled={avatarBusy}
          onClick={() => avatarInput.current?.click()}
        >
          <UserAvatar profile={previewProfile} size="lg" animated playOnHover={false} />
          <span className={"v3-settings__avatar-overlay" + (avatarBusy ? " is-busy" : "")}>
            <CameraIcon aria-hidden="true" />
          </span>
        </button>
        <input
          ref={avatarInput}
          type="file"
          accept="image/*"
          className="v3-settings__hidden-input"
          onChange={(event) => void uploadAvatar(event.target.files?.[0])}
        />
        <div className="v3-settings__row-copy">
          <p className={"v3-settings__profile-name " + nameColorClass(nameColor)}>
            <DecoratedText effect={nameDecoration as TextDecoration | null} font={nameFont} weight={nameWeight}>
              {displayName.trim() || profile?.display_name || "Your display name"}
            </DecoratedText>
          </p>
          <p className="v3-settings__row-desc">@{profile?.username ?? "username"}</p>
        </div>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <div className="v3-settings__field">
            <label htmlFor="settings-displayname" className="v3-settings__field-label">Display name</label>
            <input
              id="settings-displayname"
              className="v3-settings__input"
              value={displayName}
              maxLength={50}
              onChange={(event) => setDisplayName(event.target.value)}
            />
          </div>
        </div>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label" style={{ marginBottom: 10 }}>Avatar decoration</p>
          <AvatarDecorationStrip selected={decoration} onSelect={setDecoration} onBrowseAll={() => setDecorationPickerOpen(true)} />
        </div>
      </div>

      <AvatarDecorationPicker
        open={decorationPickerOpen}
        onOpenChange={setDecorationPickerOpen}
        selected={decoration}
        onSelect={setDecoration}
      />

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Name font</p>
          <div className="v3-settings__choice-group">
            {NAME_FONTS.map((font) => (
              <button
                key={font}
                type="button"
                className={"v3-settings__choice" + (nameFont === font ? " is-active" : "")}
                onClick={() => setNameFont(font)}
              >
                {font}
              </button>
            ))}
          </div>
        </div>
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Name weight</p>
          <div className="v3-settings__choice-group">
            {NAME_WEIGHTS.map((weight) => (
              <button
                key={weight}
                type="button"
                className={"v3-settings__choice" + (nameWeight === weight ? " is-active" : "")}
                onClick={() => setNameWeight(weight)}
              >
                {weight}
              </button>
            ))}
          </div>
        </div>
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Name effect</p>
          <div className="v3-settings__choice-group">
            {NAME_EFFECTS.map((effect) => (
              <button
                key={effect}
                type="button"
                className={"v3-settings__choice" + (nameDecoration === effect ? " is-active" : "")}
                onClick={() => setNameDecoration(effect)}
              >
                {effect}
              </button>
            ))}
            <button type="button" className={"v3-settings__choice" + (nameDecoration === null ? " is-active" : "")} onClick={() => setNameDecoration(null)}>
              None
            </button>
          </div>
        </div>
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Chat name color</p>
          <div className="v3-settings__swatch-grid">
            {NAME_COLOR_OPTIONS.map((option) => (
              <button
                key={option.value}
                type="button"
                aria-label={option.label}
                aria-pressed={nameColor === option.value}
                className={"v3-settings__swatch" + (nameColor === option.value ? " is-active" : "")}
                onClick={() => setNameColor(option.value)}
              >
                <span className={"v3-settings__swatch-dot " + option.swatch} />
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section v3-settings__field-pair">
          <div className="v3-settings__field">
            <label htmlFor="settings-username" className="v3-settings__field-label">Username</label>
            <input id="settings-username" className="v3-settings__input" value={"@" + (profile?.username ?? "")} disabled />
          </div>
          <div className="v3-settings__field">
            <label htmlFor="settings-email" className="v3-settings__field-label">Email</label>
            <input id="settings-email" className="v3-settings__input" value={email ?? ""} disabled />
          </div>
        </div>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__row">
          <div className="v3-settings__row-copy">
            <p className="v3-settings__row-title">Sign out</p>
            <p className="v3-settings__row-desc">You'll need to sign back in to use Nitro again.</p>
          </div>
          <button type="button" className="v3-settings__ghost-button is-danger" onClick={signOut}>
            <LogOutIcon aria-hidden="true" />
            Sign out
          </button>
        </div>
      </div>

      <div className={"v3-settings__savebar" + (profileChanged ? " is-visible" : "")}>
        <span>Careful — you have unsaved changes.</span>
        <div className="v3-settings__savebar-actions">
          <button type="button" className="v3-settings__ghost-button" onClick={reset} disabled={saving}>Reset</button>
          <button type="button" className="v3-settings__save" disabled={saving || !displayName.trim()} onClick={() => void save()}>
            {saving ? "Saving…" : "Save changes"}
          </button>
        </div>
      </div>
    </div>
  );
}
