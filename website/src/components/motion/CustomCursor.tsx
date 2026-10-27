import { useEffect, useRef } from "react";

import "./CustomCursor.css";

/**
 * Custom cursor (Phase 12).
 *
 * The glyph is an inline SVG outline of the approved reference: a hollow,
 * rounded cursor pointer with its tip at the top-left, drawn in the project's
 * navy (`--color-cursor`). The path was fitted to the reference image rather
 * than eyeballed — the three corner radii and the stroke width were solved
 * against the reference's measured contours, which is why the tip, right point
 * and bottom corner do not share one radius. The geometry is explained in the
 * website README.
 *
 * Why an overlay rather than `cursor: url(...)`:
 * - a CSS cursor image cannot animate, and this cursor needs a hover and
 *   pressed state;
 * - an overlay lets the state live in CSS custom properties while the position
 *   is written straight to `transform`, so no frame touches React state.
 *
 * The native cursor is hidden only where it would be the arrow or the hand.
 * Pointers that need a specialised cursor — text fields, editable content,
 * resizers, disabled controls — keep the native one, and this layer hides over
 * them, so the two can never appear at once. See `CustomCursor.css`.
 *
 * Gating: a fine pointer that can hover, and no reduced-motion request. On
 * touch, or when the user asks for reduced motion, nothing renders and the
 * native cursor is untouched.
 */

/**
 * The glyph, in the reference image's own coordinate space (its silhouette is
 * 67 x 68 units). Two subpaths: the outer contour and the inner contour, which
 * together describe the hollow band.
 */
const GLYPH_PATH =
  "M1.16 15.85 Q-6.51 -5.85 15.20 1.81 L64.61 19.25 Q71.06 21.53 66.42 26.55 " +
  "L29.83 66.14 Q21.96 74.66 18.09 63.72 L1.16 15.85 Z " +
  "M8.90 13.12 Q6.94 7.59 12.47 9.54 L55.72 24.81 Q56.58 25.12 55.96 25.78 " +
  "L25.84 58.38 Q25.16 59.11 24.83 58.18 L8.90 13.12 Z";

/** Fallback glyph size, used only until the rendered size can be measured. */
const GLYPH_PX = 26;

/**
 * Hotspot, as a fraction of the glyph box. The reference's tip is not exactly
 * at the top-left corner of its bounding box, and a cursor must point from the
 * tip — not from the corner — or it feels offset from where clicks land.
 */
const TIP_RATIO_X = 0.84 / 68;
const TIP_RATIO_Y = 1.49 / 68;

/** How much of the remaining distance is covered per frame (60fps ≈ 2 frames). */
const FOLLOW = 0.42;
/** Beyond this jump (px) the cursor is placed instead of flown, e.g. on entry. */
const SNAP_DISTANCE = 96;

/**
 * Elements that need their own native cursor. Kept in one place because the
 * CSS exemption list and the pointer test must agree.
 */
const NATIVE_CURSOR_SELECTOR = [
  "input",
  "textarea",
  "select",
  "option",
  "[contenteditable]:not([contenteditable='false'])",
  "[style*='resize']",
  "[data-native-cursor]",
  // A disabled control shows `not-allowed`; the decorative glyph steps aside so
  // the state cue is the only cursor on screen.
  ":disabled",
  "[aria-disabled='true']",
].join(",");

/** Anything the cursor should acknowledge as interactive. */
const INTERACTIVE_SELECTOR = [
  "a",
  "button",
  "summary",
  "label",
  "[role='button']",
  "[role='link']",
].join(",");

