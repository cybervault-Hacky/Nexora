/**
 * In-memory IPC telemetry recorder for the Founder Control Centre.
 *
 * Strictly adheres to Nexora privacy & security rules:
 * - Records command names, durations, and status outcomes only.
 * - NEVER records passwords, tokens, keys, code contents, or prompt text.
 * - Entirely in-memory: never written to disk or network.
 */

export interface IpcTelemetryEvent {
  id: string;
  command: string;
  durationMs: number;
  status: "ok" | "error";
  errorSummary?: string;
  timestamp: number;
}

const MAX_EVENTS = 100;
const events: IpcTelemetryEvent[] = [];
const subscribers = new Set<() => void>();

function notify() {
  for (const sub of subscribers) {
    try {
      sub();
    } catch {
      // Ignore subscriber errors
    }
  }
}

let counter = 0;

export function recordIpcCall(
  command: string,
  durationMs: number,
  status: "ok" | "error",
  errorSummary?: string,
): IpcTelemetryEvent {
  counter += 1;
  const event: IpcTelemetryEvent = {
    id: `ipc-${Date.now()}-${counter}`,
    command,
    durationMs: Math.round(durationMs),
    status,
    errorSummary: errorSummary ? errorSummary.slice(0, 100) : undefined,
    timestamp: Date.now(),
  };

  events.unshift(event);
  if (events.length > MAX_EVENTS) {
    events.pop();
  }

  notify();
  return event;
}

export const recordIpcTelemetry = recordIpcCall;

export function getIpcTelemetryEvents(): IpcTelemetryEvent[] {
  return [...events];
}

export function clearIpcTelemetry(): void {
  events.length = 0;
  notify();
}

export function subscribeIpcTelemetry(callback: () => void): () => void {
  subscribers.add(callback);
  return () => {
    subscribers.delete(callback);
  };
}
