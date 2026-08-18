declare module "@/features/v3-shell/components/MessageTemplate" {
  import type { NameColor, NameFont, NameWeight } from "@/lib/types";

  export function MessageTemplate(props: {
    name: string;
    avatar?: string | null;
    avatarDecoration?: string | null;
    nameDecoration?: string | null;
    nameColor?: NameColor;
    nameFont?: NameFont;
    nameWeight?: NameWeight;
    message?: string | null;
    timestamp: string;
    showMeta?: boolean;
    media?: unknown;
    sourceMessage?: unknown;
    isDeleted?: boolean;
    deletedCount?: number;
    isEdited?: boolean;
    decorationActive?: boolean;
    onDecorationHoverChange?: (hovered: boolean) => void;
    replyPreview?: unknown;
  }): import("react").JSX.Element;
}
