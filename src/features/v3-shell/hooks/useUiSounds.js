import { playAppSound } from "@/lib/app-sounds";

export const UI_SOUND_PRELOAD = [];

export function useUiSounds() {
  return {
    hover: () => playAppSound("ui_hover"),
    click: () => playAppSound("ui_click"),
    toggle: (isDisabled) => playAppSound(isDisabled ? "mute_off" : "mute_on"),
    menu: (isOpen) => playAppSound(isOpen ? "ui_menu_open" : "ui_menu_close"),
    alert: () => playAppSound("notification_single"),
    message: () => playAppSound("message"),
  };
}
