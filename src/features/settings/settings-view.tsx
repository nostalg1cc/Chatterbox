import { useEffect, useState } from "react";
import {
  ArrowLeftIcon,
  HeadphonesIcon,
  InfoIcon,
  KeyboardIcon,
  MessageSquareIcon,
  Music2Icon,
  PaletteIcon,
  UserRoundIcon,
} from "lucide-react";
import { useChat } from "@/stores/chat";
import { AboutTab } from "./tabs/AboutTab";
import { AppearanceTab } from "./tabs/AppearanceTab";
import { ChatTab } from "./tabs/ChatTab";
import { GeneralTab } from "./tabs/GeneralTab";
import { KeybindsTab } from "./tabs/KeybindsTab";
import { SoundboardTab } from "./tabs/SoundboardTab";
import { VoiceTab } from "./tabs/VoiceTab";

const TABS = [
  { value: "general", label: "My Account", icon: UserRoundIcon, Panel: GeneralTab },
  { value: "appearance", label: "Appearance", icon: PaletteIcon, Panel: AppearanceTab },
  { value: "chat", label: "Chat", icon: MessageSquareIcon, Panel: ChatTab },
  { value: "voice", label: "Voice & Video", icon: HeadphonesIcon, Panel: VoiceTab },
  { value: "keybinds", label: "Keybinds", icon: KeyboardIcon, Panel: KeybindsTab },
  { value: "soundboard", label: "Soundboard", icon: Music2Icon, Panel: SoundboardTab },
  { value: "about", label: "About", icon: InfoIcon, Panel: AboutTab },
];

// Mounted the same way V3Dashboard is - a plain .stage-internal sibling
// gated by chatView, never a portal/dialog. That's what keeps the window
// drag region (and .top-audio-controls above it) reachable while Settings
// is open: everything here stays inside .stage's one isolation:isolate
// stacking context instead of escaping it the way a Radix Dialog would, and
// .v3-settings's own top padding keeps its content clear of that band
// entirely rather than needing a scoped drag-region div to fight over (see
// styles.css's .v3-settings rule for the actual values).
export function SettingsView() {
  const [tab, setTab] = useState("general");

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") useChat.getState().closeSettings();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const active = TABS.find((item) => item.value === tab) ?? TABS[0];
  const ActivePanel = active.Panel;

  return (
    <>
      <button
        type="button"
        className="v3-settings__back"
        onClick={() => useChat.getState().closeSettings()}
      >
        <ArrowLeftIcon aria-hidden="true" />
        Settings
      </button>
      <div className="v3-settings">
        <nav className="v3-settings__sidebar" aria-label="Settings sections">
          <div className="v3-settings__tabs">
            {TABS.map((item) => (
              <button
                key={item.value}
                type="button"
                className={"v3-settings__tab" + (tab === item.value ? " is-active" : "")}
                aria-current={tab === item.value}
                onClick={() => setTab(item.value)}
              >
                <item.icon aria-hidden="true" />
                {item.label}
              </button>
            ))}
          </div>
        </nav>
        <div className="v3-settings__content">
          <div className="v3-settings__panel-scroll">
            <ActivePanel />
          </div>
        </div>
      </div>
    </>
  );
}
