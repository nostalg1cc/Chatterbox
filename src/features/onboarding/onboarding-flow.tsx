import { useState } from "react";
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
        <div className="v3-onboarding__progress" aria-hidden="true">
          {STEPS.map((key, index) => (
            <span key={key} className={"v3-onboarding__progress-dot" + (index < stepIndex ? " is-done" : index === stepIndex ? " is-active" : "")} />
          ))}
        </div>

        <div className="v3-onboarding__step">
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
            <button type="button" className="v3-settings__ghost-button" disabled={finishing} onClick={goBack}>Back</button>
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
