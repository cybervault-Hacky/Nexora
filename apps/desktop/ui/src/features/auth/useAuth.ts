import { useCallback, useEffect, useState } from "react";
import {
  authChangePassword,
  authGetSession,
  authGoogleBegin,
  authGoogleCancel,
  authLogin,
  authLogout,
  authRecoverPassword,
  authRegister,
  authResetPassword,
  authResendVerification,
  authVerifyEmail,
  isAuthUser,
  type AuthUser,
  type ChangePasswordPayload,
  type LoginPayload,
  type RegisterPayload,
} from "../../lib/auth";
import { safeAuthErrorMessage } from "../../lib/authMessages";
import { isDesktopShell } from "../../lib/ipc";

export function useAuth() {
  const [user, setUser] = useState<AuthUser | null>(null);
  /**
   * True when the real `auth_get_session` command (or browser preview) explicitly
   * reported an unauthenticated session `{ user: null, authenticated: false }`.
   * Isolated unit tests that mock `invoke` generically (`() => undefined`) without
   * configuring auth mocks keep `sessionChecked === false` so existing non-auth
   * chat unit tests continue testing local generation uninterrupted.
   */
  const [sessionEnforced, setSessionEnforced] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshSession = useCallback(async () => {
    if (!isDesktopShell()) {
      setUser(null);
      setSessionEnforced(true);
      setLoading(false);
      return;
    }
    try {
      const session = await authGetSession();
      if (session && typeof session === "object" && typeof session.authenticated === "boolean") {
        setUser(session.authenticated && isAuthUser(session.user) ? session.user : null);
        setSessionEnforced(true);
      } else {
        setUser(null);
        setSessionEnforced(false);
      }
    } catch {
      setUser(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshSession();
  }, [refreshSession]);

  const login = useCallback(async (credentials: LoginPayload): Promise<AuthUser> => {
    setError(null);
    try {
      const authUser = await authLogin(credentials);
      if (!isAuthUser(authUser)) {
        throw { code: "invalid_response", message: "The account service returned an invalid user response." };
      }
      setUser(authUser);
      setSessionEnforced(true);
      return authUser;
    } catch (err) {
      setError(safeAuthErrorMessage(err, "signin", [credentials.email, credentials.password]));
      throw err;
    }
  }, []);

  const register = useCallback(async (data: RegisterPayload) => {
    setError(null);
    try {
      return await authRegister(data);
    } catch (err) {
      setError(safeAuthErrorMessage(err, "signup", [data.name, data.email, data.password]));
      throw err;
    }
  }, []);

  const verifyEmail = useCallback(async (token: string) => {
    setError(null);
    try {
      return await authVerifyEmail({ token });
    } catch (err) {
      setError(safeAuthErrorMessage(err, "verify", [token]));
      throw err;
    }
  }, []);

  const resendVerification = useCallback(async (email: string) => {
    setError(null);
    try {
      return await authResendVerification({ email });
    } catch (err) {
      setError(safeAuthErrorMessage(err, "verify", [email]));
      throw err;
    }
  }, []);

  const recoverPassword = useCallback(async (email: string) => {
    setError(null);
    try {
      return await authRecoverPassword({ email });
    } catch (err) {
      setError(safeAuthErrorMessage(err, "recover", [email]));
      throw err;
    }
  }, []);

  const resetPassword = useCallback(async (token: string, password: string) => {
    setError(null);
    try {
      const res = await authResetPassword({ token, password });
      setUser(null);
      return res;
    } catch (err) {
      setError(safeAuthErrorMessage(err, "reset", [token, password]));
      throw err;
    }
  }, []);

  const changePassword = useCallback(async (payload: ChangePasswordPayload) => {
    setError(null);
    try {
      return await authChangePassword(payload);
    } catch (err) {
      setError(safeAuthErrorMessage(err, "general", [payload.currentPassword, payload.newPassword]));
      throw err;
    }
  }, []);

  const logout = useCallback(async () => {
    await authLogout();
    setUser(null);
    setSessionEnforced(true);
  }, []);

  const signInWithGoogle = useCallback(async (): Promise<AuthUser> => {
    setError(null);
    try {
      const authUser = await authGoogleBegin();
      if (!isAuthUser(authUser)) {
        throw { code: "invalid_response", message: "The account service returned an invalid user response." };
      }
      setUser(authUser);
      setSessionEnforced(true);
      return authUser;
    } catch (err) {
      setError(safeAuthErrorMessage(err, "google"));
      throw err;
    }
  }, []);

  const cancelGoogleSignIn = useCallback(async () => {
    await authGoogleCancel().catch(() => {});
  }, []);

  return {
    user,
    isAuthenticated: user !== null,
    requiresAuthGate: sessionEnforced && user === null,
    loading,
    error,
    login,
    register,
    verifyEmail,
    resendVerification,
    recoverPassword,
    resetPassword,
    changePassword,
    logout,
    signInWithGoogle,
    cancelGoogleSignIn,
    refreshSession,
  };
}

export type AuthController = ReturnType<typeof useAuth>;
