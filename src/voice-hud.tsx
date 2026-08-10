import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { VoiceHud } from "@/features/voice-hud/VoiceHud";
import "./voice-hud.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <VoiceHud />
  </StrictMode>
);
