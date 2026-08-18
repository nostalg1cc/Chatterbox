export function NameStep({
  displayName,
  onChange,
}: {
  displayName: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="v3-settings__field">
      <input
        autoFocus
        className="v3-settings__input v3-onboarding__name-input"
        value={displayName}
        maxLength={50}
        placeholder="Display name"
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}
