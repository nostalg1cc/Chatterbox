export function NameStep({
  displayName,
  onChange,
}: {
  displayName: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="v3-onboarding__step-heading">
      <h1>What should we call you?</h1>
      <p>This is your display name - not your username - and you can change it any time in Settings.</p>
      <div className="v3-settings__field" style={{ marginTop: 20 }}>
        <input
          autoFocus
          className="v3-settings__input v3-onboarding__name-input"
          value={displayName}
          maxLength={50}
          placeholder="Display name"
          onChange={(event) => onChange(event.target.value)}
        />
      </div>
    </div>
  );
}
