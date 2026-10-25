import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const getDeveloperDiagnosticsMock = vi.hoisted(() => vi.fn());
const isDesktopShellMock = vi.hoisted(() => vi.fn(() => true));

vi.mock("../lib/auth", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/auth")>();
  return { ...actual, getDeveloperDiagnostics: getDeveloperDiagnosticsMock };
});

vi.mock("../lib/ipc", () => ({ isDesktopShell: isDesktopShellMock }));

import { AuthModal, type AuthModalMode } from "./AuthModal";
import type { AuthStatusResponse, AuthUser, LoginPayload, RegisterPayload } from "../lib/auth";

const signedInUser: AuthUser = {
  id: "user-1",
  name: "Ada Lovelace",
  email: "ada@example.com",
  emailVerified: true,
};

interface HarnessProps {
  initialMode?: AuthModalMode;
  initialToken?: string;
  onLogin?: (payload: LoginPayload) => Promise<AuthUser>;
  onRegister?: (payload: RegisterPayload) => Promise<AuthStatusResponse>;
  onVerifyEmail?: (token: string) => Promise<AuthStatusResponse>;
  onResendVerification?: (email: string) => Promise<AuthStatusResponse>;
  onRecoverPassword?: (email: string) => Promise<AuthStatusResponse>;
  onResetPassword?: (token: string, password: string) => Promise<AuthStatusResponse>;
  onGoogleSignIn?: () => Promise<AuthUser>;
  onCancelGoogle?: () => Promise<void>;
  onClosed?: () => void;
}

function Harness({
  initialMode = "signin",
  initialToken = "",
  onLogin = async () => signedInUser,
  onRegister = async () => ({ status: "verification_pending", message: "Request accepted." }),
  onVerifyEmail = async () => ({ status: "verified", message: "Email verified." }),
  onResendVerification = async () => ({ status: "accepted", message: "Request accepted." }),
  onRecoverPassword = async () => ({ status: "accepted", message: "Request accepted." }),
  onResetPassword = async () => ({ status: "reset", message: "Password updated." }),
  onGoogleSignIn = async () => signedInUser,
  onCancelGoogle = async () => {},
  onClosed,
}: HarnessProps) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open sign-in modal
      </button>
      <AuthModal
        open={open}
        initialMode={initialMode}
        initialToken={initialToken}
        onClose={() => {
          setOpen(false);
          onClosed?.();
        }}
        onLogin={onLogin}
        onRegister={onRegister}
        onVerifyEmail={onVerifyEmail}
        onResendVerification={onResendVerification}
        onRecoverPassword={onRecoverPassword}
        onResetPassword={onResetPassword}
        onGoogleSignIn={onGoogleSignIn}
        onCancelGoogle={onCancelGoogle}
      />
    </>
  );
}

const unavailableDiagnostics = {
  platform: "windows",
  arch: "x86_64",
  appVersion: "0.1.0",
  credentialVault: "Windows Credential Manager",
  authServiceUrl: "https://accounts.example.test",
  googleOauthConfigured: true,
  cspEnforced: true,
  offlineCapable: true,
};

beforeEach(() => {
  isDesktopShellMock.mockReturnValue(true);
  getDeveloperDiagnosticsMock.mockReset();
  getDeveloperDiagnosticsMock.mockResolvedValue(unavailableDiagnostics);
  document.documentElement.dataset.theme = "dark";
  document.body.style.overflow = "";
});

afterEach(() => {
  document.body.style.overflow = "";
  vi.restoreAllMocks();
});

