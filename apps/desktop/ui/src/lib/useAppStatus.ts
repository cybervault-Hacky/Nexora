import { useCallback, useEffect, useState } from "react";
import { loadAppStatus, type AppStatus } from "./ipc";

export interface AppStatusState {
  status: AppStatus | null;
  /** Non-null only when the IPC call itself failed. */
  error: string | null;
  loading: boolean;
  reload: () => void;
}

function describeFailure(cause: unknown): string {
  if (typeof cause === "string" && cause.trim().length > 0) {
    return cause;
  }

  if (cause instanceof Error && cause.message.trim().length > 0) {
    return cause.message;
  }

  return "The desktop core did not answer.";
}

/**
 * Reads the Rust status once on mount and on demand.
 *
 * A failed request is kept separate from a successful `not_connected` reply:
 * the first means "we do not know", the second means "we know there is no
 * engine". Collapsing the two would let the UI claim more than it knows.
 */
export function useAppStatus(): AppStatusState {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;

    setLoading(true);

    loadAppStatus()
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
        setError(describeFailure(cause));
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
