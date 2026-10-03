import { version } from "../../package.json";

type DiagnosticValue = string | number | boolean | null;
const events: { at: string; event: string; details: Record<string, DiagnosticValue> }[] = [];

/** Local, bounded, metadata-only diagnostics. Never record SDP or credentials. */
export function recordVoiceEvent(event: string, details: Record<string, DiagnosticValue> = {}): void {
  events.push({ at: new Date().toISOString(), event, details });
  if (events.length > 200) events.shift();
}

export function voiceDiagnosticSnapshot(): string {
  return JSON.stringify({ version, events: [...events] }, null, 2);
}
