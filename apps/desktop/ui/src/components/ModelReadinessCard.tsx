import type { CheckStatus, Readiness } from "../lib/ipc";
import { estimateLines, readinessLabel, readinessTone } from "../features/models/readiness";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";

const CHECK_ICON: Record<CheckStatus, IconName> = {
  ok: "check",
  warning: "alert",
  blocked: "alert",
  unknown: "circle",
};

const CHECK_LABEL: Record<CheckStatus, string> = {
  ok: "OK",
  warning: "Warning",
  blocked: "Blocked",
  unknown: "Unknown",
};

interface ModelReadinessCardProps {
  /** The backend's answer, or null while it is being read. */
  readiness: Readiness | null;
  /** Why readiness could not be read, when it could not. */
  error?: string | null;
  /** Refreshes the answer; the card never recomputes it locally. */
  onRefresh?: () => void;
  refreshing?: boolean;
}

/**
 * Renders one authoritative readiness result.
 *
 * Every sentence here comes from the core, except the static labels — the card
 * does not decide whether a model is ready, installed, or likely to fit. The
 * numbers are the core's estimates from the published artefact size and the
 * configured context, and are labelled as estimates everywhere.
 */
export function ModelReadinessCard({
  readiness,
  error = null,
  onRefresh,
  refreshing = false,
}: ModelReadinessCardProps) {
  if (error) {
    return (
      <section className="readiness" aria-label="Model readiness" data-testid="model-readiness">
        <p className="readiness__error" role="alert">
          <Icon name="alert" size={14} /> Readiness could not be read: {error}
        </p>
      </section>
    );
  }

  if (!readiness) {
    return (
      <section className="readiness" aria-label="Model readiness" data-testid="model-readiness">
        <p className="readiness__pending" role="status">
          Reading the model's readiness from the desktop core…
        </p>
      </section>
    );
  }

  const tone = readinessTone(readiness.state);
  const estimate = estimateLines(readiness.estimate);

  return (
    <section
      className="readiness"
      aria-label="Model readiness"
      data-testid="model-readiness"
      data-state={readiness.state}
    >
      <header className="readiness__head">
        <span className={`readiness__badge readiness__badge--${tone}`} data-state={readiness.state}>
          {readinessLabel(readiness.state)}
        </span>
        <h3 className="readiness__headline">{readiness.headline}</h3>
        {onRefresh ? (
          <button
            type="button"
            className="button button--ghost button--sm readiness__refresh"
            onClick={onRefresh}
            disabled={refreshing}
          >
            <Icon name="refresh" size={12} /> {refreshing ? "Checking…" : "Check again"}
          </button>
        ) : null}
      </header>

      {/* Announced when it changes; the check list below is not, so a refresh
          does not read the whole panel aloud. */}
      <p className="readiness__detail" role="status">
        {readiness.detail}
      </p>

      <dl className="readiness__facts">
        <div className="readiness__fact">
          <dt>Model file</dt>
          <dd>{estimate.fileGib} GiB</dd>
        </div>
        <div className="readiness__fact">
          <dt>Free space needed</dt>
          <dd>
            {estimate.requiredFreeGib} GiB
            <span className="readiness__hint"> file plus reserve</span>
          </dd>
        </div>
        <div className="readiness__fact">
          <dt>Memory to load</dt>
          <dd>
            ≈{estimate.memoryGib} GiB
            <span className="readiness__hint">
              {" "}
              at {estimate.context} tokens, {estimate.cacheGib} GiB of it cache
            </span>
          </dd>
        </div>
      </dl>

      <ul className="readiness__checks">
        {readiness.checks.map((check, index) => (
          <li
            // `id` is stable but not unique across a size conflict and a digest
            // failure, so the position disambiguates without reordering.
            key={`${index}-${check.id}`}
            className={`readiness__check readiness__check--${check.status}`}
            data-check={check.id}
            data-status={check.status}
          >
            <Icon name={CHECK_ICON[check.status]} size={13} />
            <span className="readiness__check-label">{CHECK_LABEL[check.status]}</span>
            <span className="readiness__check-text">{check.message}</span>
          </li>
        ))}
      </ul>

      <p className="readiness__disclaimer">
        Storage and memory figures are estimates from the published file size and the
        context window you have configured. They are not a measurement of this
        machine's memory, and they are never a guarantee that a load will succeed.
      </p>
    </section>
  );
}
