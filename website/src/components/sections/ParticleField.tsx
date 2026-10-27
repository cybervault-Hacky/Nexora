import { useEffect, useRef } from "react";

import "./ParticleField.css";

/**
 * Homepage particle background (Phase 11).
 *
 * One `<canvas>` painting a small field of slow, low-opacity particles on a
 * transparent surface, so the page's own background shows through and the
 * effect stays a background detail rather than a panel.
 *
 * Why Canvas 2D rather than animated DOM nodes: the field needs ~20-130 moving
 * elements (about 100 on a 1440 x 1000 desktop), each with its own velocity,
 * opacity and radius, rewritten every frame. As DOM nodes that is 20-130
 * elements with a compositor layer each and a style recalculation per frame; as
 * one canvas it is a single element and a single paint. No 3D, no WebGL, no
 * particle library, no new dependency.
 *
 * Everything is driven from refs and module-scope maths. React never re-renders
 * during animation — no component state is touched by a frame, a pointer move,
 * or a resize.
 *
 * Behaviour:
 * - Particles drift continuously in a straight-ish line with per-particle speed
 *   and direction, and wrap around the edges, so the motion never starts or
 *   stops abruptly.
 * - On a mouse/pen pointer, particles inside a broad radius are gently
 *   accelerated toward the pointer with a quadratic distance falloff, then
 *   relaxed back to their own drift. The relaxation is what keeps the field
 *   distributed: particles sweep past the pointer instead of collapsing onto it
 *   or sticking to it.
 * - Touch pointers are ignored entirely: no hover interaction, no listeners
 *   that could interfere with scrolling or gestures. Coarse pointers simply get
 *   a lighter field.
 * - `prefers-reduced-motion: reduce` draws one static frame and never starts a
 *   loop; the field becomes a still texture rather than motion.
 * - The loop stops when the tab is hidden, when the component unmounts, and
 *   while reduced motion is requested. `start()` is idempotent, so React Strict
 *   Mode's double-invoked effect cannot create a second loop.
 */

/* ── Field shape ─────────────────────────────────────────────────────── */

/** One particle per this many CSS px², clamped to a sane count. */
const AREA_PER_PARTICLE = 9_000;
const MIN_PARTICLES = 18;
const MAX_PARTICLES = 130;
/** Coarse pointers get a lighter field: less work, quieter effect. */
const COARSE_FACTOR = 0.6;

/** Backing store is capped so a 3x display does not triple the fill cost. */
const MAX_DPR = 2;

/* ── Motion ──────────────────────────────────────────────────────────── */

/** Drift speed range, CSS px per second. */
const DRIFT_MIN = 5;
const DRIFT_MAX = 15;
/** Velocity is clamped so an accelerated particle can never run away. */
const MAX_SPEED = 90;
/** Rate (per second) at which velocity relaxes back to the particle's drift. */
const RELAX_PER_SEC = 1.4;
/** Acceleration toward the pointer at the centre of the radius, px/s². */
const ATTRACT_ACCEL = 120;
const RADIUS_MIN = 150;
const RADIUS_MAX = 260;
const RADIUS_RATIO = 0.18;

const RADIUS_MIN_PX = 2.0;
const RADIUS_MAX_PX = 5.2;

const TAU = Math.PI * 2;
/** Longest integration step, so a stalled frame cannot teleport particles. */
const MAX_STEP_SEC = 0.05;

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** The velocity this particle relaxes back toward. */
  driftX: number;
  driftY: number;
  r: number;
  /** Index into the pre-rendered sprite set. */
  colour: number;
  /** Applied via `globalAlpha`, so no string is built per frame. */
  alpha: number;
}

interface Rgb {
  r: number;
  g: number;
  b: number;
}

interface ColourSpec {
  token: string;
  /** Relative share of particles using this colour. */
  weight: number;
  alpha: readonly [number, number];
  fallback: Rgb;
}

/**
 * Restrained palette taken from the design tokens: the warm border tone is the
 * most visible, with cooler grey flecks and a soft brand-blue tint giving the
 * field two depths. No neon, no glow, nothing outside the existing palette.
 *
 * The alphas are bounded by contrast, not by taste. The hero's small labels use
 * `--color-text-muted`, which stands at exactly 5.04:1 on white and therefore
 * has almost no headroom: any darkening behind them has to stay above rgb(242)
 * to hold 4.5:1. Solving that per colour, and allowing for up to three particle
 * centres overlapping the same pixel (particles drift continuously, so overlaps
 * do happen), caps peak alpha at 0.079 / 0.031 / 0.026 respectively. The values
 * below sit under those ceilings. Presence comes from size and count instead of
 * darkness. The arithmetic is reproduced in the website README.
 */
