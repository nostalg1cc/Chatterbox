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
    <>
      <AvatarDecorationStrip selected={decoration} onSelect={onChange} onBrowseAll={() => setPickerOpen(true)} />
      <AvatarDecorationPicker open={pickerOpen} onOpenChange={setPickerOpen} selected={decoration} onSelect={onChange} />
    </>
  );
}
