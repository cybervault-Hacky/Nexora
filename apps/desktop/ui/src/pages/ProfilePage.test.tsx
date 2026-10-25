import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { getProfile, updateProfile } = vi.hoisted(() => ({ getProfile: vi.fn(), updateProfile: vi.fn() }));
vi.mock("../lib/auth", async (original) => ({
  ...await original<typeof import("../lib/auth")>(),
  accountGetProfile: getProfile,
  accountUpdateProfile: updateProfile,
}));
import { ProfilePage } from "./ProfilePage";

const user = { id: "account-1", name: "Ada", email: "ada@example.com", emailVerified: true };
const profile = { ...user, providers: ["password"], hasPassword: true, planId: "free", planName: "Free", createdAt: "2024-01-01T00:00:00Z" };

describe("private profile", () => {
  beforeEach(() => { getProfile.mockReset().mockResolvedValue(profile); updateProfile.mockReset(); });
  it("does not fetch private data while signed out", () => {
    render(<ProfilePage user={null} onRequestSignIn={vi.fn()} onProfileSaved={vi.fn()} />);
    expect(getProfile).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
  });
  it("loads the matching profile and persists the supported display name only", async () => {
    const saved = vi.fn();
    updateProfile.mockResolvedValue({ ...profile, name: "Grace Hopper" });
    render(<ProfilePage user={user} onRequestSignIn={vi.fn()} onProfileSaved={saved} />);
    expect(await screen.findByRole("heading", { name: "Ada" })).toBeTruthy();
    const actor = userEvent.setup();
    await actor.click(screen.getByRole("button", { name: "Edit" }));
    await actor.clear(screen.getByRole("textbox", { name: "Display name" }));
    await actor.type(screen.getByRole("textbox", { name: "Display name" }), "Grace Hopper");
    await actor.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(updateProfile).toHaveBeenCalledWith({ name: "Grace Hopper" }));
    expect(await screen.findByRole("heading", { name: "Grace Hopper" })).toBeTruthy();
    expect(saved).toHaveBeenCalled();
    expect(screen.queryByText("ada@example.com")).toBeNull();
  });
  it("rejects invalid names without sending a request", async () => {
    render(<ProfilePage user={user} onRequestSignIn={vi.fn()} onProfileSaved={vi.fn()} />);
    await screen.findByRole("heading", { name: "Ada" });
    const actor = userEvent.setup();
    await actor.click(screen.getByRole("button", { name: "Edit" }));
    await actor.clear(screen.getByRole("textbox", { name: "Display name" }));
    await actor.click(screen.getByRole("button", { name: "Save changes" }));
    expect(updateProfile).not.toHaveBeenCalled();
    expect(screen.getByText(/Enter a display name/)).toBeTruthy();
  });
});
