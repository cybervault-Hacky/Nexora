/**
 * Developer Portal — persistence.
 *
 * A single JSON document (`portal.json`) in a data directory that is **not**
 * inside the web root. Three properties matter more than speed here:
 *
 * 1. **Nothing sensitive is stored in a recoverable form.** Passwords are scrypt
 *    hashes, session tokens are sha256 digests, and the raw token only ever
 *    exists in the browser's cookie. The file is written with mode `0600`; its
 *    directory with `0700`.
 * 2. **Writes are atomic.** The document is written to a temporary file in the
 *    same directory and then renamed over the original, so a crash mid-write
 *    cannot leave a truncated file that would read as "no owner exists".
 * 3. **A corrupt store is a hard error, not an empty store.** If the JSON cannot
 *    be parsed the store fails to open and the API answers
 *    `503 portal_unavailable`; it never falls back to a fresh database, because
 *    that would silently un-provision the owner account.
 *
 * This is deliberately a single-process store. It is correct for one portal
 * process and is not a substitute for PostgreSQL — see DEVELOPER_PORTAL.md §11
 * for the migration path to `services/nexora-auth`.
 */

import { randomUUID } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";

const STORE_VERSION = 1;
const FILE_NAME = "portal.json";

/** Sessions kept in the file for at most this long after they die. */
const SESSION_RETENTION_MS = 7 * 24 * 60 * 60 * 1000;
/** Audit entries kept on disk. Bounded so the file cannot grow forever. */
const AUDIT_LIMIT = 200;

const EMPTY_STATE = Object.freeze({
  version: STORE_VERSION,
  users: [],
  sessions: [],
  audit: [],
});

export class StoreUnavailableError extends Error {
  constructor(message) {
    super(message);
    this.name = "StoreUnavailableError";
  }
}

function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Structural validation of the document. Throws `StoreUnavailableError`. */
function validateState(raw) {
  if (!isPlainObject(raw)) throw new StoreUnavailableError("store: not a JSON object");
  if (raw.version !== STORE_VERSION) throw new StoreUnavailableError("store: unsupported version");
  for (const key of ["users", "sessions", "audit"]) {
    if (!Array.isArray(raw[key])) throw new StoreUnavailableError(`store: "${key}" is not an array`);
  }
  for (const user of raw.users) {
    if (!isPlainObject(user)) throw new StoreUnavailableError("store: malformed user record");
    for (const field of ["id", "email", "role", "passwordHash", "createdAt"]) {
      if (typeof user[field] !== "string" || user[field] === "") {
        throw new StoreUnavailableError(`store: user is missing "${field}"`);
      }
    }
  }
  for (const session of raw.sessions) {
    if (!isPlainObject(session)) throw new StoreUnavailableError("store: malformed session record");
    for (const field of ["id", "userId", "tokenHash", "csrfToken", "createdAt", "expiresAt"]) {
      if (typeof session[field] !== "string" || session[field] === "") {
        throw new StoreUnavailableError(`store: session is missing "${field}"`);
      }
    }
  }
  return raw;
}

export class Store {
  /** @type {string} */
  #dir;
  /** @type {string} */
  #file;
  #state = { ...EMPTY_STATE };
  #writeChain = Promise.resolve();
  /** @type {boolean} */
  #provisioned = false;

  constructor(dir) {
    this.#dir = dir;
    this.#file = path.join(dir, FILE_NAME);
  }

  get filePath() {
    return this.#file;
  }

  /** True when the owner account exists. */
  get provisioned() {
    return this.#provisioned;
  }

  /**
   * Loads the document, creating the directory if needed. An absent file is a
   * valid empty store (nobody provisioned yet); an unreadable or corrupt one is
   * `StoreUnavailableError`.
   */
  async open() {
    try {
      await fs.mkdir(this.#dir, { recursive: true, mode: 0o700 });
    } catch (error) {
      throw new StoreUnavailableError(`store: cannot create data directory (${error?.code ?? "error"})`);
    }

    let text;
    try {
      text = await fs.readFile(this.#file, "utf8");
    } catch (error) {
      if (error?.code === "ENOENT") {
        this.#state = structuredClone(EMPTY_STATE);
        this.#provisioned = false;
        return this;
      }
      throw new StoreUnavailableError(`store: cannot read data file (${error?.code ?? "error"})`);
    }

    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch {
      throw new StoreUnavailableError("store: data file is not valid JSON");
    }
    this.#state = validateState(parsed);
    await this.#reloadProvisioned();
    this.#prune();
    return this;
  }

  async #reloadProvisioned() {
    this.#provisioned = this.#state.users.some((user) => user.role === "owner" && user.status === "active");
  }

  /** Serialises writes: every mutation awaits the previous one. */
  #persist() {
    this.#prune();
    const snapshot = JSON.stringify(this.#state, null, 2);
    this.#writeChain = this.#writeChain.then(async () => {
      const tmp = `${this.#file}.${process.pid}.${Date.now()}.tmp`;
      try {
        await fs.writeFile(tmp, snapshot, { mode: 0o600 });
        await fs.rename(tmp, this.#file);
      } catch (error) {
        await fs.rm(tmp, { force: true }).catch(() => {});
        throw new StoreUnavailableError(`store: cannot write data file (${error?.code ?? "error"})`);
      }
    });
    return this.#writeChain;
  }

