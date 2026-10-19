import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ModelManagerPage } from "./ModelManagerPage";
import { makeStatus } from "../test/fixtures";

vi.mock("../lib/inference", () => ({
  fetchSystemInfo: vi.fn().mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 22.5,
    platform: "windows",
  }),
  probeRuntime: vi.fn(),
  validateModel: vi.fn(),
  saveSettings: vi.fn(),
  loadModel: vi.fn(),
}));

import { fetchSystemInfo } from "../lib/inference";

describe("Phase 28 — ModelManagerPage component", () => {
  beforeEach(() => {
    vi.mocked(fetchSystemInfo).mockResolvedValue({
      totalRamGib: 32,
      availableRamGib: 22.5,
      platform: "windows",
    });
  });

  it("renders ModelManagerPage with header, hardware specs, and engine status", async () => {
    const status = makeStatus({
      state: "ready",
      runtimePath: "C:\\Tools\\llama-server.exe",
      model: "qwen-coder.gguf",
    });

    render(
      <ModelManagerPage
        status={status}
        statusError={null}
        desktop={true}
        unloading={false}
        onStatus={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Local models" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "System hardware & runtime target" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Engine status" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Local model setup" })).toBeTruthy();

    expect(await screen.findByText("22.5 of 32 GiB available")).toBeTruthy();
    expect(screen.getByText("llama.cpp b11714")).toBeTruthy();
  });

  it("displays honest notice when in browser preview mode", () => {
    render(
      <ModelManagerPage
        status={null}
        statusError={null}
        desktop={false}
        unloading={false}
        onStatus={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("status")).toBeTruthy();
    expect(screen.getByText(/This is the browser preview/i)).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Local model setup" })).toBeNull();
  });

  it("invokes onRefresh when clicking Refresh status", async () => {
    const onRefresh = vi.fn();
    const user = userEvent.setup();

    render(
      <ModelManagerPage
        status={null}
        statusError={null}
        desktop={true}
        unloading={false}
        onStatus={vi.fn()}
        onRefresh={onRefresh}
      />,
    );

    const refreshButton = screen.getByRole("button", { name: "Refresh status" });
    await user.click(refreshButton);
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("triggers onUnload when engine is ready and user requests stop", async () => {
    const onUnload = vi.fn();
    const user = userEvent.setup();

    const status = makeStatus({
      state: "ready",
      runtimePath: "C:\\Tools\\llama-server.exe",
      model: "model.gguf",
    });

    render(
      <ModelManagerPage
        status={status}
        statusError={null}
        desktop={true}
        unloading={false}
        onStatus={vi.fn()}
        onRefresh={vi.fn()}
        onUnload={onUnload}
      />,
    );

    const stopButton = screen.getByRole("button", { name: "Stop engine" });
    await user.click(stopButton);
    expect(onUnload).toHaveBeenCalledTimes(1);
  });
});
