import { describe, expect, it } from "vitest";
import {
  FLOW_ROUTES,
  ROUTES,
  isFlowRoute,
  navigateTo,
  parseRoute,
  routeHref,
} from "./router";

describe("parseRoute", () => {
  it("maps the empty hash and root paths directly to the AI Chat workspace", () => {
    expect(parseRoute("").id).toBe("chat");
    expect(parseRoute("#").id).toBe("chat");
    expect(parseRoute("#/").id).toBe("chat");
    expect(parseRoute("#/chat").id).toBe("chat");
    expect(parseRoute("#/chat/").id).toBe("chat");
    expect(parseRoute("#/search").id).toBe("chat");
    expect(parseRoute("#/search-chats").id).toBe("chat");
  });

  it("maps every primary desktop workspace route and aliases cleanly", () => {
    expect(parseRoute("#/home").id).toBe("home");
    expect(parseRoute("#/dashboard").id).toBe("home");
    expect(parseRoute("#/projects").id).toBe("projects");
    expect(parseRoute("#/code").id).toBe("code");
    expect(parseRoute("#/intelligence").id).toBe("code");
    expect(parseRoute("#/models").id).toBe("models");
    expect(parseRoute("#/engine").id).toBe("models");
    expect(parseRoute("#/plans").id).toBe("plans");
    expect(parseRoute("#/billing").id).toBe("plans");
    expect(parseRoute("#/pricing").id).toBe("plans");
    expect(parseRoute("#/account").id).toBe("account");
    expect(parseRoute("#/settings").id).toBe("settings");
    expect(parseRoute("#/preferences").id).toBe("settings");
    expect(parseRoute("#/licensing").id).toBe("licensing");
  });

  it("never replaces the desktop workspace with a standalone flow window", () => {
    for (const id of FLOW_ROUTES) {
      expect(isFlowRoute(id)).toBe(false);
    }
    expect(isFlowRoute("chat")).toBe(false);
  });

  it("returns notFound with original path for unknown routes", () => {
    const unknown = parseRoute("#/non-existent-route");
    expect(unknown).toEqual({ id: "notFound", path: "/non-existent-route" });
  });

  it("navigates across routes without duplicate history entries", () => {
    window.location.hash = "";
    navigateTo("chat");
    expect(parseRoute(window.location.hash).id).toBe("chat");
    navigateTo("plans");
    expect(window.location.hash).toBe(routeHref("plans"));
    expect(ROUTES.some((r) => r.id === "plans")).toBe(true);
  });
});
