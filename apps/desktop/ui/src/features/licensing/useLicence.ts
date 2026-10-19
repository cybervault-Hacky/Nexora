import { useCallback, useEffect, useState } from "react";
import {
  licenceActivate,
  licenceDeactivate,
  licenceGetStatus,
  type LicenceStatus,
} from "../../lib/licensing";

export function useLicence() {
  const [licence, setLicence] = useState<LicenceStatus>({
    tier: "community",
    planName: "Community Edition",
    active: true,
    licenceKey: null,
    licensee: null,
    expiresAt: null,
    features: [],
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const status = await licenceGetStatus();
      setLicence(status);
    } catch (err: unknown) {
      const msg = typeof err === "object" && err !== null && "message" in err
        ? String((err as { message: unknown }).message)
        : "Failed to read licence status.";
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const activateKey = useCallback(
    async (licenceKey: string) => {
      setError(null);
      const res = await licenceActivate({ licenceKey });
      setLicence(res);
      return res;
    },
    [],
  );

  const deactivateKey = useCallback(async () => {
    setError(null);
    const res = await licenceDeactivate();
    setLicence(res);
    return res;
  }, []);

  return {
    licence,
    loading,
    error,
    refresh,
    activateKey,
    deactivateKey,
  };
}
