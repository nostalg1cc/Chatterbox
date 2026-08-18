import type { ReactNode } from "react";

export function DeviceSelect({
  id,
  label,
  icon,
  value,
  devices,
  fallback,
  onChange,
}: {
  id: string;
  label: string;
  icon: ReactNode;
  value: string;
  devices: MediaDeviceInfo[];
  fallback: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="v3-settings__field">
      <label htmlFor={id} className="v3-settings__field-label">
        {icon}
        {label}
      </label>
      <select id={id} value={value} className="v3-settings__select" onChange={(event) => onChange(event.target.value)}>
        <option value="default">{fallback}</option>
        {devices.map((device, index) => (
          <option key={device.deviceId || label + index} value={device.deviceId}>
            {device.label || label + " " + (index + 1)}
          </option>
        ))}
      </select>
    </div>
  );
}