describe("Phase 09 — floating authentication dialog", () => {
  it("moves focus into the dialog, traps keyboard focus, and restores focus when closed", async () => {
    const user = userEvent.setup();
    const previousOverflow = "scroll";
    document.body.style.overflow = previousOverflow;
    render(<Harness />);

    const trigger = screen.getByRole("button", { name: "Open sign-in modal" });
    await user.click(trigger);
    const modal = await screen.findByRole("dialog", { name: "Sign in to Nexora" });
    const email = within(modal).getByLabelText("Email address");
    expect(document.activeElement).toBe(email);
    expect(document.body.style.overflow).toBe("hidden");

    const lastLink = within(modal).getByRole("button", { name: "Have a verification code?" });
    lastLink.focus();
    await user.tab();
    expect(document.activeElement).toBe(within(modal).getByRole("button", { name: "Close authentication dialog" }));

    await user.click(within(modal).getByRole("button", { name: "Close authentication dialog" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.activeElement).toBe(trigger);
    expect(document.body.style.overflow).toBe(previousOverflow);
  });

  it("keeps authentication tab relationships aligned with the panel currently rendered", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");

    const signInTab = within(modal).getByRole("tab", { name: "Sign in" });
    const signUpTab = within(modal).getByRole("tab", { name: "Create account" });
    expect(signInTab.getAttribute("aria-selected")).toBe("true");
    expect(modal.querySelector(`#${signInTab.getAttribute("aria-controls")}`)).toBeTruthy();
    expect(signUpTab.hasAttribute("aria-controls")).toBe(false);

    await user.click(signUpTab);
    const activeSignUpTab = within(modal).getByRole("tab", { name: "Create account" });
    expect(activeSignUpTab.getAttribute("aria-selected")).toBe("true");
    expect(modal.querySelector(`#${activeSignUpTab.getAttribute("aria-controls")}`)).toBeTruthy();
    expect(within(modal).getByRole("tabpanel", { name: "Create account" })).toBeTruthy();
  });

  it("dismisses with Escape and backdrop click without navigating or mutating the form host", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const trigger = screen.getByRole("button", { name: "Open sign-in modal" });

    await user.click(trigger);
    await user.type(await screen.findByLabelText("Email address"), "ada@example.com");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.activeElement).toBe(trigger);

    await user.click(trigger);
    const reopened = await screen.findByRole("dialog", { name: "Sign in to Nexora" });
    expect((within(reopened).getByLabelText("Email address") as HTMLInputElement).value).toBe("");
    const backdrop = await screen.findByTestId("auth-modal-backdrop");
    await user.click(backdrop);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it("validates locally, shows loading/disabled state, and rejects duplicate submissions", async () => {
    let resolveLogin!: (value: AuthUser) => void;
    const login = vi.fn(
      () =>
        new Promise<AuthUser>((resolve) => {
          resolveLogin = resolve;
        }),
    );
    const user = userEvent.setup();
    render(<Harness onLogin={login} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");

    const email = within(modal).getByLabelText("Email address");
    const password = within(modal).getByLabelText("Password");
    await user.type(email, "not-an-email");
    await user.type(password, "keep-this-password-in-memory");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));
    expect(within(modal).getByRole("alert").textContent).toContain("valid email");
    expect(email.getAttribute("aria-invalid")).toBe("true");
    expect((password as HTMLInputElement).value).toBe("keep-this-password-in-memory");
    expect(login).not.toHaveBeenCalled();

    await user.clear(email);
    await user.type(email, "ada@example.com");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));
    expect(login).toHaveBeenCalledTimes(1);
    const submit = within(modal).getByRole("button", { name: "Signing in…" }) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    expect(modal.getAttribute("aria-busy")).toBe("true");

    fireEvent.submit(within(modal).getByRole("tabpanel"));
    expect(login).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveLogin(signedInUser);
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("ignores a late authentication response after the modal unmounts", async () => {
    let resolveLogin!: (value: AuthUser) => void;
    const login = vi.fn(
      () =>
        new Promise<AuthUser>((resolve) => {
          resolveLogin = resolve;
        }),
    );
    const closed = vi.fn();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const user = userEvent.setup();
    const { unmount } = render(<Harness onLogin={login} onClosed={closed} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(modal).getByLabelText("Password"), "keep-this-password-in-memory");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));
    expect(login).toHaveBeenCalledTimes(1);

    unmount();
    await act(async () => {
      resolveLogin(signedInUser);
    });
    expect(closed).not.toHaveBeenCalled();
    expect(consoleError).not.toHaveBeenCalled();
  });

  it("shows actionable service failures without echoing credentials, tokens, or logging them", async () => {
    const secret = "very-secret-password-8842";
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const consoleLog = vi.spyOn(console, "log").mockImplementation(() => {});
    const login = vi.fn(async () => {
      throw {
        code: "login_failed",
        message: `Rejected password: ${secret}; token: ${"a".repeat(64)}; email ada@example.com`,
      };
    });
    const user = userEvent.setup();
    render(<Harness onLogin={login} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(modal).getByLabelText("Password"), secret);
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));

    const message = (await within(modal).findByRole("alert")).textContent ?? "";
    expect(message).toContain("Sign-in could not be completed");
    expect(message).not.toContain(secret);
    expect(message).not.toContain("ada@example.com");
    expect(message).not.toContain("a".repeat(64));
    expect(consoleError).not.toHaveBeenCalled();
    expect(consoleWarn).not.toHaveBeenCalled();
    expect(consoleLog).not.toHaveBeenCalled();
  });

  it("filters infrastructure details from otherwise-unclassified service errors", async () => {
    const login = vi.fn(async () => {
      throw { code: "auth_rejected", message: "Authentication failed at internal-auth.example:443/v1/auth/login" };
    });
    const user = userEvent.setup();
    render(<Harness onLogin={login} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(modal).getByLabelText("Password"), "correct horse battery staple");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));

    const message = (await within(modal).findByRole("alert")).textContent ?? "";
    expect(message).toContain("Sign-in could not be completed");
    expect(message).not.toContain("internal-auth.example");
    expect(message).not.toContain(":443");
  });

  it("reports unavailable account service and disables unsupported Google sign-in", async () => {
    getDeveloperDiagnosticsMock.mockResolvedValueOnce({
      ...unavailableDiagnostics,
      authServiceUrl: "unconfigured",
    });
    const google = vi.fn(async () => signedInUser);
    const user = userEvent.setup();
    render(<Harness onGoogleSignIn={google} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    expect(await within(modal).findByTestId("auth-backend-note")).toBeTruthy();

    const googleButton = within(modal).getByRole("button", { name: /Continue with Google/i }) as HTMLButtonElement;
    expect(googleButton.disabled).toBe(true);
    expect(googleButton.getAttribute("aria-describedby")).toBeTruthy();
    expect(google).not.toHaveBeenCalled();
    expect((within(modal).getByRole("button", { name: "Sign in" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("disables Google when the desktop client ID is absent without disabling email sign-in", async () => {
    getDeveloperDiagnosticsMock.mockResolvedValueOnce({
      ...unavailableDiagnostics,
      googleOauthConfigured: false,
    });
    const google = vi.fn(async () => signedInUser);
    const user = userEvent.setup();
    render(<Harness onGoogleSignIn={google} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");

    const googleButton = within(modal).getByRole("button", { name: /Continue with Google/i }) as HTMLButtonElement;
    expect(googleButton.disabled).toBe(true);
    expect(await within(modal).findByText(/Google sign-in is not configured in this desktop build/i)).toBeTruthy();
    expect((within(modal).getByRole("button", { name: "Sign in" }) as HTMLButtonElement).disabled).toBe(false);
    expect((within(modal).getByLabelText("Email address") as HTMLInputElement).disabled).toBe(false);
    expect(google).not.toHaveBeenCalled();
  });

  it("explains that account actions require the desktop shell in browser preview", async () => {
    isDesktopShellMock.mockReturnValue(false);
    const login = vi.fn(async () => signedInUser);
    const google = vi.fn(async () => signedInUser);
    const user = userEvent.setup();
    render(<Harness onLogin={login} onGoogleSignIn={google} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");

    expect(await within(modal).findByText(/available only in the Nexora desktop app/i)).toBeTruthy();
    expect((within(modal).getByRole("button", { name: "Sign in" }) as HTMLButtonElement).disabled).toBe(true);
    expect((within(modal).getByRole("button", { name: /Continue with Google/i }) as HTMLButtonElement).disabled).toBe(true);
    expect(login).not.toHaveBeenCalled();
    expect(google).not.toHaveBeenCalled();
  });

  it("turns a real Google-provider-unavailable response into a clear unavailable state", async () => {
    const google = vi.fn(async () => {
      throw { code: "dependency_unavailable", message: "Google OAuth is not configured." };
    });
    const user = userEvent.setup();
    render(<Harness onGoogleSignIn={google} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    const googleButton = within(modal).getByRole("button", { name: /Continue with Google/i }) as HTMLButtonElement;
    await waitFor(() => expect(googleButton.disabled).toBe(false));
    await user.click(googleButton);

    expect(google).toHaveBeenCalledTimes(1);
    expect(await within(modal).findByText(/Google sign-in isn't configured for this account service/i)).toBeTruthy();
    expect(within(modal).getByRole("status")).toBeTruthy();
    expect(googleButton.disabled).toBe(true);
    expect(within(modal).getByRole("button", { name: "Sign in" })).toBeTruthy();
  });

  it("keeps the signed-out modal and account state unchanged when Google authorization is cancelled", async () => {
    const google = vi.fn(async () => {
      throw { code: "cancelled", message: "Google sign-in was cancelled." };
    });
    const user = userEvent.setup();
    render(<Harness onGoogleSignIn={google} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    const googleButton = within(modal).getByRole("button", { name: /Continue with Google/i });
    await user.click(googleButton);

    expect(google).toHaveBeenCalledTimes(1);
    expect(await within(modal).findByRole("tabpanel", { name: "Sign in" })).toBeTruthy();
    expect(within(modal).queryByRole("alert")).toBeNull();
    expect(within(modal).getByRole("button", { name: /Continue with Google/i })).toBeTruthy();
  });

  it("maps network failures to a retryable message without exposing service details", async () => {
    const login = vi.fn(async () => {
      throw { code: "network_error", message: "connect ECONNREFUSED internal-auth-host:443" };
    });
    const user = userEvent.setup();
    render(<Harness onLogin={login} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(modal).getByLabelText("Password"), "keep-this-password-in-memory");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));

    const message = (await within(modal).findByRole("alert")).textContent ?? "";
    expect(message).toContain("Can't reach the Nexora account service");
    expect(message).not.toContain("internal-auth-host");
    expect(message).not.toContain("ECONNREFUSED");
  });

  it("shows the real registration status without claiming email delivery when the service says it is unconfigured", async () => {
    const register = vi.fn(async () => ({
      status: "verification_pending",
      emailDelivery: "unconfigured",
      message: "Email delivery is not configured on this server; no verification email was sent.",
    }));
    const user = userEvent.setup();
    render(<Harness onRegister={register} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    await user.click(within(modal).getByRole("tab", { name: "Create account" }));
    await user.type(within(modal).getByLabelText("Name"), "Ada Lovelace");
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(modal).getByLabelText("Password"), "correct-horse-battery");
    await user.type(within(modal).getByLabelText("Confirm password"), "correct-horse-battery");
    await user.click(within(modal).getByRole("button", { name: "Create account" }));

    expect(register).toHaveBeenCalledTimes(1);
    expect(await within(modal).findByLabelText("Verification code")).toBeTruthy();
    const status = within(modal).getByRole("status").textContent ?? "";
    expect(status).toContain("no verification email was sent");
    expect(status).not.toMatch(/check your email/i);
  });

  it("keeps recovery honest when the service accepts a request but confirms no email delivery", async () => {
    const recover = vi.fn(async () => ({
      status: "accepted",
      emailDelivery: "unconfigured",
      message: "Request accepted, but no email was sent: outgoing email is not configured in this build.",
    }));
    const user = userEvent.setup();
    render(<Harness onRecoverPassword={recover} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog");
    await user.click(within(modal).getByRole("button", { name: "Forgot password?" }));
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.click(within(modal).getByRole("button", { name: "Request password reset" }));

    expect(recover).toHaveBeenCalledWith("ada@example.com");
    expect(within(modal).queryByLabelText("Reset token")).toBeNull();
    expect(within(modal).getByRole("status").textContent).toContain("no email was sent");
    expect(within(modal).getByRole("button", { name: "Already have a reset token?" })).toBeTruthy();
  });

  it("validates one-time link tokens and shows consistent dark/light token-driven rendering", async () => {
    const reset = vi.fn(async () => ({ status: "reset", message: "Password updated." }));
    const user = userEvent.setup();
    const { rerender } = render(<Harness initialMode="reset" initialToken="short-token" onResetPassword={reset} />);
    document.documentElement.dataset.theme = "dark";
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const modal = await screen.findByRole("dialog", { name: "Set a new password" });
    expect(modal.className).toBe("auth-modal");
    await user.type(within(modal).getByLabelText("New password"), "strong-password-9922");
    await user.click(within(modal).getByRole("button", { name: "Update password" }));
    expect(within(modal).getByRole("alert").textContent).toContain("64-character reset token");
    expect(reset).not.toHaveBeenCalled();

    await user.click(within(modal).getByRole("button", { name: "Close authentication dialog" }));
    document.documentElement.dataset.theme = "light";
    rerender(<Harness initialMode="reset" initialToken={"c".repeat(64)} onResetPassword={reset} />);
    await user.click(screen.getByRole("button", { name: "Open sign-in modal" }));
    const lightModal = await screen.findByRole("dialog", { name: "Set a new password" });
    expect(lightModal.className).toBe("auth-modal");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(within(lightModal).getByLabelText("Reset token")).toBeTruthy();
  });
});
