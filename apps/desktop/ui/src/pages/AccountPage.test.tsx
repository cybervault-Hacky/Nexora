import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockUseAuth, accountGetProfileMock, accountUpdateProfileMock } = vi.hoisted(() => ({
  mockUseAuth: vi.fn(),
  accountGetProfileMock: vi.fn(),
  accountUpdateProfileMock: vi.fn(),
}));

vi.mock("../features/auth/useAuth", () => ({
  useAuth: () => mockUseAuth(),
}));

vi.mock("../lib/auth", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/auth")>();
  return {
    ...actual,
    accountGetProfile: (...args: unknown[]) => accountGetProfileMock(...args),
    accountUpdateProfile: (...args: unknown[]) => accountUpdateProfileMock(...args),
  };
});

import type { AuthUser, AccountProfile } from "../lib/auth";
import { AccountPage } from "./AccountPage";

function makeAccountProfile(overrides: Partial<AccountProfile> = {}): AccountProfile {
  return {
    id: "usr_42",
    name: "Ada Lovelace",
    email: "ada@example.com",
    emailVerified: true,
    hasPassword: true,
    providers: ["password", "google"],
    planId: "free",
    planName: "Free",
    createdAt: "2023-01-12T00:00:00Z",
    ...overrides,
  };
}

