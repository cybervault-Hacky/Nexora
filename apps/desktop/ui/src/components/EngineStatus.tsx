import type { AppStatus, EngineState } from "../lib/ipc";
import { Icon } from "./Icon";

interface EngineStatusProps {
  status: AppStatus | null;
  failed: boolean;
  collapsed: boolean;
  /** Stops the engine and frees memory. Absent in the browser preview. */
  onUnload?: () => void;
  unloading?: boolean;
}

const LABELS: Record<EngineState, string> = {
  not_configured: "No model",
  stopped: "Stopped",
  loading: "Loading",
  ready: "Ready",
  generating: "Generating",
  error: "Failed",
};

const TONES: Record<EngineState, "idle" | "ok" | "busy" | "unknown"> = {
  not_configured: "idle",
  stopped: "idle",
  loading: "busy",
  ready: "ok",
  generating: "busy",
  error: "unknown",
};

function toneClass(tone: "idle" | "ok" | "busy" | "unknown"): string {
  switch (tone) {
    case "ok":
      return "status-dot--ok";
    case "busy":
      return "status-dot--busy";
    case "unknown":
      return "status-dot--unknown";
    default:
      return "status-dot--idle";
  }
}

export function EngineStatus({
  status,
  failed,
  collapsed,
  onUnload,
  unloading = false,
}: EngineStatusProps) {
  const state: EngineState = failed
    ? "error"
    : (status?.engine.state ?? "not_configured");
  const label = failed ? "Unknown" : LABELS[state];
  const tone = failed ? "unknown" : TONES[state];
  const detail = failed
    ? "The desktop core did not answer, so the engine state cannot be confirmed."
    : (status?.engine.detail ?? "Reading engine status…");

  if (collapsed) {
    return (
      <div className="engine-mini" title={`AI engine: ${label}`}>
        <span className={`status-dot ${toneClass(tone)}`} aria-hidden="true" />
        <span className="sr-only">AI engine: {label}</span>
      </div>
    );
  }

  return (
    <section className="engine-card" aria-label="AI engine status">
      <div className="engine-card__head">
        <span className={`status-dot ${toneClass(tone)}`} aria-hidden="true" />
        <span className="engine-card__title">AI engine</span>
        <span className={`engine-card__state engine-card__state--${tone}`}>
          {label}
        </span>
      </div>

      <p className="engine-card__detail">{detail}</p>

      {status?.engine.model ? (
        <div className="engine-card__meta engine-card__meta--muted">
          {status.engine.model} · {Math.round(status.contextSize / 1024)}K context
        </div>
      ) : (
        <div className="engine-card__meta">
          <Icon name="cloud-off" size={13} />
          <span>No cloud inference. Local-first by design.</span>
        </div>
      )}

      {onUnload && (state === "ready" || state === "generating") ? (
        <button
          type="button"
          className="engine-card__action"
          onClick={onUnload}
          disabled={unloading}
        >
          {unloading ? "Stopping…" : "Stop engine"}
        </button>
      ) : null}
    </section>
  );
}
