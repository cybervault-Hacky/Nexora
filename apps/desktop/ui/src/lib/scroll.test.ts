import { describe, expect, it } from "vitest";
import { isNearBottom, SCROLL_BOTTOM_THRESHOLD } from "./scroll";

describe("isNearBottom", () => {
  it("treats a short conversation as already at the bottom", () => {
    expect(
      isNearBottom({ scrollTop: 0, scrollHeight: 400, clientHeight: 600 }),
    ).toBe(true);
  });

  it("is true when scrolled to the exact bottom", () => {
    expect(
      isNearBottom({ scrollTop: 400, scrollHeight: 1_200, clientHeight: 800 }),
    ).toBe(true);
  });

  it("tolerates a small distance below the threshold", () => {
    expect(
      isNearBottom({
        scrollTop: 400 - SCROLL_BOTTOM_THRESHOLD,
        scrollHeight: 1_200,
        clientHeight: 800,
      }),
    ).toBe(true);
  });

  it("is false when the reader has scrolled up", () => {
    expect(
      isNearBottom({
        scrollTop: 100,
        scrollHeight: 1_200,
        clientHeight: 800,
      }),
    ).toBe(false);
  });

  it("honours a custom threshold", () => {
    expect(
      isNearBottom(
        { scrollTop: 350, scrollHeight: 1_200, clientHeight: 800 },
        20,
      ),
    ).toBe(false);
  });
});
