import { useCallback, useEffect, useState } from "react";
import {
  authChangePassword,
  authGetSession,
  authGoogleBegin,
  authLogin,
  authLogout,
  authRecoverPassword,
  authRegister,
  authResetPassword,
  authResendVerification,
  authVerifyEmail,
  type AuthError,
  type AuthUser,
  type ChangePasswordPayload,
  type LoginPayload,
  type RegisterPayload,
} from "../../lib/auth";
import { isDesktopShell } from "../../lib/ipc";

export function useAuth() {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshSession = useCallback(async () => {
    if (!isDesktopShell()) {
      setLoading(false);
      return;
    }
    try {
      const session = await authGetSession();
      setUser(session.user);
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
      setUser(authUser);
      return authUser;
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Sign-in failed.");
      throw err;
    }
  }, []);

  const register = useCallback(async (data: RegisterPayload) => {
    setError(null);
    try {
      return await authRegister(data);
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Registration failed.");
      throw err;
    }
  }, []);

  const verifyEmail = useCallback(async (token: string) => {
    setError(null);
    try {
      return await authVerifyEmail({ token });
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Verification failed.");
      throw err;
    }
  }, []);

  const resendVerification = useCallback(async (email: string) => {
    setError(null);
    try {
      return await authResendVerification({ email });
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Resend failed.");
      throw err;
    }
  }, []);

  const recoverPassword = useCallback(async (email: string) => {
    setError(null);
    try {
      return await authRecoverPassword({ email });
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Recovery request failed.");
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
      const authErr = err as AuthError;
      setError(authErr.message || "Password reset failed.");
      throw err;
    }
  }, []);

  const changePassword = useCallback(async (payload: ChangePasswordPayload) => {
    setError(null);
    try {
      return await authChangePassword(payload);
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Failed to change password.");
      throw err;
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await authLogout();
    } finally {
      setUser(null);
    }
  }, []);

  const signInWithGoogle = useCallback(async (): Promise<AuthUser> => {
    setError(null);
    try {
      const authUser = await authGoogleBegin();
      setUser(authUser);
      return authUser;
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Google sign-in failed.");
      throw err;
    }
  }, []);

  return {
    user,
    isAuthenticated: user !== null,
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
    refreshSession,
  };
}
