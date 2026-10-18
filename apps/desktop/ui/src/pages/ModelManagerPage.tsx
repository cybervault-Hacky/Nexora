import { EngineStatus } from "../components/EngineStatus";
import { Icon } from "../components/Icon";
import { ModelSetup } from "../components/ModelSetup";
import type { AppStatus } from "../lib/ipc";
import { PageHeader } from "./PageHeader";

interface ModelManagerPageProps {
  status: AppStatus | null;
  statusError: string | null;
  desktop: boolean;
  unloading: boolean;
  onStatus: (status: AppStatus) => void;
  onRefresh: () => void;
  onUnload?: () => void;
}

/**
 * Local Model Manager: the existing ModelSetup flow (runtime probe, GGUF
 * validation, load) plus live engine state. Nexora never downloads a runtime
 * or model — both paths come from the machine.
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
  return (
    <div className="page page--models">
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
      </div>
    </div>
  );
}
