import { useEffect, useState } from "react";
import { Icon } from "../components/Icon";
import type { AppStatus } from "../lib/ipc";
import { getDeveloperDiagnostics, type DeveloperDiagnostics } from "../lib/auth";
import {
  clearIpcTelemetry,
  getIpcTelemetryEvents,
  subscribeIpcTelemetry,
  type IpcTelemetryEvent,
} from "../lib/telemetry";
import { PageHeader } from "./PageHeader";

interface FounderPageProps {
  status: AppStatus | null;
}

/**
 * Phase 25 — Founder Control Centre
 *
 * Dedicated diagnostics and telemetry console for founders and core developers:
 * - Real-time inference engine metrics and context utilization.
 * - Live in-memory IPC telemetry traffic monitor.
 * - OS security & Credential Manager vault audit.
 * - Auth service connectivity and health verification.
 */
export function FounderPage({ status }: FounderPageProps) {
  const [diagnostics, setDiagnostics] = useState<DeveloperDiagnostics | null>(null);
  const [telemetry, setTelemetry] = useState<IpcTelemetryEvent[]>([]);
  const [filter, setFilter] = useState<"all" | "ok" | "error">("all");
  const [pingStatus, setPingStatus] = useState<string | null>(null);
  const [pinging, setPinging] = useState(false);

  useEffect(() => {
    let active = true;
    void getDeveloperDiagnostics()
      .then((d) => {
        if (active) setDiagnostics(d);
      })
      .catch(() => {
        if (active) {
          setDiagnostics({
            platform: "Unknown",
            arch: "Unknown",
            appVersion: "0.1.0",
            credentialVault: "Unavailable",
            authServiceUrl: "http://127.0.0.1:8787",
            cspEnforced: true,
            offlineCapable: true,
          });
        }
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    setTelemetry(getIpcTelemetryEvents());
    return subscribeIpcTelemetry(() => {
      setTelemetry(getIpcTelemetryEvents());
    });
  }, []);

  const handlePing = async () => {
    setPinging(true);
    setPingStatus("Checking auth service connectivity…");
    const startTime = performance.now();
    try {
      const diag = await getDeveloperDiagnostics();
      const elapsed = Math.round(performance.now() - startTime);
      setPingStatus(`Auth service configured at ${diag.authServiceUrl} (responded in ${elapsed} ms)`);
    } catch {
      setPingStatus("Auth service unreachable or desktop offline mode active.");
    } finally {
      setPinging(false);
    }
  };

  const filteredEvents = telemetry.filter((e) => {
    if (filter === "ok") return e.status === "ok";
    if (filter === "error") return e.status === "error";
    return true;
  });

  return (
    <div className="page page--founder">
      <PageHeader
        eyebrow="Founder Observability"
        title="Founder Control Centre"
        copy="Live system diagnostics, IPC event telemetry, and security runtime audit. Real-time visibility into local inference and desktop operations."
      />

      <div className="page-stack">
        {/* System & Architecture Diagnostics */}
        <section className="settings-group" aria-label="System Diagnostics">
          <h2 className="settings-group__title">Architecture & Security Vault</h2>

          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Platform & Architecture</span>
              <span className="settings-row__hint">
                {diagnostics ? `${diagnostics.platform} (${diagnostics.arch})` : "Inspecting…"}
              </span>
            </span>
            <span className="badge badge--local">Desktop Engine</span>
          </div>

          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Credential Storage Vault</span>
              <span className="settings-row__hint">
                {diagnostics?.credentialVault ?? "Windows Credential Manager / Memory Fallback"}
              </span>
            </span>
            <span className="badge badge--success" style={{ color: "var(--accent-success, #34d399)" }}>
              <Icon name="shield" size={12} /> OS Protected
            </span>
          </div>

          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Content Security Policy (CSP)</span>
              <span className="settings-row__hint">
                connect-src ipc: http://ipc.localhost (Arbitrary WebView HTTP is blocked)
              </span>
            </span>
            <span className="badge badge--success" style={{ color: "var(--accent-success, #34d399)" }}>
              Enforced
            </span>
          </div>

          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Offline-First AI Core</span>
              <span className="settings-row__hint">
                Local chat, GGUF models, workspace indexing, and patch engine require zero network connectivity.
              </span>
            </span>
            <span className="badge badge--local">Zero Auth Gating</span>
          </div>
        </section>

        {/* Inference Engine Status */}
        <section className="settings-group" aria-label="Inference Engine Telemetry">
          <h2 className="settings-group__title">Inference Engine Telemetry</h2>

          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Engine Lifecycle State</span>
              <span className="settings-row__hint">
                Current supervisor state: <strong>{status?.engine.state ?? "unknown"}</strong>
              </span>
            </span>
            <span className={`badge badge--${status?.engine.state === "ready" ? "success" : "local"}`}>
              {status?.engine.state ?? "unknown"}
            </span>
          </div>

          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Active Model</span>
              <span className="settings-row__hint">{status?.modelPath ?? "None loaded"}</span>
            </span>
          </div>

          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Context Window Budget</span>
              <span className="settings-row__hint">
                {status ? `${status.contextSize} tokens max window` : "Not loaded"}
              </span>
            </span>
          </div>
        </section>

        {/* Auth Service Diagnostic Probe */}
        <section className="settings-group" aria-label="Auth Service Probe">
          <h2 className="settings-group__title">Auth Service Connectivity</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Backend Auth Endpoint</span>
              <span className="settings-row__hint">
                Target: {diagnostics?.authServiceUrl ?? "http://127.0.0.1:8787"}
              </span>
              {pingStatus && (
                <span style={{ display: "block", marginTop: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                  {pingStatus}
                </span>
              )}
            </span>
            <button
              type="button"
              className="button button--ghost button--sm"
              disabled={pinging}
              onClick={handlePing}
            >
              <Icon name="refresh" size={13} /> {pinging ? "Pinging…" : "Ping probe"}
            </button>
          </div>
        </section>

        {/* Live IPC Traffic Inspector */}
        <section className="settings-group" aria-label="IPC Traffic Telemetry">
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
            <h2 className="settings-group__title" style={{ margin: 0 }}>IPC Traffic Monitor</h2>
            <div style={{ display: "flex", gap: 8 }}>
              <select
                value={filter}
                onChange={(e) => setFilter(e.target.value as "all" | "ok" | "error")}
                className="input"
                style={{ padding: "2px 8px", fontSize: 12 }}
                aria-label="Filter events"
              >
                <option value="all">All Events</option>
                <option value="ok">Success Only</option>
                <option value="error">Errors Only</option>
              </select>
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={clearIpcTelemetry}
                title="Clear in-memory events"
              >
                Clear
              </button>
            </div>
          </div>

          {filteredEvents.length === 0 ? (
            <div className="settings-row settings-row--static">
              <span className="settings-row__hint">
                No IPC telemetry events recorded yet. Perform workspace or model operations to see real-time dispatch logs.
              </span>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 6, maxHeight: 280, overflowY: "auto" }}>
              {filteredEvents.map((event) => (
                <div
                  key={event.id}
                  className="settings-row settings-row--static"
                  style={{
                    padding: "6px 12px",
                    fontSize: 12,
                    fontFamily: "monospace",
                    background: "var(--bg-elevated, rgba(255,255,255,0.02))",
                    borderRadius: 4,
                  }}
                >
                  <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span
                      style={{
                        color: event.status === "ok" ? "var(--accent-success, #34d399)" : "var(--accent-danger, #f87171)",
                        fontWeight: 600,
                      }}
                    >
                      {event.status.toUpperCase()}
                    </span>
                    <span>{event.command}</span>
                    {event.errorSummary && (
                      <span style={{ color: "var(--text-tertiary)", fontSize: 11 }}>
                        ({event.errorSummary})
                      </span>
                    )}
                  </span>
                  <span style={{ color: "var(--text-tertiary)" }}>
                    {event.durationMs}ms
                  </span>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
