import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  authChangePassword,
  authGetSession,
  billingListPlans,
  authGoogleBegin,
  authGoogleCancel,
  authLogin,
  authLogout,
  authRecoverPassword,
  authRegister,
  authResetPassword,
  authResendVerification,
  authVerifyEmail,
  getDeveloperDiagnostics,
} from "../../lib/auth";
import { markDesktop } from "../../test/fixtures";

describe("Phase 24 — Desktop Authentication IPC Contracts", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    markDesktop(true);
  });

  it("authGetSession invokes auth_get_session", async () => {
    invokeMock.mockResolvedValueOnce({
      user: { id: "u-1", name: "Ada", email: "ada@example.com", emailVerified: true },
      authenticated: true,
    });

    const session = await authGetSession();
    expect(invokeMock).toHaveBeenCalledWith("auth_get_session");
    expect(session.authenticated).toBe(true);
    expect(session.user?.name).toBe("Ada");
  });

  it("authLogin sends typed credentials under request parameter", async () => {
    invokeMock.mockResolvedValueOnce({
      id: "u-1",
      name: "Ada",
      email: "ada@example.com",
      emailVerified: true,
    });

    const user = await authLogin({ email: "ada@example.com", password: "hunter2password" });
    expect(invokeMock).toHaveBeenCalledWith("auth_login", {
      request: { email: "ada@example.com", password: "hunter2password" },
    });
    expect(user.id).toBe("u-1");
  });

  it("authRegister sends name, email, password", async () => {
    invokeMock.mockResolvedValueOnce({
      status: "verification_pending",
      message: "Please verify your email",
    });

    const res = await authRegister({
      name: "Ada Lovelace",
      email: "ada@example.com",
      password: "securepassword123",
    });

    expect(invokeMock).toHaveBeenCalledWith("auth_register", {
      request: {
        name: "Ada Lovelace",
        email: "ada@example.com",
        password: "securepassword123",
      },
    });
    expect(res.status).toBe("verification_pending");
  });

  it("authVerifyEmail sends token", async () => {
    invokeMock.mockResolvedValueOnce({ status: "verified", message: "Verified" });
    const res = await authVerifyEmail({ token: "tok-abc" });
    expect(invokeMock).toHaveBeenCalledWith("auth_verify_email", {
      request: { token: "tok-abc" },
    });
    expect(res.status).toBe("verified");
  });

  it("authResendVerification sends email", async () => {
    invokeMock.mockResolvedValueOnce({ status: "sent", message: "Sent" });
    const res = await authResendVerification({ email: "ada@example.com" });
    expect(invokeMock).toHaveBeenCalledWith("auth_resend_verification", {
      request: { email: "ada@example.com" },
    });
    expect(res.status).toBe("sent");
  });

  it("authRecoverPassword sends email", async () => {
    invokeMock.mockResolvedValueOnce({ status: "sent", message: "Sent" });
    const res = await authRecoverPassword({ email: "ada@example.com" });
    expect(invokeMock).toHaveBeenCalledWith("auth_recover_password", {
      request: { email: "ada@example.com" },
    });
    expect(res.status).toBe("sent");
  });

  it("authResetPassword sends token and new password", async () => {
    invokeMock.mockResolvedValueOnce({ status: "reset", message: "Reset" });
    const res = await authResetPassword({ token: "tok-123", password: "newpassword8" });
    expect(invokeMock).toHaveBeenCalledWith("auth_reset_password", {
      request: { token: "tok-123", password: "newpassword8" },
    });
    expect(res.status).toBe("reset");
  });

  it("authLogout invokes auth_logout", async () => {
    invokeMock.mockResolvedValueOnce(true);
    const ok = await authLogout();
    expect(invokeMock).toHaveBeenCalledWith("auth_logout");
    expect(ok).toBe(true);
  });

  it("authGoogleBegin invokes auth_google_begin", async () => {
    invokeMock.mockResolvedValueOnce({
      id: "g-1",
      name: "Google User",
      email: "google@example.com",
      emailVerified: true,
    });

    const user = await authGoogleBegin();
    expect(invokeMock).toHaveBeenCalledWith("auth_google_begin");
    expect(user.id).toBe("g-1");
  });

  it("authGoogleCancel invokes auth_google_cancel", async () => {
    invokeMock.mockResolvedValueOnce(true);
    const ok = await authGoogleCancel();
    expect(invokeMock).toHaveBeenCalledWith("auth_google_cancel");
    expect(ok).toBe(true);
  });

  it("authChangePassword invokes auth_change_password", async () => {
    invokeMock.mockResolvedValueOnce({ status: "success", message: "Password updated successfully." });
    const res = await authChangePassword({
      currentPassword: "oldpassword8",
      newPassword: "newpassword8",
    });
    expect(invokeMock).toHaveBeenCalledWith("auth_change_password", {
      request: {
        currentPassword: "oldpassword8",
        newPassword: "newpassword8",
      },
    });
    expect(res.status).toBe("success");
  });

  it("getDeveloperDiagnostics invokes get_developer_diagnostics", async () => {
    invokeMock.mockResolvedValueOnce({
      platform: "linux",
      arch: "x86_64",
      appVersion: "0.1.0",
      credentialVault: "Keyring OS Vault",
      authServiceUrl: "http://127.0.0.1:8787",
      cspEnforced: true,
      offlineCapable: true,
    });
    const diag = await getDeveloperDiagnostics();
    expect(invokeMock).toHaveBeenCalledWith("get_developer_diagnostics");
    expect(diag.credentialVault).toBe("Keyring OS Vault");
    expect(diag.cspEnforced).toBe(true);
    expect(diag.offlineCapable).toBe(true);
  });

  it("falls back safely when not in desktop shell", async () => {
    markDesktop(false);

    const session = await authGetSession();
    expect(session.authenticated).toBe(false);
    expect(session.user).toBeNull();
    await expect(billingListPlans()).resolves.toEqual({
      paymentConfigured: false,
      provider: "unconfigured",
      plans: [],
    });
    expect(invokeMock).not.toHaveBeenCalledWith("billing_list_plans");

    await expect(authLogin({ email: "a@b.c", password: "p" })).rejects.toMatchObject({
      code: "desktop_required",
    });
  });
});
