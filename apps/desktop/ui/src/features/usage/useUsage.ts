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
 * Tracks device-local generation accounting. A reset timer is used only when
 * the core explicitly reports a configured local policy and reset instant.
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
    const resetAtMs = status?.policyConfigured ? status.resetAtMs : null;
    if (resetAtMs === null || resetAtMs === undefined) {
      return;
    }

    const id = window.setInterval(() => {
      const current = Date.now();
      setNow(current);

      if (!crossed.current && current >= resetAtMs) {
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
