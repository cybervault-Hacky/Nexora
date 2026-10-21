import { useCallback, useEffect, useRef, useState } from "react";
import type { UsageStatus } from "../../lib/ipc";
import { fetchUsage, toUsageError } from "../../lib/usage";

export interface UsageView {
  status: UsageStatus | null;
  /** Set only when the record could not be read. */
  error: string | null;
  loading: boolean;
  /** Re-reads the authoritative record. */
  reload: () => void;
  /** Clock used for the countdown; ticks every 30 s. */
  now: number;
}

const TICK_MS = 30_000;

/**
 * Tracks the daily free allowance.
 *
 * The status comes from the Rust core. The only thing computed here is the
 * countdown to the reset instant the core supplied, recomputed from the clock
 * on every tick — no stored timer, and no claim that a reset happened early.
 */
export function useUsage(refreshKey = 0): UsageView {
  const [status, setStatus] = useState<UsageStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const [now, setNow] = useState(() => Date.now());

  const crossed = useRef(false);

  useEffect(() => {
    let active = true;

    setLoading(true);

    fetchUsage()
      .then((next) => {
        if (!active) return;

        setStatus(next);
        setError(null);
        setNow(Date.now());
        crossed.current = false;
      })
      .catch((cause: unknown) => {
        if (!active) return;

        setStatus(null);
        setError(toUsageError(cause).message);
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [attempt, refreshKey]);

  // Clock-driven ticker: refreshes the countdown and re-reads the record once
  // the reset instant has actually passed.
  useEffect(() => {
    if (!status) {
      return;
    }

    const id = window.setInterval(() => {
      const current = Date.now();
      setNow(current);

      if (!crossed.current && current >= status.resetAtMs) {
        crossed.current = true;
        setAttempt((value) => value + 1);
      }
    }, TICK_MS);

    return () => window.clearInterval(id);
  }, [status]);

  const reload = useCallback(() => {
    setAttempt((value) => value + 1);
  }, []);

  return { status, error, loading, reload, now };
}
