declare module "@/features/v3-shell/components/Toggle" {
  export function Toggle(props: {
    checked: boolean;
    onChange: (checked: boolean) => void;
    label?: string;
  }): import("react").JSX.Element;
}
