import { useState } from "react";
import { ActionButton } from "@/features/v3-shell/components/ActionButton";
import { useGeneralAccountForm } from "@/features/settings/hooks/useGeneralAccountForm";
import { useAuth } from "@/stores/auth";
import { usePreferences } from "@/stores/preferences";
import { NameStep } from "./steps/NameStep";
import { AvatarStep } from "./steps/AvatarStep";
import { DecorationStep } from "./steps/DecorationStep";
import { NameStyleStep } from "./steps/NameStyleStep";
import { ThemeStep } from "./steps/ThemeStep";
import { FinishStep } from "./steps/FinishStep";

const STEPS = ["name", "avatar", "decoration", "style", "theme", "finish"] as const;

// Rendered by the shell (not each step component) so the title/description
// stay in the fixed header alongside the progress dots - steps only ever
// render their own interactive content, which is what actually needs to
// scroll (the theme grid in particular is taller than the viewport).
const STEP_META: Record<(typeof STEPS)[number], { title: string; description: string }> = {
  name: { title: "What should we call you?", description: "This is your display name - not your username - and you can change it any time in Settings." },
  avatar: { title: "Add a profile picture", description: "Helps your partner recognize you at a glance. GIFs work too." },
  decoration: { title: "Pick an avatar decoration", description: "Totally optional flair around your avatar. You can always change this later." },
  style: { title: "Style your name", description: "How your display name appears in chat, to everyone." },
  theme: { title: "Pick a look", description: "Sets the color palette for backgrounds, buttons, and other surfaces. Change it any time in Settings." },
  finish: { title: "You're all set", description: "Here's how you'll show up. You can fine-tune any of this later in Settings." },
};

// Full-screen, blocking, one-time flow - not part of .stage, since it
// replaces V3Shell outright while active (see App.tsx) rather than sitting
// inside it like Settings/Dashboard do. That means the usual window drag
// region isn't mounted either, so this renders its own copy of the same
// .window-drag-region strip instead of trying to reuse .stage's.
export function OnboardingFlow() {
  const [stepIndex, setStepIndex] = useState(0);
  const [finishing, setFinishing] = useState(false);
  const theme = usePreferences((state) => state.theme);
  const setPreference = usePreferences((state) => state.setPreference);
  const form = useGeneralAccountForm();

  const stepKey = STEPS[stepIndex];
  const meta = STEP_META[stepKey];
  const isFirst = stepIndex === 0;
  const isLast = stepKey === "finish";
  const hasExistingAvatar = Boolean(form.profile?.avatar_path);

  const goBack = () => setStepIndex((index) => Math.max(index - 1, 0));
  const goNext = () => setStepIndex((index) => Math.min(index + 1, STEPS.length - 1));

  const finish = async () => {
    setFinishing(true);
    try {
      await form.save();
      await useAuth.getState().completeOnboarding();
    } finally {
      setFinishing(false);
    }
  };

  // The avatar step is the one place skipping is conditional - everywhere
  // else a default (no decoration, sans/medium name, the default theme) is
  // already a valid end state, but a never-set avatar falling back to
  // initials is worth actually prompting for once.
  const continueDisabled =
    finishing || (stepKey === "name" && !form.displayName.trim()) || (stepKey === "avatar" && !hasExistingAvatar);
  let primaryLabel = "Continue";
  if (stepKey === "avatar") primaryLabel = hasExistingAvatar ? "Skip" : "Continue";
  if (isLast) primaryLabel = finishing ? "Finishing…" : "Finish";

  return (
    <div className="v3-onboarding">
      <div className="window-drag-region" data-tauri-drag-region aria-hidden="true" />
      <div className="v3-onboarding__card">
        <div className="v3-onboarding__header">
          <div className="v3-onboarding__progress" aria-hidden="true">
            {STEPS.map((key, index) => (
              <span key={key} className={"v3-onboarding__progress-dot" + (index < stepIndex ? " is-done" : index === stepIndex ? " is-active" : "")} />
            ))}
          </div>
          <div className="v3-onboarding__heading">
            <h1>{meta.title}</h1>
            <p>{meta.description}</p>
          </div>
        </div>

        <div className="v3-onboarding__body">
          {stepKey === "name" && <NameStep displayName={form.displayName} onChange={form.setDisplayName} />}
          {stepKey === "avatar" && <AvatarStep previewProfile={form.previewProfile} avatarBusy={form.avatarBusy} onUpload={form.uploadAvatar} />}
          {stepKey === "decoration" && <DecorationStep decoration={form.decoration} onChange={form.setDecoration} />}
          {stepKey === "style" && (
            <NameStyleStep
              displayName={form.displayName}
              nameFont={form.nameFont}
              nameWeight={form.nameWeight}
              nameColor={form.nameColor}
              nameDecoration={form.nameDecoration}
              onFontChange={form.setNameFont}
              onWeightChange={form.setNameWeight}
              onColorChange={form.setNameColor}
              onDecorationChange={form.setNameDecoration}
            />
          )}
          {stepKey === "theme" && <ThemeStep theme={theme} onChange={(value) => setPreference("theme", value)} />}
          {stepKey === "finish" && <FinishStep previewProfile={form.previewProfile} />}
        </div>

        <div className="v3-onboarding__footer">
          {!isFirst ? (
            <ActionButton text="Back" onClick={goBack} disabled={finishing} />
          ) : <span />}
          <button
            type="button"
            className="v3-settings__save"
            disabled={continueDisabled}
            onClick={() => void (isLast ? finish() : goNext())}
          >
            {primaryLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
