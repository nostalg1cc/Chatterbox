declare module "@/features/v3-shell/components/Slider" {
  export function Slider(props: {
    label: string;
    value: number;
    min?: number;
    max?: number;
    formatValue?: (value: number) => string;
    onChange: (value: number) => void;
  }): import("react").JSX.Element;
}
