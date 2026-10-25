import type { UsageView } from "../features/usage/useUsage";
import { formatCountdown, formatResetClock, usedPercent } from "../lib/usage";
import { Icon } from "./Icon";

interface UsageMeterProps {
  usage: UsageView;
}

/** Presents device-local accounting without inventing a quota or entitlement. */
export function UsageMeter({ usage }: UsageMeterProps) {
  const { status, error, loading, now } = usage;

  if (loading && !status && !error) {
    return (
      <section className="usage" aria-label="Local generation usage">
        <p className="usage__hint" role="status">
          Checking local usage…
        </p>
      </section>
    );
  }

  if (!status) {
    return (
      <section className="usage usage--error" aria-label="Local generation usage">
        <p className="usage__hint" role="alert">
          <Icon name="alert" size={12} />
          Local usage is unavailable: {error ?? "the desktop core did not answer."}
        </p>
      </section>
    );
  }

  if (!status.policyConfigured) {
    return (
      <section className={`usage ${status.used === null ? "usage--error" : ""}`} aria-label="Local generation usage">
        <div className="usage__head">
          <span className="usage__label">
            <Icon name="activity" size={12} />
            Local generation usage
          </span>
          <span className="usage__value">
            {status.used === null ? "Count unknown" : `${status.used} recorded today`}
          </span>
        </div>

        <p className="usage__hint">
          {status.used === null
            ? "The local count could not be verified. Local inference remains available while no local policy is configured."
            : "This device-local count is informational; no local daily limit is configured."}
        </p>

        {status.error ? (
          <p className="usage__hint usage__hint--error" role="alert">
            <Icon name="alert" size={12} />
            The usage record could not be updated: {status.error}
          </p>
        ) : null}

        {status.notice ? (
          <p className="usage__hint" role="status">
            {status.notice}
          </p>
        ) : null}
      </section>
    );
  }

  if (
    status.allowance === null ||
    status.used === null ||
    status.remaining === null ||
    status.resetAtMs === null
  ) {
    return (
      <section className="usage usage--error" aria-label="Local generation usage">
        <p className="usage__hint" role="alert">
          <Icon name="alert" size={12} />
          Usage could not be verified against the configured local limit. New generations are unavailable until the usage record can be checked.
        </p>
        {status.error ? <p className="usage__hint usage__hint--error">{status.error}</p> : null}
        {status.notice ? <p className="usage__hint">{status.notice}</p> : null}
      </section>
    );
  }

  const percent = usedPercent(status);
  const countdown = formatCountdown(status.resetAtMs, now);
  const clock = formatResetClock(status.resetAtMs);
  const resets = clock ? `${countdown} · ${clock} your time` : countdown;

  return (
    <section
      className={`usage ${status.exhausted ? "usage--exhausted" : ""}`}
      aria-label="Local generation usage"
    >
      <div className="usage__head">
        <span className="usage__label">
          <Icon name="activity" size={12} />
          Configured local daily limit
        </span>
        <span className="usage__value">
          {status.exhausted
            ? `${status.used} of ${status.allowance} used`
            : `${status.remaining} of ${status.allowance} left`}
        </span>
      </div>

      <div
        className="usage__bar"
        role="progressbar"
        aria-label="Local generations used"
        aria-valuemin={0}
        aria-valuemax={status.allowance}
        aria-valuenow={Math.min(status.used, status.allowance)}
        aria-valuetext={`${status.used} of ${status.allowance} generations used`}
      >
        <span className="usage__fill" style={{ width: `${percent}%` }} />
      </div>

      <p className="usage__hint">
        {status.exhausted
          ? `Configured local allowance reached. It resets in ${resets}.`
          : `Resets in ${resets}.`}
      </p>

      <p className="usage__policy">Resets at 00:00 UTC each day.</p>

      {status.error ? (
        <p className="usage__hint usage__hint--error" role="alert">
          <Icon name="alert" size={12} />
          The usage record could not be updated: {status.error}
        </p>
      ) : null}

      {status.notice ? (
        <p className="usage__hint" role="status">
          {status.notice}
        </p>
      ) : null}
    </section>
  );
}
