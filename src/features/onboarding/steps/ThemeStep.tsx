import { ThemePicker } from "@/features/settings/components/ThemePicker";
import type { AppTheme } from "@/stores/preferences";

export function ThemeStep({ theme, onChange }: { theme: AppTheme; onChange: (theme: AppTheme) => void }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 22 }}>
      <ThemePicker theme={theme} onChange={onChange} />
    </div>
  );
}
