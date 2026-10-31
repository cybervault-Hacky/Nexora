import { useEffect, useRef } from "react";

import "./ParticleField.css";

/**
 * Homepage particle background — Antigravity-inspired spatial field.
 *
 * One `<canvas>` painting a small field of soft particles on a transparent
 * surface, so the page's own background shows through and the effect stays a
 * background detail rather than a panel. The field reads as a shallow volume
 * of floating motes: particles drift along gently curving paths, and the
 * pointer acts as a small gravity well — nearby motes swirl around it and are
 * pushed away when they get too close — while a faint ambient light follows
 * the pointer and a depth gradient carries parallax. The visual language is
 * an original Nexora take on spatial particle interaction; nothing is copied
 * from any other product.
 *
 * Why Canvas 2D rather than animated DOM nodes: the field needs ~20-130 moving
 * elements (about 100 on a 1440 x 1000 desktop), each with its own velocity,
 * depth, opacity and soft radial falloff, rewritten every frame. As DOM nodes
 * that is 20-130 elements with a compositor layer each and a style recalculation
 * per frame; as one canvas it is a single element and a single paint. No 3D, no
 * WebGL, no particle library, no new dependency.
 *
 * Everything is driven from refs and module-scope maths. React never re-renders
 * during animation — no component state is touched by a frame, a pointer move,
 * or a resize.
 *
 * Behaviour:
 * - Particles drift along curved paths (the drift direction rotates slowly per
 *   particle) and wrap around the edges, so the motion never starts or stops
 *   abruptly.
 * - The pointer is a gravity well. Inside a broad radius, motes in a close zone
 *   are repelled (pushed away), and motes in the outer band are drawn into a
 *   tangential swirl with a weak inward pull. Every mote's velocity relaxes
 *   back toward its own drift, so the field sweeps past the pointer instead of
 *   collapsing onto it or sticking.
 * - Depth: each mote has a depth in [0, 1]. Near motes are larger, drift a
 *   little faster and react more strongly to the pointer; a subtle parallax
 *   offset (near motes shift more with the pointer) sells the volume.
 * - Ambient light: a soft, restrained glow follows the pointer and gently
 *   illuminates the motes closest to it. It fades out when the pointer leaves.
 * - Touch: the field is lighter on coarse pointers and drifts organically. A
 *   quick tap emits a gentle outward ripple that relaxes away — touch has no
 *   hover, and a drag-tracked attraction would compete with scrolling, so the
 *   interaction is confined to a non-interfering tap.
 * - `prefers-reduced-motion: reduce` draws one static frame and never starts a
 *   loop; the field becomes a still texture rather than motion.
 * - The loop stops when the tab is hidden, when the component unmounts, and
 *   while reduced motion is requested. `start()` is idempotent, so React
 *   Strict Mode's double-invoked effect cannot create a second loop.
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
/** How fast each particle's drift direction rotates (rad/s). A slow rotation
 *  is what turns straight-line drift into a gently curving, organic path. */
const DRIFT_ROT_MIN = -0.2;
const DRIFT_ROT_MAX = 0.2;
/** Velocity is clamped so a repelled or swirled particle can never run away. */
const MAX_SPEED = 90;
/** Rate (per second) at which velocity relaxes back to the particle's drift. */
const RELAX_PER_SEC = 1.4;
/** Longest integration step, so a stalled frame cannot teleport particles. */
const MAX_STEP_SEC = 0.05;

/* ── Pointer gravity well ────────────────────────────────────────────── */

/** Interaction radius: 18% of the field width, clamped to 150-260 px. */
const RADIUS_MIN = 150;
const RADIUS_MAX = 260;
const RADIUS_RATIO = 0.18;
/** Fraction of the radius that is the close (repel) zone. */
const REPEL_RATIO = 0.42;
/** Weak inward pull in the swirl band, px/s² at full falloff. */
const ATTRACT_ACCEL = 40;
/** Tangential force that orbits the particle around the pointer, px/s². */
const TANGENTIAL_ACCEL = 80;
/** Outward push in the close zone, px/s² at full falloff. */
const REPEL_ACCEL = 190;