function makeAuth(overrides: Record<string, unknown> = {}) {
  const user: AuthUser = {
    id: "usr_42",
    name: "Cached session name",
    email: "cached@example.test",
    emailVerified: false,
  };
  return {
    user,
    isAuthenticated: true,
    loading: false,
    logout: vi.fn().mockResolvedValue(true),
    changePassword: vi.fn().mockResolvedValue({ message: "Password updated successfully." }),
    resendVerification: vi.fn().mockResolvedValue({ message: "Verification email sent." }),
    refreshSession: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("AccountPage profile integration", () => {
  beforeEach(() => {
    mockUseAuth.mockReset();
    accountGetProfileMock.mockReset().mockResolvedValue(makeAccountProfile());
    accountUpdateProfileMock.mockReset();
  });

  it("renders guest state without fetching a private profile", () => {
    mockUseAuth.mockReturnValue({
      ...makeAuth(),
      user: null,
      isAuthenticated: false,
    });

    render(<AccountPage />);

    expect(screen.getByRole("heading", { name: "Developer Account" })).toBeTruthy();
    expect(screen.getByText(/Local Guest Mode/i)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Create account" })).toBeTruthy();
    expect(accountGetProfileMock).not.toHaveBeenCalled();
    expect(screen.queryByLabelText(/Current password/i)).toBeNull();
  });

  it("loads trusted profile details instead of presenting the cached session snapshot as current", async () => {
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    expect(await screen.findByText("Ada Lovelace")).toBeTruthy();
    expect(screen.getByText("ada@example.com")).toBeTruthy();
    expect(screen.getByText("usr_42")).toBeTruthy();
    expect(screen.getByText("Email and password, Google")).toBeTruthy();
    expect(screen.getByText(/Verified/i)).toBeTruthy();
    expect(screen.getByText("Jan 12, 2023")).toBeTruthy();
    expect(screen.queryByText("Cached session name")).toBeNull();
    expect(screen.queryByText("cached@example.test")).toBeNull();
    expect(screen.queryByText("Free")).toBeNull();
    expect(screen.getByLabelText(/Current password/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
    expect(accountGetProfileMock).toHaveBeenCalledTimes(1);
  });

  it("shows unavailable optional metadata without inventing profile values", async () => {
    accountGetProfileMock.mockResolvedValue(makeAccountProfile({ createdAt: "not-a-date" }));
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    await screen.findByText("Ada Lovelace");
    const createdRow = screen.getByText("Account created").closest(".settings-row");
    expect(createdRow?.textContent).toContain("Not available");
  });

  it("keeps the profile in a loading state until the authenticated profile request resolves", async () => {
    let finishLoad!: (profile: AccountProfile) => void;
    accountGetProfileMock.mockReturnValue(
      new Promise<AccountProfile>((resolve) => {
        finishLoad = resolve;
      }),
    );
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    expect(screen.getByRole("status").textContent).toContain("Loading your account profile");
    expect(screen.queryByText("Cached session name")).toBeNull();

    finishLoad(makeAccountProfile());
    expect(await screen.findByText("Ada Lovelace")).toBeTruthy();
  });

  it("does not show a previous account profile while the next session is loading", async () => {
    const previousUser: AuthUser = {
      id: "user-a",
      name: "Previous cached name",
      email: "previous-cached@example.test",
      emailVerified: true,
    };
    const nextUser: AuthUser = {
      id: "user-b",
      name: "Next cached name",
      email: "next-cached@example.test",
      emailVerified: true,
    };
    let finishNextProfile!: (profile: AccountProfile) => void;
    accountGetProfileMock
      .mockResolvedValueOnce(
        makeAccountProfile({
          id: previousUser.id,
          name: "Previous Account",
          email: "previous@example.test",
        }),
      )
      .mockReturnValueOnce(
        new Promise<AccountProfile>((resolve) => {
          finishNextProfile = resolve;
        }),
      );
    mockUseAuth.mockReturnValue(makeAuth({ user: previousUser }));

    const { rerender } = render(<AccountPage />);
    expect(await screen.findByText("Previous Account")).toBeTruthy();

    mockUseAuth.mockReturnValue(makeAuth({ user: nextUser }));
    rerender(<AccountPage />);

    expect(screen.queryByText("Previous Account")).toBeNull();
    expect(screen.queryByText("previous@example.test")).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("Loading your account profile");

    finishNextProfile(
      makeAccountProfile({
        id: nextUser.id,
        name: "Next Account",
        email: "next@example.test",
      }),
    );
    expect(await screen.findByText("Next Account")).toBeTruthy();
  });

  it("edits only the display name and uses the server-returned profile after success", async () => {
    const user = userEvent.setup();
    const auth = makeAuth();
    let finishUpdate!: (profile: AccountProfile) => void;
    accountUpdateProfileMock.mockReturnValue(
      new Promise<AccountProfile>((resolve) => {
        finishUpdate = resolve;
      }),
    );
    mockUseAuth.mockReturnValue(auth);

    render(<AccountPage />);

    await screen.findByText("Ada Lovelace");
    await user.click(screen.getByRole("button", { name: "Edit name" }));
    const nameInput = screen.getByRole("textbox", { name: "Display name" });
    await user.clear(nameInput);
    await user.type(nameInput, "  Ada Byron  ");
    await user.click(screen.getByRole("button", { name: "Save name" }));

    expect(accountUpdateProfileMock).toHaveBeenCalledWith({ name: "Ada Byron" });
    expect(screen.getByRole("button", { name: "Saving…" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Display name" })).toBeTruthy();

    finishUpdate(makeAccountProfile({ name: "Ada Byron" }));
    expect(await screen.findByText("Ada Byron")).toBeTruthy();
    expect(await screen.findByText("Display name updated.")).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "Display name" })).toBeNull();
    expect(auth.refreshSession).toHaveBeenCalledTimes(1);
  });

  it("rejects a display name over the server limit before making a protected update", async () => {
    const user = userEvent.setup();
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    await screen.findByText("Ada Lovelace");
    await user.click(screen.getByRole("button", { name: "Edit name" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Display name" }), {
      target: { value: "x".repeat(81) },
    });
    await user.click(screen.getByRole("button", { name: "Save name" }));

    expect((await screen.findByRole("alert")).textContent).toContain(
      "Display name must be 80 characters or fewer.",
    );
    expect(accountUpdateProfileMock).not.toHaveBeenCalled();
  });

  it("shows the backend validation error without claiming the profile update succeeded", async () => {
    const user = userEvent.setup();
    accountUpdateProfileMock.mockRejectedValue({
      code: "validation_failed",
      message: "Display name must be between 1 and 80 characters.",
    });
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    await screen.findByText("Ada Lovelace");
    await user.click(screen.getByRole("button", { name: "Edit name" }));
    await user.clear(screen.getByRole("textbox", { name: "Display name" }));
    await user.type(screen.getByRole("textbox", { name: "Display name" }), "New Name");
    await user.click(screen.getByRole("button", { name: "Save name" }));

    expect((await screen.findByRole("alert")).textContent).toContain(
      "Display name must be between 1 and 80 characters.",
    );
    expect(screen.queryByText("Display name updated.")).toBeNull();
    expect(screen.getByRole("textbox", { name: "Display name" })).toBeTruthy();
  });

  it("does not display stale cached identity when profile loading fails and keeps sign-out available", async () => {
    accountGetProfileMock.mockRejectedValue({
      code: "dependency_unavailable",
      message: "database connection failed at postgres://secret.invalid",
    });
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("temporarily unavailable");
    expect(alert.textContent).not.toContain("postgres");
    expect(alert.textContent).not.toContain("secret.invalid");
    expect(screen.queryByText("Cached session name")).toBeNull();
    expect(screen.queryByLabelText(/Current password/i)).toBeNull();
    expect(screen.getByRole("button", { name: "Retry profile load" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
  });

  it("rejects a profile response whose account ID does not match the active session", async () => {
    accountGetProfileMock.mockResolvedValue(makeAccountProfile({ id: "another-account" }));
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    expect((await screen.findByRole("alert")).textContent).toContain("invalid profile response");
    expect(screen.queryByText("ada@example.com")).toBeNull();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
  });

  it("supports a Google-only profile without presenting password changes as available", async () => {
    accountGetProfileMock.mockResolvedValue(
      makeAccountProfile({ hasPassword: false, providers: ["google"] }),
    );
    mockUseAuth.mockReturnValue(makeAuth());

    render(<AccountPage />);

    expect(await screen.findByText("Google")).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Password security" })).toBeNull();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
  });

  it("validates and submits password change successfully for an account with a password", async () => {
    const user = userEvent.setup();
    const changePasswordMock = vi.fn().mockResolvedValue({
      status: "success",
      message: "Password updated successfully.",
    });

    mockUseAuth.mockReturnValue(makeAuth({ changePassword: changePasswordMock }));
    render(<AccountPage />);

    const currentInput = await screen.findByLabelText(/Current password/i);
    const newInput = screen.getByLabelText(/New password \(at least 8 characters\)/i);
    const confirmInput = screen.getByLabelText(/Confirm new password/i);
    const submitBtn = screen.getByRole("button", { name: "Update password" });

    await user.type(currentInput, "CurrentSecret123");
    await user.type(newInput, "ValidNewSecret123");
    await user.type(confirmInput, "DifferentSecret123");
    await user.click(submitBtn);

    expect(await screen.findByText(/Passwords do not match/i)).toBeTruthy();
    expect(changePasswordMock).not.toHaveBeenCalled();

    await user.clear(confirmInput);
    await user.type(confirmInput, "ValidNewSecret123");
    await user.click(submitBtn);

    await waitFor(() => {
      expect(changePasswordMock).toHaveBeenCalledWith({
        currentPassword: "CurrentSecret123",
        newPassword: "ValidNewSecret123",
      });
    });
    expect(await screen.findByText("Password updated successfully.")).toBeTruthy();
  });

  it("handles unverified email and resends verification to the server profile address", async () => {
    const user = userEvent.setup();
    const resendMock = vi.fn().mockResolvedValue({
      status: "sent",
      message: "Verification email sent.",
    });
    mockUseAuth.mockReturnValue(makeAuth({ resendVerification: resendMock }));
    accountGetProfileMock.mockResolvedValue(makeAccountProfile({ emailVerified: false }));

    render(<AccountPage />);

    expect(await screen.findByText(/Unverified/i)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /Resend email/i }));

    await waitFor(() => {
      expect(resendMock).toHaveBeenCalledWith("ada@example.com");
    });
    expect(await screen.findByText("Verification email sent.")).toBeTruthy();
  });
});
