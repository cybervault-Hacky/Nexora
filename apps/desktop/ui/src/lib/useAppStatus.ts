import { useCallback, useEffect, useState } from "react";
import { fetchStatus, toInferenceError } from "./inference";
import type { AppStatus } from "./ipc";

export interface AppStatusState {
  status: AppStatus | null;
  /** Non-null only when the IPC call itself failed. */
  error: string | null;
  loading: boolean;
  reload: () => void;
}

/**
 * Reads the real engine status from the Rust core.
 *
 * A failed request is kept separate from a successful `stopped` or
 * `not_configured` reply: the first means "unknown", the others mean "known and
 * not ready". The UI must never blur that distinction.
 */
export function useAppStatus(): AppStatusState {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;

    setLoading(true);

    fetchStatus()
      .then((next) => {
        if (!active) {
          return;
        }

        setStatus(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        if (!active) {
          return;
        }

        setStatus(null);
        setError(toInferenceError(cause).message);
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [attempt]);

  const reload = useCallback(() => {
    setAttempt((current) => current + 1);
  }, []);

  return { status, error, loading, reload };
}