/* ── Parallax / depth ────────────────────────────────────────────────── */

/** Max render offset (px) at depth 1; near motes shift more than far ones. */
const PARALLAX_MAX = 7;
const PARALLAX_EASE = 2.0;

/* ── Ambient light ───────────────────────────────────────────────────── */

/** Ambient light radius: 30% of the field width, clamped. */
const LIGHT_RADIUS_MIN = 220;
const LIGHT_RADIUS_MAX = 440;
const LIGHT_RADIUS_RATIO = 0.3;
/** Global alpha the light sprite is drawn at; its own gradient is softer. */
const LIGHT_ALPHA = 0.05;
const LIGHT_EASE = 2.5;
/** Max alpha / size multiplier for a mote sitting at the light's centre. */
const ILLUMINATION_ALPHA = 0.3;
const ILLUMINATION_SIZE = 0.2;

/* ── Touch ripple ────────────────────────────────────────────────────── */

/** Outward impulse scale for a tap ripple (a fraction of REPEL_ACCEL). */
const RIPPLE_FORCE = 0.45;
const RIPPLE_DECAY = 2.2;
/** A tap is a quick press with little movement; drags and scrolls are ignored. */
const TAP_MAX_MS = 320;
const TAP_MAX_MOVE = 12;

/* ── Visuals ─────────────────────────────────────────────────────────── */

const RADIUS_MIN_PX = 2.0;
const RADIUS_MAX_PX = 5.2;

const TAU = Math.PI * 2;

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** Current drift direction (rad); rotates slowly for organic paths. */
  driftAngle: number;
  driftSpeed: number;
  driftRot: number;
  /** 0 = far, 1 = near. Drives size, speed, force weight and parallax. */
  depth: number;
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
 * do happen), caps peak alpha at 0.219 / 0.076 / 0.090 respectively. The values
 * below sit under those ceilings. Presence comes from size and count instead of
 * darkness. The arithmetic is reproduced in the website README.
 *
 * The illumination multiplier applied near the pointer stays modest so a mote
 * at the light's centre never exceeds these bounds by more than a whisper.
 */
const COLOUR_SPECS: readonly ColourSpec[] = [
  { token: "--color-border-strong", weight: 0.45, alpha: [0.045, 0.075], fallback: { r: 200, g: 195, b: 187 } },
  { token: "--color-text-muted", weight: 0.2, alpha: [0.02, 0.03], fallback: { r: 107, g: 111, b: 119 } },
  { token: "--color-accent", weight: 0.35, alpha: [0.016, 0.025], fallback: { r: 26, g: 86, b: 219 } },
];

/** Fallback for the ambient light, matching the accent token. */
const LIGHT_FALLBACK: Rgb = { r: 26, g: 86, b: 219 };

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

function resolveLightColour(): Rgb {
  try {
    const value = getComputedStyle(document.documentElement).getPropertyValue("--color-accent");
    return parseHexColour(value) ?? LIGHT_FALLBACK;
  } catch {
    return LIGHT_FALLBACK;
  }
}