const COLOUR_SPECS: readonly ColourSpec[] = [
  { token: "--color-border-strong", weight: 0.45, alpha: [0.045, 0.075], fallback: { r: 200, g: 195, b: 187 } },
  { token: "--color-text-muted", weight: 0.2, alpha: [0.02, 0.03], fallback: { r: 107, g: 111, b: 119 } },
  { token: "--color-accent", weight: 0.35, alpha: [0.016, 0.025], fallback: { r: 26, g: 86, b: 219 } },
];

function parseHexColour(value: string): Rgb | null {
  const hex = value.trim().replace(/^#/, "");
  if (hex.length === 3) {
    const expanded = hex
      .split("")
      .map((char) => char + char)
      .join("");
    return parseHexColour(`#${expanded}`);
  }
  if (!/^[0-9a-f]{6}$/i.test(hex)) return null;
  const int = Number.parseInt(hex, 16);
  return { r: (int >> 16) & 255, g: (int >> 8) & 255, b: int & 255 };
}

function resolveColours(): Rgb[] {
  let styles: CSSStyleDeclaration | null = null;
  try {
    styles = getComputedStyle(document.documentElement);
  } catch {
    styles = null;
  }
  return COLOUR_SPECS.map(
    (spec) =>
      (styles ? parseHexColour(styles.getPropertyValue(spec.token)) : null) ?? spec.fallback,
  );
}

function randomBetween(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

export default function ParticleField() {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const host = hostRef.current;
    const canvas = canvasRef.current;
    if (!host || !canvas) return;

    const ctx = canvas.getContext("2d");
    // No 2D context → no effect at all. The page is complete without it.
    if (!ctx) return;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const coarsePointer = window.matchMedia("(pointer: coarse)");

    const colours = resolveColours();
    const totals = COLOUR_SPECS.map((spec) => spec.weight);
    const colourCumulative: number[] = [];
    totals.reduce((sum, weight, index) => {
      const next = sum + weight;
      colourCumulative[index] = next;
      return next;
    }, 0);

    let width = 0;
    let height = 0;
    let dpr = 1;
    let radius = RADIUS_MAX;
    let particles: Particle[] = [];
    let rafId: number | null = null;
    let lastTime = 0;
    let disposed = false;
    let resizeScheduled = false;
    let rectDirty = true;
    let bounds = { left: 0, top: 0 };

    /** Pointer state in canvas coordinates; never causes a React update. */
    const pointer = { x: 0, y: 0, active: false };

    function pickColour(): { colour: number; alpha: number } {
      const roll = Math.random();
      const found = colourCumulative.findIndex((limit) => roll <= limit);
      const index = found < 0 ? COLOUR_SPECS.length - 1 : found;
      const spec = COLOUR_SPECS[index] ?? COLOUR_SPECS[COLOUR_SPECS.length - 1];
      const alpha = spec ? randomBetween(spec.alpha[0], spec.alpha[1]) : 0.12;
      return { colour: index, alpha };
    }

    /**
     * Pre-rendered soft particle sprites, one per palette colour, built once.
     *
     * Drawing a radial gradient per particle per frame would be far too
     * expensive, and hard-edged `arc()` dots at these very low alphas are
     * almost invisible. A 64px sprite scaled down per particle gives a soft
     * falloff, which reads as atmosphere at a peak alpha low enough to keep
     * small muted text above AA.
     */
    const SPRITE_SIZE = 64;
    const sprites = colours.map((rgb) => {
      const sprite = document.createElement("canvas");
      sprite.width = SPRITE_SIZE;
      sprite.height = SPRITE_SIZE;
      const spriteCtx = sprite.getContext("2d");
      if (spriteCtx) {
        const half = SPRITE_SIZE / 2;
        const gradient = spriteCtx.createRadialGradient(half, half, 0, half, half, half);
        gradient.addColorStop(0, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 1)`);
        gradient.addColorStop(0.35, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.7)`);
        gradient.addColorStop(0.7, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.25)`);
        gradient.addColorStop(1, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0)`);
        spriteCtx.fillStyle = gradient;
        spriteCtx.fillRect(0, 0, SPRITE_SIZE, SPRITE_SIZE);
      }
      return sprite;
    });

    function makeParticle(x: number, y: number): Particle {
      const angle = Math.random() * TAU;
      const speed = randomBetween(DRIFT_MIN, DRIFT_MAX);
      return {
        x,
        y,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        driftX: Math.cos(angle) * speed,
        driftY: Math.sin(angle) * speed,
        r: randomBetween(RADIUS_MIN_PX, RADIUS_MAX_PX),
        ...pickColour(),
      };
    }

    function targetCount(): number {
      const raw = Math.round((width * height) / AREA_PER_PARTICLE);
      const bounded = Math.min(MAX_PARTICLES, Math.max(MIN_PARTICLES, raw));
      return Math.max(MIN_PARTICLES, Math.round(bounded * (coarsePointer.matches ? COARSE_FACTOR : 1)));
    }

    /** Keeps existing particles and rescales them, so a resize does not reset the field. */
    function seed(previousWidth: number, previousHeight: number): void {
      const count = targetCount();
      if (particles.length === 0 || previousWidth === 0 || previousHeight === 0) {
        particles = [];
        for (let index = 0; index < count; index += 1) {
          particles.push(makeParticle(Math.random() * width, Math.random() * height));
        }
        return;
      }

      const scaleX = width / previousWidth;
      const scaleY = height / previousHeight;
      for (const particle of particles) {
        particle.x *= scaleX;
        particle.y *= scaleY;
      }
      while (particles.length > count) particles.pop();
      while (particles.length < count) {
        particles.push(makeParticle(Math.random() * width, Math.random() * height));
      }
    }

    function measureBounds(): void {
      const rect = host!.getBoundingClientRect();
      bounds = { left: rect.left, top: rect.top };
      rectDirty = false;
    }

    function draw(): void {
      ctx!.clearRect(0, 0, width, height);
      for (const particle of particles) {
        const sprite = sprites[particle.colour];
        if (!sprite) continue;
        ctx!.globalAlpha = particle.alpha;
        // Sprites are drawn at 3× the nominal radius: the visible core is a
        // fraction of the sprite, the rest is the soft falloff.
        const size = particle.r * 6;
        ctx!.drawImage(sprite, particle.x - size / 2, particle.y - size / 2, size, size);
      }
      ctx!.globalAlpha = 1;
    }

    function resize(): void {
      if (disposed) return;
      const nextWidth = host!.clientWidth;
      const nextHeight = host!.clientHeight;
      if (nextWidth === 0 || nextHeight === 0) return;

      const nextDpr = Math.min(window.devicePixelRatio || 1, MAX_DPR);

      // Scroll also asks for a resize (the canvas position changes), so bail out
      // unless the size really changed: reassigning canvas.width reallocates the
      // backing store and clears the frame, which would flicker while scrolling.
      const sizeChanged =
        nextWidth !== width || nextHeight !== height || nextDpr !== dpr;

      if (sizeChanged) {
        const previousWidth = width;
        const previousHeight = height;

        width = nextWidth;
        height = nextHeight;
        dpr = nextDpr;

        canvas!.width = Math.round(width * dpr);
        canvas!.height = Math.round(height * dpr);
        ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);

        radius = Math.min(RADIUS_MAX, Math.max(RADIUS_MIN, width * RADIUS_RATIO));
        seed(previousWidth, previousHeight);

        if (reducedMotion.matches) draw();
      }

      measureBounds();
    }

    function scheduleResize(): void {
      if (resizeScheduled || disposed) return;
      resizeScheduled = true;
      requestAnimationFrame(() => {
        resizeScheduled = false;
        if (disposed) return;
        resize();
      });
    }

    function step(now: number): void {
      const elapsed = lastTime === 0 ? 0.016 : (now - lastTime) / 1000;
      lastTime = now;
      // Clamped so returning to a backgrounded tab does not jump the field.
      const dt = Math.min(Math.max(elapsed, 0.001), MAX_STEP_SEC);
      const relax = 1 - Math.exp(-RELAX_PER_SEC * dt);
      const radiusSquared = radius * radius;

      for (const particle of particles) {
        if (pointer.active) {
          const dx = pointer.x - particle.x;
          const dy = pointer.y - particle.y;
          const distanceSquared = dx * dx + dy * dy;

          if (distanceSquared < radiusSquared && distanceSquared > 0) {
            const distance = Math.sqrt(distanceSquared);
            // Quadratic falloff: full strength at the pointer, nothing at the rim.
            const falloff = 1 - distance / radius;
            const accel = falloff * falloff * ATTRACT_ACCEL;
            particle.vx += (dx / distance) * accel * dt;
            particle.vy += (dy / distance) * accel * dt;
          }
        }

        // Relax toward the particle's own drift. This is what stops attraction
        // from accumulating into a cluster around the pointer.
        particle.vx += (particle.driftX - particle.vx) * relax;
        particle.vy += (particle.driftY - particle.vy) * relax;

        const speed = Math.hypot(particle.vx, particle.vy);
        if (speed > MAX_SPEED) {
          const scale = MAX_SPEED / speed;
          particle.vx *= scale;
          particle.vy *= scale;
        }

        particle.x += particle.vx * dt;
        particle.y += particle.vy * dt;

        // Wrap with a margin so particles re-enter smoothly instead of popping
        // into existence exactly on the edge.
        const margin = 8;
        if (particle.x < -margin) particle.x = width + margin;
        else if (particle.x > width + margin) particle.x = -margin;
        if (particle.y < -margin) particle.y = height + margin;
        else if (particle.y > height + margin) particle.y = -margin;
      }

      draw();
      rafId = requestAnimationFrame(step);
    }

    function start(): void {
      // Idempotent: Strict Mode's second effect run must not add a second loop.
      if (rafId !== null || disposed || document.hidden) return;
      if (reducedMotion.matches) {
        draw();
        return;
      }
      lastTime = 0;
      rafId = requestAnimationFrame(step);
    }

    function stop(): void {
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
    }

    /* ── Input ───────────────────────────────────────────────────────── */

    function onPointerMove(event: PointerEvent): void {
      // Touch has no hover, and listening for it would compete with scrolling.
      if (event.pointerType === "touch") return;
      if (rectDirty) measureBounds();

      const x = event.clientX - bounds.left;
      const y = event.clientY - bounds.top;

      // "Interacting with the homepage" includes the margin around the canvas,
      // so the effect does not snap off at the exact edge of the field.
      if (x < -radius || y < -radius || x > width + radius || y > height + radius) {
        pointer.active = false;
        return;
      }

      pointer.x = x;
      pointer.y = y;
      pointer.active = true;
      if (reducedMotion.matches) draw();
    }

    function onPointerOut(): void {
      pointer.active = false;
    }

    function onVisibilityChange(): void {
      if (document.hidden) {
        stop();
      } else {
        // Picks up a device-pixel-ratio or zoom change made while hidden.
        scheduleResize();
        start();
      }
    }

    function onReducedMotionChange(): void {
      stop();
      pointer.active = false;
      resize();
      start();
    }

    /**
     * Scrolling cannot change the canvas size — the layer is `inset: 0` inside a
     * block whose height comes from layout, and any real change to that arrives
     * through the `ResizeObserver`. Phase 15 measured the old behaviour at
     * 19 scroll events -> 19 rAF callbacks, 19 `getBoundingClientRect` calls and
     * 38 `clientWidth`/`clientHeight` reads, with zero backing-store
     * reallocations. So a scroll now only marks the cached viewport position
     * stale; `onPointerMove` re-measures it lazily, on the one event that
     * actually needs the value. A viewport resize still schedules a full
     * resize, because it can change the element's size and the device-pixel
     * ratio.
     */
    function onScroll(): void {
      rectDirty = true;
    }

    function onResize(): void {
      rectDirty = true;
      scheduleResize();
    }

    /* ── Lifecycle ───────────────────────────────────────────────────── */

    const resizeObserver = new ResizeObserver(scheduleResize);
    resizeObserver.observe(host);

    resize();
    start();

    window.addEventListener("pointermove", onPointerMove, { passive: true });
    window.addEventListener("blur", onPointerOut);
    window.addEventListener("resize", onResize, { passive: true });
    window.addEventListener("scroll", onScroll, { passive: true });
    document.documentElement.addEventListener("pointerleave", onPointerOut);
    document.addEventListener("visibilitychange", onVisibilityChange);
    reducedMotion.addEventListener("change", onReducedMotionChange);

    return () => {
      disposed = true;
      stop();
      resizeObserver.disconnect();
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("blur", onPointerOut);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("scroll", onScroll);
      document.documentElement.removeEventListener("pointerleave", onPointerOut);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      reducedMotion.removeEventListener("change", onReducedMotionChange);
      particles = [];
    };
  }, []);

  return (
    <div className="particle-layer" ref={hostRef} aria-hidden="true">
      <canvas className="particle-layer__canvas" ref={canvasRef} />
    </div>
  );
}
