import { emitTo } from "@tauri-apps/api/event";
import { LogicalPosition, LogicalSize, primaryMonitor } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { isTauri } from "@/lib/tauri";
import type { NameColor, NameFont, NameWeight } from "@/lib/types";
import type { TextDecoration } from "@/components/decorated-text";

export interface VoiceHudParticipant {
  id: string;
  name: string;
  avatar: string | null;
  avatarDecoration: string | null;
  nameColor: NameColor | null;
  nameDecoration: TextDecoration | null;
  nameFont: NameFont | null;
  nameWeight: NameWeight | null;
  speaking: boolean;
  level: number;
}

const HUD_LABEL = "voice-hud";
// Must match src-tauri/tauri.conf.json's "voice-hud" window width/height, and
// src/voice-hud.css's ".voice-hud" box - the size at 100% scale, before the
// user's overlay-size preference scales it.
const HUD_BASE_WIDTH = 200;
const HUD_BASE_HEIGHT = 140;
const HUD_MARGIN_X = 24;

let hudWindowPromise: Promise<WebviewWindow | null> | null = null;
let lastScale = 100;

function getHudWindow(): Promise<WebviewWindow | null> {
  if (!isTauri) return Promise.resolve(null);
  hudWindowPromise ??= WebviewWindow.getByLabel(HUD_LABEL);
  return hudWindowPromise;
}

async function sizeAndPositionHud(hud: WebviewWindow, scale: number): Promise<void> {
  try {
    const width = Math.round(HUD_BASE_WIDTH * (scale / 100));
    const height = Math.round(HUD_BASE_HEIGHT * (scale / 100));
    await hud.setSize(new LogicalSize(width, height));

    const monitor = await primaryMonitor();
    if (!monitor) return;
    const monitorSize = monitor.size.toLogical(monitor.scaleFactor);
    const monitorPosition = monitor.position.toLogical(monitor.scaleFactor);
    const x = monitorPosition.x + HUD_MARGIN_X;
    const y = monitorPosition.y + (monitorSize.height - height) / 2;
    await hud.setPosition(new LogicalPosition(x, y));
  } catch {
    // Sizing/positioning is cosmetic - leave the HUD wherever it last was
    // rather than failing the voice call over it.
  }
}

export async function showVoiceHud(scale = 100): Promise<void> {
  const hud = await getHudWindow();
  if (!hud) return;
  try {
    lastScale = scale;
    await sizeAndPositionHud(hud, scale);
    // Click-through: the HUD must never intercept clicks meant for whatever
    // is running fullscreen/borderless behind it.
    await hud.setIgnoreCursorEvents(true);
    await hud.show();
  } catch {
    // Never let HUD window failures affect the actual voice call.
  }
}

export async function hideVoiceHud(): Promise<void> {
  const hud = await getHudWindow();
  if (!hud) return;
  await hud.hide().catch(() => undefined);
}

export async function resizeVoiceHud(scale: number): Promise<void> {
  if (scale === lastScale) return;
  const hud = await getHudWindow();
  if (!hud) return;
  const visible = await hud.isVisible().catch(() => false);
  if (!visible) return;
  lastScale = scale;
  await sizeAndPositionHud(hud, scale);
}

export interface VoiceHudUpdate {
  participants: VoiceHudParticipant[];
  scale: number;
  showNames: boolean;
}

export async function updateVoiceHud(update: VoiceHudUpdate): Promise<void> {
  if (!isTauri) return;
  await emitTo(HUD_LABEL, "voice-hud:update", update).catch(() => undefined);
}
