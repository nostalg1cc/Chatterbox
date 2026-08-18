import { useState } from "react";
import { AvatarDecorationPicker } from "@/features/settings/components/AvatarDecorationPicker";
import { AvatarDecorationStrip } from "@/features/settings/components/AvatarDecorationStrip";

export function DecorationStep({
  decoration,
  onChange,
}: {
  decoration: string | null;
  onChange: (id: string | null) => void;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);

  return (
    <div className="v3-onboarding__step-heading">
      <h1>Pick an avatar decoration</h1>
      <p>Totally optional flair around your avatar. You can always change this later.</p>
      <div style={{ marginTop: 20 }}>
        <AvatarDecorationStrip selected={decoration} onSelect={onChange} onBrowseAll={() => setPickerOpen(true)} />
      </div>
      <AvatarDecorationPicker open={pickerOpen} onOpenChange={setPickerOpen} selected={decoration} onSelect={onChange} />
    </div>
  );
}
