import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// jsdom does not implement every animation primitive the shell uses.
if (typeof globalThis.requestAnimationFrame !== "function") {
  globalThis.requestAnimationFrame = (callback: FrameRequestCallback) =>
    setTimeout(() => callback(performance.now()), 0) as unknown as number;
  globalThis.cancelAnimationFrame = (handle: number) => clearTimeout(handle);
}

afterEach(() => {
  cleanup();
});
