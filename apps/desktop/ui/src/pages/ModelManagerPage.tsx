import { useEffect, useState } from "react";
import { EngineStatus } from "../components/EngineStatus";
import { Icon } from "../components/Icon";
import { ModelSetup } from "../components/ModelSetup";
import { RouterLink } from "../components/RouterLink";
import { fetchSystemInfo } from "../lib/inference";
import type { AppStatus, SystemInfo } from "../lib/ipc";
import { PageHeader } from "./PageHeader";

export interface ModelManagerPageProps {
  status: AppStatus | null;
  statusError: string | null;
  desktop: boolean;
  unloading: boolean;
  onStatus: (status: AppStatus) => void;
  onRefresh: () => void;
  onUnload?: () => void;
}

/**
 * Local Model Manager: the ModelSetup flow (runtime probe, GGUF
 * validation, load) plus live engine state and hardware capabilities.
 * Nexora never downloads a runtime or model — both paths come from the machine.
 */
export function ModelManagerPage({
  status,
  statusError,
  desktop,
  unloading,
  onStatus,
  onRefresh,
  onUnload,
}: ModelManagerPageProps) {
  const [system, setSystem] = useState<SystemInfo | null>(null);

  useEffect(() => {
    let active = true;
    void fetchSystemInfo()
      .then((info) => {
        if (active) setSystem(info);
      })
      .catch(() => {
        // System detection is informational
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="page page--models" data-testid="model-manager-page">
      <PageHeader
        eyebrow="On-device inference"
        title="Local models"
        copy="Connect the llama.cpp runtime and GGUF model you already have. Both are validated by the desktop core before anything is launched, and nothing is downloaded."
      />

      {!desktop ? (
        <div className="notice" role="status">
          <Icon name="cloud-off" size={14} />
          <span>
            This is the browser preview. Loading and unloading models needs the
            Tauri desktop build — the engine is a separate process on your
            machine that a browser tab cannot start or reach.
          </span>
        </div>
      ) : null}

      <div className="page-stack">
        <section className="model-manager__hardware" aria-label="System hardware & runtime target">
          <h3 className="model-manager__hardware-title">
            <Icon name="chip" size={14} />
            Hardware & Inference Environment
          </h3>
          <div className="model-manager__hardware-grid">
            <div className="model-manager__hardware-item">
              <span className="model-manager__hardware-label">System memory</span>
              <span className="model-manager__hardware-val">
                {system && system.totalRamGib
                  ? `${system.availableRamGib ?? "?"} of ${system.totalRamGib} GiB available`
                  : "Detection unavailable"}
              </span>
            </div>
            <div className="model-manager__hardware-item">
              <span className="model-manager__hardware-label">Host platform</span>
              <span className="model-manager__hardware-val">
                {system?.platform ?? "Desktop OS"}
              </span>
            </div>
            <div className="model-manager__hardware-item">
              <span className="model-manager__hardware-label">Engine supervisor</span>
              <span className="model-manager__hardware-val">llama.cpp b11714</span>
            </div>
            <div className="model-manager__hardware-item">
              <span className="model-manager__hardware-label">Format support</span>
              <span className="model-manager__hardware-val">GGUF v3 (Local disk)</span>
            </div>
          </div>
        </section>

        <section className="model-manager__status" aria-label="Engine status">
          <EngineStatus
            status={status}
            failed={statusError !== null}
            collapsed={false}
            onUnload={desktop ? onUnload : undefined}
            unloading={unloading}
          />
          {desktop ? (
            <button
              type="button"
              className="button button--ghost button--sm"
              onClick={onRefresh}
            >
              <Icon name="refresh" size={12} /> Refresh status
            </button>
          ) : null}
        </section>

        {desktop ? (
          <ModelSetup status={status} onStatus={onStatus} onRefresh={onRefresh} />
        ) : (
          <p className="page-note">
            Model setup is available in the desktop app only. The form, probes,
            and load actions are real IPC calls — the preview cannot fake them.
          </p>
        )}

        <div className="setup__actions" style={{ justifyContent: "space-between", marginTop: "8px" }}>
          <RouterLink to="settings" className="button button--ghost button--sm">
            <Icon name="settings" size={12} /> View all Settings
          </RouterLink>
          <RouterLink to="chat" className="button button--ghost button--sm">
            <Icon name="message" size={12} /> Go to AI Chat
          </RouterLink>
        </div>
      </div>
    </div>
  );
}
