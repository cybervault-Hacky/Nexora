import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";

export interface AuthUser {
  id: string;
  name: string;
  email: string;
  emailVerified: boolean;
}

export interface AuthSessionInfo {
  user: AuthUser | null;
  authenticated: boolean;
}

export interface AuthStatusResponse {
  status: string;
  message: string;
}

export interface AuthError {
  code: string;
  message: string;
  retryAfterSeconds?: number;
}

export interface LoginPayload {
  email: string;
  password: string;
}

export interface RegisterPayload {
  name: string;
  email: string;
  password: string;
}

export interface VerifyEmailPayload {
  token: string;
}

export interface ResendVerificationPayload {
  email: string;
}

export interface RecoveryPayload {
  email: string;
}

export interface ResetPasswordPayload {
  token: string;
  password: string;
}

export interface ChangePasswordPayload {
  currentPassword: string;
  newPassword: string;
}

export interface DeveloperDiagnostics {
  platform: string;
  arch: string;
  appVersion: string;
  credentialVault: string;
  authServiceUrl: string;
  cspEnforced: boolean;
  offlineCapable: boolean;
}

export async function authGetSession(): Promise<AuthSessionInfo> {
  if (!isDesktopShell()) {
    return { user: null, authenticated: false };
  }
  return await invoke<AuthSessionInfo>("auth_get_session");
}

export async function authLogin(payload: LoginPayload): Promise<AuthUser> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required for authentication.",
    } as AuthError;
  }
  return await invoke<AuthUser>("auth_login", { request: payload });
}

export async function authRegister(payload: RegisterPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required for authentication.",
    } as AuthError;
  }
  return await invoke<AuthStatusResponse>("auth_register", { request: payload });
}

export async function authVerifyEmail(payload: VerifyEmailPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required for authentication.",
    } as AuthError;
  }
  return await invoke<AuthStatusResponse>("auth_verify_email", { request: payload });
}

export async function authResendVerification(payload: ResendVerificationPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required for authentication.",
    } as AuthError;
  }
  return await invoke<AuthStatusResponse>("auth_resend_verification", { request: payload });
}

export async function authRecoverPassword(payload: RecoveryPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required for authentication.",
    } as AuthError;
  }
  return await invoke<AuthStatusResponse>("auth_recover_password", { request: payload });
}

export async function authResetPassword(payload: ResetPasswordPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required for authentication.",
    } as AuthError;
  }
  return await invoke<AuthStatusResponse>("auth_reset_password", { request: payload });
}

export async function authLogout(): Promise<boolean> {
  if (!isDesktopShell()) {
    return true;
  }
  return await invoke<boolean>("auth_logout");
}

export async function authGoogleBegin(): Promise<AuthUser> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required for Google authentication.",
    } as AuthError;
  }
  return await invoke<AuthUser>("auth_google_begin");
}

export async function authGoogleCancel(): Promise<boolean> {
  if (!isDesktopShell()) {
    return true;
  }
  return await invoke<boolean>("auth_google_cancel");
}

export async function authChangePassword(payload: ChangePasswordPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Desktop app required to change password.",
    } as AuthError;
  }
  return await invoke<AuthStatusResponse>("auth_change_password", { request: payload });
}

export async function getDeveloperDiagnostics(): Promise<DeveloperDiagnostics> {
  if (!isDesktopShell()) {
    return {
      platform: "browser",
      arch: "wasm/js",
      appVersion: "0.1.0",
      credentialVault: "Browser Mock (no native keyring)",
      authServiceUrl: "http://127.0.0.1:8787",
      cspEnforced: true,
      offlineCapable: true,
    };
  }
  return await invoke<DeveloperDiagnostics>("get_developer_diagnostics");
}
