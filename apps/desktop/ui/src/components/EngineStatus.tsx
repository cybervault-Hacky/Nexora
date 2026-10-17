import { isEngineReady, type AppStatus } from "../lib/ipc";
import { Icon } from "./Icon";

interface EngineStatusProps {
  status: AppStatus | null;
  failed: boolean;
  collapsed: boolean;
}

interface Presentation {
  label: string;
  tone: "idle" | "ok" | "unknown";
  detail: string;
}

function present(status: AppStatus | null, failed: boolean): Presentation {
  if (failed) {
    return {
      label: "Unknown",
      tone: "unknown",
      detail:
        "The desktop core did not answer. The engine state cannot be confirmed.",
    };
  }

  if (isEngineReady(status)) {
    return {
      label: "Connected",
      tone: "ok",
      detail: "A local model is loaded and ready.",
    };
  }

  return {
    label: "Not connected",
    tone: "idle",
    detail:
      status?.engine.detail ??
      "No local model is loaded. Replies are scaffold text, not model output.",
  };
}

export function EngineStatus({ status, failed, collapsed }: EngineStatusProps) {
  const { label, tone, detail } = present(status, failed);
  const toneClass =
    tone === "ok"
      ? "status-dot--ok"
      : tone === "unknown"
        ? "status-dot--unknown"
        : "status-dot--idle";

  if (collapsed) {
    return (
      <div className="engine-mini" title={`AI engine: ${label}`}>
        <span className={`status-dot ${toneClass}`} aria-hidden="true" />
        <span className="sr-only">AI engine: {label}</span>
      </div>
    );
  }

  return (
    <section className="engine-card" aria-label="AI engine status">
      <div className="engine-card__head">
        <span className={`status-dot ${toneClass}`} aria-hidden="true" />
        <span className="engine-card__title">AI engine</span>
        <span className={`engine-card__state engine-card__state--${tone}`}>
          {label}
        </span>
      </div>

      <p className="engine-card__detail">{detail}</p>

      <div className="engine-card__meta">
        <Icon name="cloud-off" size={13} />
        <span>No cloud inference. Local-first by design.</span>
      </div>

      {status ? (
        <div className="engine-card__meta engine-card__meta--muted">
          Runtime: {status.engine.runtime}
        </div>
      ) : null}
    </section>
  );
}
