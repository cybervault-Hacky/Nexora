import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mockUseAuth = vi.fn();

vi.mock("../features/auth/useAuth", () => ({
  useAuth: () => mockUseAuth(),
}));

import { AccountPage } from "./AccountPage";

describe("Phase 25 — AccountPage", () => {
  beforeEach(() => {
    mockUseAuth.mockReset();
  });

  it("renders guest state with links to sign in when unauthenticated", () => {
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      loading: false,
      logout: vi.fn(),
      changePassword: vi.fn(),
      resendVerification: vi.fn(),
    });

    render(<AccountPage />);

    expect(screen.getByRole("heading", { name: "Developer Account" })).toBeTruthy();
    expect(screen.getByText(/Local Guest Mode/i)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Create account" })).toBeTruthy();
    expect(screen.queryByLabelText(/Current password/i)).toBeNull();
  });

  it("renders profile details and password form when authenticated", () => {
    mockUseAuth.mockReturnValue({
      user: {
        id: "usr_42",
        name: "Ada Lovelace",
        email: "ada@example.com",
        emailVerified: true,
      },
      isAuthenticated: true,
      loading: false,
      logout: vi.fn(),
      changePassword: vi.fn(),
      resendVerification: vi.fn(),
    });

    render(<AccountPage />);

    expect(screen.getByText("Ada Lovelace")).toBeTruthy();
    expect(screen.getByText("ada@example.com")).toBeTruthy();
    expect(screen.getByText("usr_42")).toBeTruthy();
    expect(screen.getByText(/Verified/i)).toBeTruthy();
    expect(screen.getByLabelText(/Current password/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
  });

  it("validates and submits password change successfully", async () => {
    const user = userEvent.setup();
    const changePasswordMock = vi.fn().mockResolvedValue({
      status: "success",
      message: "Password updated successfully.",
    });

    mockUseAuth.mockReturnValue({
      user: {
        id: "usr_42",
        name: "Ada Lovelace",
        email: "ada@example.com",
        emailVerified: true,
      },
      isAuthenticated: true,
      loading: false,
      logout: vi.fn(),
      changePassword: changePasswordMock,
      resendVerification: vi.fn(),
    });

    render(<AccountPage />);

    const currentInput = screen.getByLabelText(/Current password/i);
    const newInput = screen.getByLabelText(/New password \(at least 8 characters\)/i);
    const confirmInput = screen.getByLabelText(/Confirm new password/i);
    const submitBtn = screen.getByRole("button", { name: "Update password" });

    // Submit with mismatch
    await user.type(currentInput, "CurrentSecret123");
    await user.type(newInput, "ValidNewSecret123");
    await user.type(confirmInput, "DifferentSecret123");
    await user.click(submitBtn);

    expect(await screen.findByText(/Passwords do not match/i)).toBeTruthy();
    expect(changePasswordMock).not.toHaveBeenCalled();

    // Fix confirmation
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

  it("handles unverified email and triggers resend verification", async () => {
    const user = userEvent.setup();
    const resendMock = vi.fn().mockResolvedValue({
      status: "sent",
      message: "Verification email sent.",
    });

    mockUseAuth.mockReturnValue({
      user: {
        id: "usr_42",
        name: "Ada Lovelace",
        email: "ada@example.com",
        emailVerified: false,
      },
      isAuthenticated: true,
      loading: false,
      logout: vi.fn(),
      changePassword: vi.fn(),
      resendVerification: resendMock,
    });

    render(<AccountPage />);

    expect(screen.getByText(/Unverified/i)).toBeTruthy();
    const resendBtn = screen.getByRole("button", { name: /Resend email/i });
    await user.click(resendBtn);

    await waitFor(() => {
      expect(resendMock).toHaveBeenCalledWith("ada@example.com");
    });
    expect(await screen.findByText("Verification email sent.")).toBeTruthy();
  });
});