export default function CustomCursor() {
  const layerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const layer = layerRef.current;
    if (!layer) return;

    const finePointer = window.matchMedia("(hover: hover) and (pointer: fine)");
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

    // Capability gate. `supports` is checked once: without it the native cursor
    // would be hidden with nothing to replace it.
    if (!finePointer.matches || reducedMotion.matches) return;

    const root = document.documentElement;

    // The glyph's CSS size is the source of truth (it is set on `.cursor`), so
    // the hotspot is derived from the rendered box rather than a second
    // hard-coded number that could drift out of step with the stylesheet.
    const glyphEl = layer.querySelector<SVGElement>(".cursor__glyph");
    let tipX = GLYPH_PX * TIP_RATIO_X;
    let tipY = GLYPH_PX * TIP_RATIO_Y;

    function measureHotspot(): void {
      // Computed height is the layout box, so the hover/press transform scale
      // cannot move the hotspot.
      const size = glyphEl ? Number.parseFloat(getComputedStyle(glyphEl).height) : Number.NaN;
      const safe = Number.isFinite(size) && size > 0 ? size : GLYPH_PX;
      tipX = safe * TIP_RATIO_X;
      tipY = safe * TIP_RATIO_Y;
    }

    /* ── State, all in refs: no React render is caused by the pointer ──── */

    let targetX = 0;
    let targetY = 0;
    let currentX = 0;
    let currentY = 0;
    let rafId: number | null = null;
    let placed = false;
    let visible = false;
    let disposed = false;
    /** Last element the hover/overlay state was computed for. */
    let lastTarget: Element | null = null;
    let hovered = false;
    let overNative = false;

    function showLayer(): void {
      if (visible) return;
      visible = true;
      layer!.dataset.visible = "true";
    }

    function hideLayer(): void {
      if (!visible) return;
      visible = false;
      layer!.dataset.visible = "false";
    }

    function setState(next: "idle" | "hover" | "press"): void {
      layer!.dataset.state = next;
    }

    /**
     * Work out, once per element the pointer enters, whether the custom cursor
     * belongs over it. Reading `closest` here (rather than on every move) keeps
     * the move handler to two number writes.
     */
    function evaluateTarget(element: Element | null): void {
      if (element === lastTarget) return;
      lastTarget = element;

      const nativeEl = element?.closest(NATIVE_CURSOR_SELECTOR) ?? null;
      overNative = nativeEl !== null;

      if (overNative) {
        // A text field or a resizer: step aside and let the native cursor
        // through. The class is removed so nothing is suppressed behind it.
        root.classList.remove("has-nexora-cursor");
        hideLayer();
        hovered = false;
        return;
      }

      root.classList.add("has-nexora-cursor");

      const interactive = element?.closest(INTERACTIVE_SELECTOR) ?? null;
      hovered = interactive !== null;
      setState(hovered ? "hover" : "idle");

      // Reveal only once a real move has positioned the glyph. A `pointerover`
      // with no preceding move (Chromium fires one at 0,0 when the page first
      // receives pointer input) must not paint the cursor in the top-left
      // corner; onPointerMove calls this again after positioning.
      if (placed) showLayer();
    }

    /* ── Animation ─────────────────────────────────────────────────────── */

    function frame(): void {
      rafId = null;
      if (disposed) return;

      const dx = targetX - currentX;
      const dy = targetY - currentY;

      // Settle, then stop the loop. A permanently running rAF for a cursor
      // that is not moving is wasted work.
      if (Math.abs(dx) < 0.1 && Math.abs(dy) < 0.1) {
        currentX = targetX;
        currentY = targetY;
        layer!.style.transform = `translate3d(${currentX}px, ${currentY}px, 0)`;
        return;
      }

      currentX += dx * FOLLOW;
      currentY += dy * FOLLOW;
      layer!.style.transform = `translate3d(${currentX}px, ${currentY}px, 0)`;
      rafId = requestAnimationFrame(frame);
    }

    function ensureRunning(): void {
      if (rafId === null && !disposed) rafId = requestAnimationFrame(frame);
    }

    /* ── Input ─────────────────────────────────────────────────────────── */

    function onPointerMove(event: PointerEvent): void {
      // Touch and pen-with-hover ambiguity: only a real fine pointer drives this.
      if (event.pointerType === "touch") return;

      const highlight = (event.target as Element | null) ?? null;
      evaluateTarget(highlight);
      if (overNative) return;

      targetX = event.clientX - tipX;
      targetY = event.clientY - tipY;

      if (!placed) {
        placed = true;
        currentX = targetX;
        currentY = targetY;
        layer!.style.transform = `translate3d(${currentX}px, ${currentY}px, 0)`;
      } else if (Math.hypot(targetX - currentX, targetY - currentY) > SNAP_DISTANCE) {
        currentX = targetX;
        currentY = targetY;
      }

      // Positioned, so the glyph can appear.
      showLayer();
      ensureRunning();
    }

    function onPointerOver(event: PointerEvent): void {
      if (event.pointerType === "touch") return;
      evaluateTarget((event.target as Element | null) ?? null);
    }

    function onPointerDown(event: PointerEvent): void {
      if (event.pointerType === "touch" || overNative) return;
      setState("press");
    }

    function onPointerUp(): void {
      if (overNative) return;
      setState(hovered ? "hover" : "idle");
    }

    /** Pointer gone (left the window, or the window lost focus). */
    function onPointerGone(): void {
      if (disposed) return;
      hideLayer();
      // Do not leave the native cursor suppressed while the pointer is away.
      root.classList.remove("has-nexora-cursor");
      lastTarget = null;
      placed = false;
    }

    function onBlur(): void {
      onPointerGone();
    }

    function onReducedMotionChange(): void {
      // Hand the native cursor straight back.
      stop();
      root.classList.remove("has-nexora-cursor");
      hideLayer();
    }

    function stop(): void {
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
    }

    /* ── Lifecycle ─────────────────────────────────────────────────────── */

    measureHotspot();

    window.addEventListener("pointermove", onPointerMove, { passive: true });
    window.addEventListener("pointerover", onPointerOver, { passive: true });
    window.addEventListener("pointerdown", onPointerDown, { passive: true });
    window.addEventListener("pointerup", onPointerUp, { passive: true });
    window.addEventListener("pointercancel", onPointerGone, { passive: true });
    window.addEventListener("blur", onBlur);
    window.addEventListener("resize", measureHotspot, { passive: true });
    document.addEventListener("pointerleave", onPointerGone);
    document.documentElement.addEventListener("pointerleave", onPointerGone);
    reducedMotion.addEventListener("change", onReducedMotionChange);

    return () => {
      disposed = true;
      stop();
      root.classList.remove("has-nexora-cursor");
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerover", onPointerOver);
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerGone);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("resize", measureHotspot);
      document.removeEventListener("pointerleave", onPointerGone);
      document.documentElement.removeEventListener("pointerleave", onPointerGone);
      reducedMotion.removeEventListener("change", onReducedMotionChange);
    };
  }, []);

  return (
    <div
      className="cursor"
      ref={layerRef}
      aria-hidden="true"
      data-visible="false"
      data-state="idle"
    >
      <svg
        className="cursor__glyph"
        viewBox="0 0 68 68"
        width={GLYPH_PX}
        height={GLYPH_PX}
        aria-hidden="true"
        focusable="false"
      >
        {/* The reference is hollow, so the glyph is outline only.
         *
         * That alone disappears over the black display headings: a dark navy
         * stroke on black type leaves almost nothing to see. The first path is
         * therefore a keyline in the page background colour, stroked along both
         * contours of the band. On the site's white and warm-white surfaces it
         * is invisible — the glyph looks exactly like the reference — and over
         * text it opens a 1px gap that keeps the pointer legible. It is a thin
         * keyline, not a glow or a drop shadow. */}
        <path className="cursor__keyline" d={GLYPH_PATH} />
        <path className="cursor__outline" fillRule="evenodd" d={GLYPH_PATH} />

      </svg>
    </div>
  );
}
