import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    setupFiles: ["ui/src/test/setup.ts"],
    include: ["ui/src/**/*.test.{ts,tsx}"],
    restoreMocks: true,
  },
});