function randomBetween(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

function lerp(min: number, max: number, t: number): number {
  return min + (max - min) * t;
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
    const lightColour = resolveLightColour();
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
    let repelRadius = 0;
    let swirlSpan = 1;
    let rippleRadius = 0;
    let rippleRadiusSquared = 0;
    let lightRadius = LIGHT_RADIUS_MAX;
    let particles: Particle[] = [];
    let rafId: number | null = null;
    let lastTime = 0;
    let disposed = false;
    let resizeScheduled = false;
    let rectDirty = true;
    let bounds = { left: 0, top: 0 };

    /** Pointer state in canvas coordinates; never causes a React update. */
    const pointer = { x: 0, y: 0, active: false };
    /** Parallax target (px, depth-1 scale) and the eased current value. */
    let parallaxTargetX = 0;
    let parallaxTargetY = 0;
    let parallaxCurX = 0;
    let parallaxCurY = 0;
    /** Ambient light intensity, eased toward 1 while the pointer is present. */
    let lightAlpha = 0;

    /** A touch tap ripple: a gentle outward pulse that relaxes away. */
    interface Ripple {
      x: number;
      y: number;
      strength: number;
    }
    const ripples: Ripple[] = [];
    /** The in-flight touch session, used to tell a tap from a drag. */
    let touchSession: {
      x: number;
      y: number;
      startTime: number;
      moved: boolean;
      pointerId: number;
    } | null = null;

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

    /**
     * Pre-rendered ambient light: a large, soft radial glow in the accent tint.
     * Drawn at a low global alpha so it stays a restrained haze rather than a
     * colour wash, and never reads as neon.
     */
    const LIGHT_SIZE = 256;
    const lightSprite = (() => {
      const sprite = document.createElement("canvas");
      sprite.width = LIGHT_SIZE;
      sprite.height = LIGHT_SIZE;
      const spriteCtx = sprite.getContext("2d");
      if (spriteCtx) {
        const half = LIGHT_SIZE / 2;
        const gradient = spriteCtx.createRadialGradient(half, half, 0, half, half, half);
        gradient.addColorStop(0, `rgba(${lightColour.r}, ${lightColour.g}, ${lightColour.b}, 0.9)`);
        gradient.addColorStop(0.4, `rgba(${lightColour.r}, ${lightColour.g}, ${lightColour.b}, 0.35)`);
        gradient.addColorStop(1, `rgba(${lightColour.r}, ${lightColour.g}, ${lightColour.b}, 0)`);
        spriteCtx.fillStyle = gradient;
        spriteCtx.fillRect(0, 0, LIGHT_SIZE, LIGHT_SIZE);
      }
      return sprite;
    })();

    function makeParticle(x: number, y: number): Particle {
      // Slightly more distant motes than near ones: near motes are bigger and
      // brighter, so a smaller number of them carries the foreground.
      const depth = lerp(0.18, 1, Math.pow(Math.random(), 1.5));
      const angle = Math.random() * TAU;
      const speed = lerp(DRIFT_MIN, DRIFT_MAX, depth);
      const { colour, alpha } = pickColour();
      return {
        x,
        y,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        driftAngle: angle,
        driftSpeed: speed,
        driftRot: randomBetween(DRIFT_ROT_MIN, DRIFT_ROT_MAX),
        depth,
        r: clamp(
          lerp(RADIUS_MIN_PX, RADIUS_MAX_PX, depth) * randomBetween(0.85, 1.15),
          RADIUS_MIN_PX,
          RADIUS_MAX_PX,
        ),
        colour,
        alpha,
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

      // Ambient light first, so the motes sit on top of it. Fades out when the
      // pointer is gone, so the field returns to its natural, unlit state.
      if (lightAlpha > 0.004) {
        const size = lightRadius * 2;
        ctx!.globalAlpha = lightAlpha * LIGHT_ALPHA;
        ctx!.drawImage(lightSprite, pointer.x - size / 2, pointer.y - size / 2, size, size);
        ctx!.globalAlpha = 1;
      }

      const lightOn = lightAlpha > 0.004;
      const lightRadiusSquared = lightRadius * lightRadius;

      for (const particle of particles) {
        const sprite = sprites[particle.colour];
        if (!sprite) continue;

        let size = particle.r * 6;
        let alpha = particle.alpha;

        // Illuminate the motes closest to the light: a touch more presence
        // near the pointer reads as ambient glow without changing the palette.
        if (lightOn) {
          const dx = pointer.x - particle.x;
          const dy = pointer.y - particle.y;
          const distanceSquared = dx * dx + dy * dy;
          if (distanceSquared < lightRadiusSquared) {
            const falloff = 1 - Math.sqrt(distanceSquared) / lightRadius;
            size *= 1 + ILLUMINATION_SIZE * falloff * lightAlpha;
            alpha *= 1 + ILLUMINATION_ALPHA * falloff * lightAlpha;
          }
        }

        // Parallax: near motes (high depth) shift more with the pointer, which
        // is what sells the volume. The offset is tiny and eased, and eases
        // back to zero when the pointer leaves.
        const renderX = particle.x + parallaxCurX * particle.depth;
        const renderY = particle.y + parallaxCurY * particle.depth;

        ctx!.globalAlpha = alpha > 1 ? 1 : alpha;
        // Sprites are drawn at 3× the nominal radius: the visible core is a
        // fraction of the sprite, the rest is the soft falloff.
        ctx!.drawImage(sprite, renderX - size / 2, renderY - size / 2, size, size);
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

        radius = clamp(width * RADIUS_RATIO, RADIUS_MIN, RADIUS_MAX);
        repelRadius = radius * REPEL_RATIO;
        swirlSpan = Math.max(1, radius - repelRadius);
        rippleRadius = radius * 1.15;
        rippleRadiusSquared = rippleRadius * rippleRadius;
        lightRadius = clamp(width * LIGHT_RADIUS_RATIO, LIGHT_RADIUS_MIN, LIGHT_RADIUS_MAX);
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
      const parallaxEase = 1 - Math.exp(-PARALLAX_EASE * dt);
      const lightEase = 1 - Math.exp(-LIGHT_EASE * dt);

      // Ease the parallax offset and ambient light toward their targets, and
      // the light fades to nothing when the pointer is gone.
      parallaxCurX += (parallaxTargetX - parallaxCurX) * parallaxEase;
      parallaxCurY += (parallaxTargetY - parallaxCurY) * parallaxEase;
      lightAlpha += ((pointer.active ? 1 : 0) - lightAlpha) * lightEase;

      // Decay tap ripples (touch).
      for (let index = ripples.length - 1; index >= 0; index -= 1) {
        const ripple = ripples[index];
        if (!ripple) continue;
        ripple.strength *= Math.exp(-RIPPLE_DECAY * dt);
        if (ripple.strength < 0.04) ripples.splice(index, 1);
      }

      const radiusSquared = radius * radius;
      const repelRadiusSquared = repelRadius * repelRadius;

      for (const particle of particles) {
        // Organic drift: the drift direction rotates slowly, so paths curve
        // instead of running in straight lines.
        particle.driftAngle += particle.driftRot * dt;
        const driftVx = Math.cos(particle.driftAngle) * particle.driftSpeed;
        const driftVy = Math.sin(particle.driftAngle) * particle.driftSpeed;

        if (pointer.active) {
          const dx = pointer.x - particle.x;
          const dy = pointer.y - particle.y;
          const distanceSquared = dx * dx + dy * dy;

          if (distanceSquared < radiusSquared && distanceSquared > 0) {
            const distance = Math.sqrt(distanceSquared);
            const ux = dx / distance;
            const uy = dy / distance;
            // Near particles react more to the pointer; that gradient is itself
            // a depth cue.
            const force = 0.45 + 0.55 * particle.depth;

            if (distanceSquared < repelRadiusSquared) {
              // Close zone: push away from the pointer — the "antigravity"
              // repel. Quadratic falloff, nothing at the repel rim.
              const falloff = 1 - distance / repelRadius;
              const accel = REPEL_ACCEL * falloff * falloff * force;
              particle.vx -= ux * accel * dt;
              particle.vy -= uy * accel * dt;
            } else {
              // Swirl zone: a tangential force orbits the particle around the
              // pointer, with a weak inward pull. The relax term below stops it
              // from collapsing, so motes sweep past rather than sticking.
              const falloff = 1 - (distance - repelRadius) / swirlSpan;
              const f = falloff * falloff;
              const tangential = TANGENTIAL_ACCEL * f * force;
              particle.vx += (-uy) * tangential * dt;
              particle.vy += (ux) * tangential * dt;
              const inward = ATTRACT_ACCEL * f * force;
              particle.vx += ux * inward * dt;
              particle.vy += uy * inward * dt;
            }
          }
        }

        // Tap ripples (touch): a gentle outward nudge that relaxes away.
        for (const ripple of ripples) {
          const dx = particle.x - ripple.x;
          const dy = particle.y - ripple.y;
          const distanceSquared = dx * dx + dy * dy;
          if (distanceSquared < rippleRadiusSquared && distanceSquared > 0) {
            const distance = Math.sqrt(distanceSquared);
            const falloff = (1 - distance / rippleRadius) * ripple.strength;
            const accel = REPEL_ACCEL * RIPPLE_FORCE * falloff;
            particle.vx += (dx / distance) * accel * dt;
            particle.vy += (dy / distance) * accel * dt;
          }
        }

        // Relax toward the particle's own (rotating) drift. This is what keeps
        // the field distributed: cursor forces are temporary and the field
        // resumes its natural motion once the pointer moves on.
        particle.vx += (driftVx - particle.vx) * relax;
        particle.vy += (driftVy - particle.vy) * relax;

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

    /** Pointer gone: drop the interaction and let everything ease back. */
    function setPointerInactive(): void {
      pointer.active = false;
      parallaxTargetX = 0;
      parallaxTargetY = 0;
    }

    /* ── Input ───────────────────────────────────────────────────────── */

    function onPointerMove(event: PointerEvent): void {
      if (event.pointerType === "touch") {
        // Track whether this touch is a tap (little movement) for the ripple.
        // Touch has no hover, so it never drives the gravity well — that would
        // compete with scrolling.
        if (touchSession && event.pointerId === touchSession.pointerId) {
          if (rectDirty) measureBounds();
          const x = event.clientX - bounds.left;
          const y = event.clientY - bounds.top;
          if (Math.hypot(x - touchSession.x, y - touchSession.y) > TAP_MAX_MOVE) {
            touchSession.moved = true;
          }
        }
        return;
      }

      if (rectDirty) measureBounds();

      const x = event.clientX - bounds.left;
      const y = event.clientY - bounds.top;

      // "Interacting with the homepage" includes the margin around the canvas,
      // so the effect does not snap off at the exact edge of the field.
      if (x < -radius || y < -radius || x > width + radius || y > height + radius) {
        setPointerInactive();
        return;
      }

      pointer.x = x;
      pointer.y = y;
      pointer.active = true;

      // Parallax target: how far the pointer sits from the field centre.
      const ox = (pointer.x - width / 2) / (width / 2);
      const oy = (pointer.y - height / 2) / (height / 2);
      parallaxTargetX = Math.max(-1, Math.min(1, ox)) * PARALLAX_MAX;
      parallaxTargetY = Math.max(-1, Math.min(1, oy)) * PARALLAX_MAX;

      if (reducedMotion.matches) draw();
    }

    function onPointerDown(event: PointerEvent): void {
      if (event.pointerType !== "touch") return;
      if (rectDirty) measureBounds();
      touchSession = {
        x: event.clientX - bounds.left,
        y: event.clientY - bounds.top,
        startTime: performance.now(),
        moved: false,
        pointerId: event.pointerId,
      };
    }

    function onPointerUp(event: PointerEvent): void {
      if (event.pointerType !== "touch") return;
      const session = touchSession;
      if (!session || session.pointerId !== event.pointerId) return;
      touchSession = null;
      // A drag or a long press is a scroll/gesture, not a tap: ignore it.
      if (session.moved || performance.now() - session.startTime > TAP_MAX_MS) return;
      if (ripples.length >= 3) ripples.shift();
      ripples.push({ x: session.x, y: session.y, strength: 1 });
    }

    function onPointerCancel(event: PointerEvent): void {
      if (event.pointerType !== "touch") return;
      if (touchSession && event.pointerId === touchSession.pointerId) {
        touchSession = null;
      }
    }

    function onPointerOut(): void {
      setPointerInactive();
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
      setPointerInactive();
      touchSession = null;
      ripples.length = 0;
      parallaxCurX = 0;
      parallaxCurY = 0;
      lightAlpha = 0;
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
    window.addEventListener("pointerdown", onPointerDown, { passive: true });
    window.addEventListener("pointerup", onPointerUp, { passive: true });
    window.addEventListener("pointercancel", onPointerCancel, { passive: true });
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
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerCancel);
      window.removeEventListener("blur", onPointerOut);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("scroll", onScroll);
      document.removeEventListener("pointerleave", onPointerOut);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      reducedMotion.removeEventListener("change", onReducedMotionChange);
      particles = [];
      ripples.length = 0;
    };
  }, []);

  return (
    <div className="particle-layer" ref={hostRef} aria-hidden="true">
      <canvas className="particle-layer__canvas" ref={canvasRef} />
    </div>
  );
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
