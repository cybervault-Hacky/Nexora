import type { UsageView } from "../features/usage/useUsage";
import { formatCountdown, formatResetClock, usedPercent } from "../lib/usage";
import { Icon } from "./Icon";

interface UsageMeterProps {
  usage: UsageView;
}

/**
 * Daily free allowance, next to the composer.
 *
 * Renders only what the Rust core reported. There is no upsell, no "restore"
 * action and no suggestion that buying anything changes the allowance: Phase 7
 * enforces one free limit and nothing else.
 */
export function UsageMeter({ usage }: UsageMeterProps) {
  const { status, error, loading, now } = usage;

  if (loading && !status && !error) {
    return (
      <section className="usage" aria-label="Daily free usage">
        <p className="usage__hint" role="status">
          Checking daily usage…
        </p>
      </section>
    );
  }

  if (!status) {
    return (
      <section className="usage usage--error" aria-label="Daily free usage">
        <p className="usage__hint" role="alert">
          <Icon name="alert" size={12} />
          Daily usage is unavailable: {error ?? "the desktop core did not answer."}
        </p>
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
      aria-label="Daily free usage"
    >
      <div className="usage__head">
        <span className="usage__label">
          <Icon name="activity" size={12} />
          Daily free usage
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
        aria-label="Daily generations used"
        aria-valuemin={0}
        aria-valuemax={status.allowance}
        aria-valuenow={Math.min(status.used, status.allowance)}
        aria-valuetext={`${status.used} of ${status.allowance} generations used`}
      >
        <span className="usage__fill" style={{ width: `${percent}%` }} />
      </div>

      <p className="usage__hint">
        {status.exhausted
          ? `Daily free allowance reached. It resets in ${resets}.`
          : `Resets in ${resets}.`}
        {status.provisional ? " The allowance is provisional." : ""}
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
