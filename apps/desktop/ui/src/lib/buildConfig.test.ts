// @vitest-environment node
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, normalize, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import viteConfig from "../../../vite.config";

/**
 * Coherence checks for the native build configuration.
 *
 * These do not build anything. They catch the class of mistake that only
 * surfaces on a Windows machine — a missing bundle icon, a dev port that two
 * files disagree about, a capability granted but never needed — by comparing
 * the real configuration files with each other.
 */

const appRoot = resolve(process.cwd());
const tauriDir = join(appRoot, "src-tauri");

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

interface TauriWindow {
  label: string;
  title: string;
  minWidth: number;
  minHeight: number;
}

interface TauriConfig {
  productName: string;
  version: string;
  identifier: string;
  build: {
    beforeDevCommand: string;
    devUrl: string;
    beforeBuildCommand: string;
    frontendDist: string;
  };
  app: {
    windows: TauriWindow[];
    security: { csp: string; devCsp: string };
  };
  bundle: {
    active: boolean;
    targets: string[];
    icon: string[];
    windows: { nsis: { installMode: string } };
  };
}

interface PackageJson {
  name: string;
  version: string;
  scripts: Record<string, string>;
  packageManager?: string;
}

interface Capability {
  identifier: string;
  windows: string[];
  permissions: string[];
}

const tauri = readJson<TauriConfig>(join(tauriDir, "tauri.conf.json"));
const capability = readJson<Capability>(
  join(tauriDir, "capabilities", "main.json"),
);
const appPackage = readJson<PackageJson>(join(appRoot, "package.json"));
const rootPackage = readJson<PackageJson>(join(appRoot, "../../package.json"));
const toolchain = readFileSync(join(appRoot, "../../rust-toolchain.toml"), "utf8");

describe("Tauri window and build wiring", () => {
  it("serves the dev window on the port Vite actually binds", () => {
    const dev = new URL(tauri.build.devUrl);
    const server = viteConfig.server as {
      host?: string;
      port?: number;
      strictPort?: boolean;
    };

    expect(dev.hostname).toBe("localhost");
    expect(server.port).toBe(Number(dev.port));
    // Without strictPort, Vite silently moves to another port and the Tauri
    // window loads an empty page.
    expect(server.strictPort).toBe(true);
  });

  it("points the bundle at the directory Vite writes", () => {
    const outDir = (viteConfig.build?.outDir ?? "dist").replace(/^\.\//, "");
    const expected = normalize(join(appRoot, "ui", outDir));

    // `dist` is a build output and is not committed, so only the paths are
    // compared; the CI build step is what proves Vite writes there.
    expect(normalize(resolve(tauriDir, tauri.build.frontendDist))).toBe(expected);
  });

  it("targets a WebView2-compatible baseline", () => {
    // Windows 10/11 ship Edge WebView2, whose Chromium floor is far above this;
    // pinning the target keeps the output from assuming newer syntax.
    expect(viteConfig.build?.target).toBe("chrome105");
  });

  it("runs commands that exist in the app package", () => {
    const scriptOf = (value: string): string => {
      const [, name = ""] = /^pnpm\s+(\S+)/.exec(value) ?? [];

      return name;
    };

    const dev = scriptOf(tauri.build.beforeDevCommand);
    const build = scriptOf(tauri.build.beforeBuildCommand);

    expect(dev).not.toBe("");
    expect(build).not.toBe("");
    expect(Object.keys(appPackage.scripts)).toContain(dev);
    expect(Object.keys(appPackage.scripts)).toContain(build);
  });

  it("keeps one version across the crate, the app, and the workspace", () => {
    expect(tauri.version).toBe(appPackage.version);
    expect(tauri.version).toBe(rootPackage.version);
    expect(tauri.productName).toBe("Nexora");
    expect(tauri.identifier).toMatch(/^[a-z0-9]+(\.[a-z0-9-]+)+$/);
  });
});

describe("Windows bundle", () => {
  it("produces an NSIS installer for the current user", () => {
    expect(tauri.bundle.active).toBe(true);
    expect(tauri.bundle.targets).toEqual(["nsis"]);
    // Per-user avoids an elevation prompt for an app that only writes to its
    // own config directory.
    expect(tauri.bundle.windows.nsis.installMode).toBe("currentUser");
  });

  it("references only icon files that exist", () => {
    expect(tauri.bundle.icon.length).toBeGreaterThan(0);

    for (const icon of tauri.bundle.icon) {
      expect({ icon, present: existsSync(join(tauriDir, icon)) }).toEqual({
        icon,
        present: true,
      });
    }

    // Windows packaging requires an .ico.
    expect(tauri.bundle.icon.some((icon) => icon.endsWith(".ico"))).toBe(true);
  });

  it("declares the window the capability targets", () => {
    expect(tauri.app.windows).toHaveLength(1);
    expect(tauri.app.windows[0]?.label).toBe("main");
    expect(capability.windows).toEqual(["main"]);
  });
});

describe("webview policy", () => {
  it("grants no capability the app does not use", () => {
    const allowed = new Set([
      "core:default",
      "core:window:allow-minimize",
      "core:window:allow-toggle-maximize",
      "core:window:allow-close",
      "core:window:allow-start-dragging",
    ]);

    for (const permission of capability.permissions) {
      expect({ permission, allowed: allowed.has(permission) }).toEqual({
        permission,
        allowed: true,
      });
    }
  });

  it("adds no filesystem, shell, or HTTP plugin permissions", () => {
    const forbidden = /^(fs|shell|http|process|dialog|os):/;

    expect(capability.permissions.filter((permission) => forbidden.test(permission))).toEqual(
      [],
    );
  });

  it("keeps script sources local and network access loopback-only", () => {
    const csp = tauri.app.security.csp;

    expect(csp).not.toContain("unsafe-eval");
    expect(csp).toContain("script-src 'self'");

    const connect = /connect-src ([^;]+);?/.exec(csp)?.[1]?.trim().split(/\s+/) ?? [];

    expect(connect).toEqual(["ipc:", "http://ipc.localhost"]);

    // No wildcard or remote origin anywhere in the policy.
    expect(csp).not.toMatch(/\*/);
    expect(csp).not.toMatch(/https?:\/\/(?!ipc\.localhost|asset\.localhost)/);
  });
});

describe("toolchain reproducibility", () => {
  it("pins an exact Rust version with the components CI uses", () => {
    const channel = /channel\s*=\s*"([^"]+)"/.exec(toolchain)?.[1];

    // "stable" moves under the project; a Windows build must be repeatable.
    expect(channel).toMatch(/^\d+\.\d+\.\d+$/);
    expect(toolchain).toContain("rustfmt");
    expect(toolchain).toContain("clippy");
  });

  it("pins the package manager the lockfile was written with", () => {
    expect(rootPackage.packageManager).toMatch(/^pnpm@\d+\.\d+\.\d+$/);
  });

  it("keeps a single workspace package and a committed lockfile", () => {
    expect(existsSync(join(appRoot, "../../pnpm-lock.yaml"))).toBe(true);
    expect(existsSync(join(appRoot, "../../pnpm-workspace.yaml"))).toBe(true);
    expect(isAbsolute(tauri.build.frontendDist)).toBe(false);
    expect(dirname(join(tauriDir, tauri.build.frontendDist))).toBe(
      normalize(join(appRoot, "ui")),
    );
  });
});
