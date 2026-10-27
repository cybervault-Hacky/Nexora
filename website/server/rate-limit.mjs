/**
 * Developer Portal — login rate limiting.
 *
 * An in-memory sliding window. Deliberately simple and deliberately *not* the
 * only control: it slows credential stuffing and online guessing, and the
 * response is a generic `429` with `Retry-After`. A distributed attacker with
 * many addresses is not stopped by this alone — that is what the (absent)
 * reverse-proxy layer would be for, and DEVELOPER_PORTAL.md says so.
 *
 * Keys are never logged. Callers pass an opaque key (an IP, or
 * `account:<normalised email>`) and receive a decision.
 */

/** @typedef {{ limit: number, windowMs: number }} Rule */

/**
 * @param {number} limit attempts allowed per window
 * @param {number} windowSeconds window length
 * @returns {Rule}
 */
export function rule(limit, windowSeconds) {
  return { limit, windowMs: windowSeconds * 1000 };
}

export class RateLimiter {
  /** @type {Map<string, number[]>} */
  #hits = new Map();
  /** @type {Map<string, Rule>} */
  #rules;
  /** @type {number} */
  #sweepEveryMs;
  #lastSweep = 0;

  /** @param {Record<string, Rule>} rules */
  constructor(rules) {
    this.#rules = new Map(Object.entries(rules));
    this.#sweepEveryMs = 5 * 60 * 1000;
  }

  #bucket(key, name) {
    const id = `${name}:${key}`;
    let hits = this.#hits.get(id);
    if (!hits) {
      hits = [];
      this.#hits.set(id, hits);
    }
    return hits;
  }

  #sweep(now) {
    if (now - this.#lastSweep < this.#sweepEveryMs) return;
    this.#lastSweep = now;
    const longest = Math.max(...[...this.#rules.values()].map((r) => r.windowMs));
    for (const [id, hits] of this.#hits) {
      const kept = hits.filter((at) => now - at < longest);
      if (kept.length === 0) this.#hits.delete(id);
      else this.#hits.set(id, kept);
    }
  }

  /**
   * Checks every named rule for a key without recording a hit.
   *
   * @param {string} key
   * @param {string[]} ruleNames
   * @param {number} [now]
   * @returns {{ allowed: boolean, retryAfterSeconds: number, blockedBy: string | null }}
   */
  check(key, ruleNames, now = Date.now()) {
    this.#sweep(now);
    let retryAfterSeconds = 0;
    let blockedBy = null;
    for (const name of ruleNames) {
      const current = this.#rules.get(name);
      if (!current) continue;
      const hits = this.#bucket(key, name).filter((at) => now - at < current.windowMs);
      this.#hits.set(`${name}:${key}`, hits);
      if (hits.length >= current.limit) {
        const oldest = hits[0] ?? now;
        const wait = Math.max(1, Math.ceil((current.windowMs - (now - oldest)) / 1000));
        retryAfterSeconds = Math.max(retryAfterSeconds, wait);
        blockedBy = name;
      }
    }
    return { allowed: blockedBy === null, retryAfterSeconds, blockedBy };
  }

  /** Records one attempt against every named rule. */
  record(key, ruleNames, now = Date.now()) {
    for (const name of ruleNames) {
      if (!this.#rules.has(name)) continue;
      const hits = this.#bucket(key, name);
      hits.push(now);
    }
  }

  /** Clears a key (used for the account key after a successful sign-in). */
  reset(key, ruleNames = null) {
    const names = ruleNames ?? [...this.#rules.keys()];
    for (const name of names) this.#hits.delete(`${name}:${key}`);
  }

  /** Test/observability helper: how many hits are recorded for a key+rule. */
  count(key, ruleName, now = Date.now()) {
    const current = this.#rules.get(ruleName);
    if (!current) return 0;
    return this.#bucket(key, ruleName).filter((at) => now - at < current.windowMs).length;
  }
}

/** Rules used by the sign-in endpoint. */
export const LOGIN_RULES = Object.freeze({
  /** Five failures per account per 15 minutes. */
  account: rule(5, 15 * 60),
  /** Ten failures per address per 15 minutes. */
  ip: rule(10, 15 * 60),
  /** A hard hourly ceiling per address, so a slow drip still stops. */
  ipHourly: rule(30, 60 * 60),
});

export function createLoginLimiter() {
  return new RateLimiter(LOGIN_RULES);
}
