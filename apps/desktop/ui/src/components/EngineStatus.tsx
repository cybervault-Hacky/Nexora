import type { AppStatus } from "../lib/ipc";
import { Icon } from "./Icon";

interface EngineStatusProps {
  status: AppStatus | null;
}

export function EngineStatus({ status }: EngineStatusProps) {
  const connected = status?.engine.inferenceAvailable === true;
  const stateLabel = status === null
    ? "Checking…"
    : connected
      ? "Connected"
      : "Not connected";

  return (
    <section className="engine-card" aria-label="AI engine status">
      <div className="engine-card__head">
        <span
          className={`status-dot ${connected ? "status-dot--ok" : "status-dot--idle"}`}
          aria-hidden="true"
        />
        <span className="engine-card__title">AI engine</span>
        <span className="engine-card__state">{stateLabel}</span>
      </div>

      <p className="engine-card__detail">
        {status?.engine.detail ??
          "Checking the local runtime status from the desktop core."}
      </p>

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
