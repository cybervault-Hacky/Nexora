import { afterEach, describe, expect, it } from "vitest";
import { isFlowRoute, navigateTo, parseRoute, routeHref, ROUTES } from "./router";

afterEach(() => {
  window.location.hash = "";
});

describe("parseRoute", () => {
  it("maps the empty hash and root paths to the Welcome entry screen", () => {
    expect(parseRoute("").id).toBe("welcome");
    expect(parseRoute("#").id).toBe("welcome");
    expect(parseRoute("#/").id).toBe("welcome");
    expect(parseRoute("#/welcome").id).toBe("welcome");
    expect(parseRoute("#/welcome/").id).toBe("welcome");
  });

  it("maps each registered page path", () => {
    expect(parseRoute("#/onboarding").id).toBe("onboarding");
    expect(parseRoute("#/signin").id).toBe("signin");
    expect(parseRoute("#/signup").id).toBe("signup");
    expect(parseRoute("#/verify-email").id).toBe("verify");
    expect(parseRoute("#/recover-password").id).toBe("recover");
    expect(parseRoute("#/reset-password").id).toBe("reset");
    expect(parseRoute("#/home").id).toBe("home");
    expect(parseRoute("#/chat").id).toBe("chat");
    expect(parseRoute("#/projects").id).toBe("projects");
    expect(parseRoute("#/code").id).toBe("code");
    expect(parseRoute("#/intelligence").id).toBe("code");
    expect(parseRoute("#/models").id).toBe("models");
    expect(parseRoute("#/model-setup").id).toBe("models");
    expect(parseRoute("#/engine").id).toBe("models");
    expect(parseRoute("#/settings").id).toBe("settings");
    expect(parseRoute("#/preferences").id).toBe("settings");
  });

  it("tolerates trailing slashes and query noise", () => {
    expect(parseRoute("#/chat/").id).toBe("chat");
    expect(parseRoute("#/projects?from=home").id).toBe("projects");
  });

  it("returns an honest not-found match for unknown paths", () => {
    const match = parseRoute("#/does-not-exist");
    expect(match.id).toBe("notFound");
    expect(match.path).toBe("/does-not-exist");
    expect(parseRoute("#/Chat").id).toBe("notFound"); // paths are exact, not fuzzy
  });

  it("marks only the onboarding flow routes as flow routes", () => {
    expect(isFlowRoute("welcome")).toBe(true);
    expect(isFlowRoute("onboarding")).toBe(true);
    expect(isFlowRoute("signin")).toBe(true);
    expect(isFlowRoute("signup")).toBe(true);
    expect(isFlowRoute("verify")).toBe(true);
    expect(isFlowRoute("recover")).toBe(true);
    expect(isFlowRoute("reset")).toBe(true);
    expect(isFlowRoute("home")).toBe(false);
    expect(isFlowRoute("chat")).toBe(false);
    expect(isFlowRoute("notFound")).toBe(false);
  });
});

describe("routeHref", () => {
  it("builds hash hrefs for every registered route", () => {
    for (const route of ROUTES) {
      expect(routeHref(route.id)).toBe(`#${route.path}`);
    }
    expect(routeHref("welcome")).toBe("#/welcome");
    expect(routeHref("onboarding")).toBe("#/onboarding");
    expect(routeHref("signin")).toBe("#/signin");
    expect(routeHref("signup")).toBe("#/signup");
    expect(routeHref("verify")).toBe("#/verify-email");
    expect(routeHref("recover")).toBe("#/recover-password");
    expect(routeHref("reset")).toBe("#/reset-password");
    expect(routeHref("home")).toBe("#/home");
    expect(routeHref("chat")).toBe("#/chat");
  });
});

describe("navigateTo", () => {
  it("changes the hash without leaving the document", () => {
    window.location.hash = "#/";
    navigateTo("projects");
    expect(window.location.hash).toBe("#/projects");
    expect(parseRoute(window.location.hash).id).toBe("projects");
  });

  it("is a no-op when already on the target route", () => {
    window.location.hash = "#/settings";
    let events = 0;
    const count = () => {
      events += 1;
    };
    window.addEventListener("hashchange", count);
    navigateTo("settings");
    window.removeEventListener("hashchange", count);
    expect(window.location.hash).toBe("#/settings");
    expect(events).toBe(0);
  });

  it("treats the root hash as the welcome route", () => {
    window.location.hash = "";
    let events = 0;
    const count = () => {
      events += 1;
    };
    window.addEventListener("hashchange", count);
    navigateTo("welcome");
    window.removeEventListener("hashchange", count);
    expect(events).toBe(0); // already there via the `#/` alias
    expect(parseRoute(window.location.hash).id).toBe("welcome");
  });
});