  #prune() {
    const now = Date.now();
    this.#state.sessions = this.#state.sessions.filter((session) => {
      const expires = Date.parse(session.expiresAt);
      return Number.isFinite(expires) && expires + SESSION_RETENTION_MS > now;
    });
    if (this.#state.audit.length > AUDIT_LIMIT) {
      this.#state.audit = this.#state.audit.slice(-AUDIT_LIMIT);
    }
  }

  /* ─────────────────────────────── users ─────────────────────────────── */

  listUsers() {
    return this.#state.users.map((user) => ({ ...user }));
  }

  /** @param {string} email already normalised */
  getUserByEmail(email) {
    const user = this.#state.users.find((candidate) => candidate.email === email);
    return user ? { ...user } : null;
  }

  getUserById(id) {
    const user = this.#state.users.find((candidate) => candidate.id === id);
    return user ? { ...user } : null;
  }

  ownerCount() {
    return this.#state.users.filter((user) => user.role === "owner" && user.status === "active").length;
  }

  /**
   * Creates the owner account. Refuses when an active owner already exists, so
   * a second call cannot silently replace the first account.
   */
  async createOwner({ email, displayName, passwordHash }) {
    if (this.ownerCount() > 0) {
      throw new Error("an owner account already exists");
    }
    const now = new Date().toISOString();
    const user = {
      id: randomUUID(),
      email,
      displayName,
      role: "owner",
      status: "active",
      passwordHash,
      createdAt: now,
      updatedAt: now,
      lastSignInAt: null,
    };
    this.#state.users.push(user);
    this.#provisioned = true;
    await this.#persist();
    return { ...user };
  }

  /** Replaces the stored hash (CLI `--rotate-password`). */
  async setPasswordHash(userId, passwordHash) {
    const user = this.#state.users.find((candidate) => candidate.id === userId);
    if (!user) throw new Error("no such user");
    user.passwordHash = passwordHash;
    user.updatedAt = new Date().toISOString();
    await this.#persist();
  }

  async recordSignIn(userId) {
    const user = this.#state.users.find((candidate) => candidate.id === userId);
    const now = new Date().toISOString();
    if (user) {
      user.lastSignInAt = now;
      user.updatedAt = now;
    }
    this.#appendAudit({ at: now, event: "sign_in_success", userId });
    await this.#persist();
  }

  /* ────────────────────────────── sessions ───────────────────────────── */

  async createSession({ userId, tokenHash, csrfToken, expiresAt }) {
    const session = {
      id: randomUUID(),
      userId,
      tokenHash,
      csrfToken,
      createdAt: new Date().toISOString(),
      lastSeenAt: new Date().toISOString(),
      expiresAt,
    };
    this.#state.sessions.push(session);
    await this.#persist();
    return { ...session };
  }

  getSessionByTokenHash(tokenHash) {
    const session = this.#state.sessions.find((candidate) => candidate.tokenHash === tokenHash);
    return session ? { ...session } : null;
  }

  async dropSession(sessionId) {
    const before = this.#state.sessions.length;
    this.#state.sessions = this.#state.sessions.filter((session) => session.id !== sessionId);
    if (this.#state.sessions.length !== before) await this.#persist();
  }

  async dropSessionsForUser(userId, exceptSessionId = null) {
    const before = this.#state.sessions.length;
    this.#state.sessions = this.#state.sessions.filter(
      (session) => session.userId !== userId || session.id === exceptSessionId,
    );
    if (this.#state.sessions.length !== before) await this.#persist();
  }

  /**
   * Extends the idle window. Written at most once a minute per session so a
   * dashboard poll cannot turn every request into a disk write.
   */
  async touchSession(sessionId, { now = new Date(), idleSeconds, absoluteExpiresAt }) {
    const session = this.#state.sessions.find((candidate) => candidate.id === sessionId);
    if (!session) return;
    const lastSeen = Date.parse(session.lastSeenAt);
    const nowMs = now.getTime();
    session.lastSeenAt = now.toISOString();
    const idleExpiry = nowMs + idleSeconds * 1000;
    const absolute = Date.parse(absoluteExpiresAt);
    session.expiresAt = new Date(Math.min(idleExpiry, absolute)).toISOString();
    if (nowMs - lastSeen > 60_000) await this.#persist();
  }

  /* ──────────────────────────────── audit ────────────────────────────── */

  #appendAudit(entry) {
    this.#state.audit.push(entry);
    if (this.#state.audit.length > AUDIT_LIMIT) {
      this.#state.audit = this.#state.audit.slice(-AUDIT_LIMIT);
    }
  }

  async recordAudit(event, userId = null) {
    this.#appendAudit({ at: new Date().toISOString(), event, userId });
    await this.#persist();
  }

  recentAudit(limit = 10) {
    return this.#state.audit.slice(-limit).reverse().map((entry) => ({ ...entry }));
  }

  /** Count of a given event since a timestamp — used by the dashboard. */
  countSince(event, sinceIso) {
    const since = Date.parse(sinceIso);
    return this.#state.audit.filter(
      (entry) => entry.event === event && Number.isFinite(Date.parse(entry.at)) && Date.parse(entry.at) >= since,
    ).length;
  }
}

/**
 * Opens the store, translating every failure into `StoreUnavailableError` so
 * callers have exactly one "portal is not usable" path.
 *
 * @param {string} dir
 * @returns {Promise<Store>}
 */
export async function openStore(dir) {
  const store = new Store(dir);
  try {
    await store.open();
  } catch (error) {
    if (error instanceof StoreUnavailableError) throw error;
    throw new StoreUnavailableError("store: could not be opened");
  }
  return store;
}
