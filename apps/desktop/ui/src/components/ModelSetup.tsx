import { useCallback, useEffect, useRef, useState } from "react";
import {
  fetchSystemInfo,
  loadModel,
  probeRuntime,
  saveSettings,
  toInferenceError,
  validateModel,
} from "../lib/inference";
import type {
  AppStatus,
  ModelInfo,
  RuntimeInfo,
  SystemInfo,
} from "../lib/ipc";
import { Icon } from "./Icon";

const CONTEXT_CHOICES = [8192, 16384, 32768, 65536] as const;

interface ModelSetupProps {
  status: AppStatus | null;
  onStatus: (status: AppStatus) => void;
}

type Busy = "runtime" | "model" | "load" | null;

/**
 * Connects the UI to a runtime executable and a GGUF file the user already has.
 *
 * Nexora does not download weights or binaries. Both paths are validated by the
 * Rust core before anything is launched.
 */
export function ModelSetup({ status, onStatus }: ModelSetupProps) {
  const [runtimePath, setRuntimePath] = useState("");
  const [modelPath, setModelPath] = useState("");
  const [contextSize, setContextSize] = useState<number>(32768);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [modelInfo, setModelInfo] = useState<ModelInfo | null>(null);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const seeded = useRef(false);

  useEffect(() => {
    if (seeded.current || !status) {
      return;
    }

    seeded.current = true;
    setRuntimePath(status.runtimePath ?? "");
    setModelPath(status.modelPath ?? "");
    setContextSize(status.contextSize ?? 32768);
  }, [status]);

  useEffect(() => {
    let active = true;

    fetchSystemInfo()
      .then((info) => {
        if (active) {
          setSystem(info);
        }
      })
      .catch(() => {
        // Memory detection is informational; lack of it is not an error.
      });

    return () => {
      active = false;
    };
  }, []);

  const checkRuntime = useCallback(async () => {
    setBusy("runtime");
    setError(null);
    setNote(null);

    try {
      const info = await probeRuntime(runtimePath);
      setRuntimeInfo(info);
      setNote(`Runtime answered: ${info.versionLine}`);
    } catch (cause) {
      setRuntimeInfo(null);
      setError(toInferenceError(cause).message);
    } finally {
      setBusy(null);
    }
  }, [runtimePath]);

  const checkModel = useCallback(async () => {
    setBusy("model");
    setError(null);
    setNote(null);

    try {
      const info = await validateModel(modelPath, contextSize);
      setModelInfo(info);
    } catch (cause) {
      setModelInfo(null);
      setError(toInferenceError(cause).message);
    } finally {
      setBusy(null);
    }
  }, [contextSize, modelPath]);

  const startEngine = useCallback(async () => {
    setBusy("load");
    setError(null);
    setNote("Starting the engine. Loading a large model can take a few minutes.");

    try {
      await saveSettings({
        modelPath: modelPath.trim() === "" ? null : modelPath,
        runtimePath: runtimePath.trim() === "" ? null : runtimePath,
        contextSize,
      });

      const next = await loadModel();
      onStatus(next);
      setNote(null);
    } catch (cause) {
      setError(toInferenceError(cause).message);
      setNote(null);
    } finally {
      setBusy(null);
    }
  }, [contextSize, modelPath, onStatus, runtimePath]);

  const configured = Boolean(status?.modelPath && status?.runtimePath);

  return (
    <section className="setup" aria-label="Local model setup">
      <header className="setup__head">
        <div>
          <p className="eyebrow">Engine setup</p>
          <h2 className="setup__title">Connect your local model</h2>
        </div>
        <span className="chip chip--warning">
          <Icon name="cloud-off" size={13} />
          Offline after setup
        </span>
      </header>

      <p className="setup__lead">
        Nexora runs a model on this machine. Point it at an existing
        <strong> llama-server</strong> executable and a <strong>GGUF</strong>{" "}
        model file — nothing is downloaded.
      </p>

      <div className="setup__grid">
        <div className="field">
          <label className="field__label" htmlFor="runtime-path">
            Runtime executable
          </label>
          <div className="field__row">
            <input
              id="runtime-path"
              className="field__input"
              value={runtimePath}
              spellCheck={false}
              placeholder="C:\\Tools\\llama.cpp\\llama-server.exe"
              onChange={(event) => {
                setRuntimePath(event.target.value);
                setRuntimeInfo(null);
              }}
            />
            <button
              type="button"
              className="button button--ghost"
              onClick={() => void checkRuntime()}
              disabled={busy !== null || runtimePath.trim() === ""}
            >
              {busy === "runtime" ? "Checking…" : "Check"}
            </button>
          </div>
          <p className="field__hint">
            {runtimeInfo
              ? runtimeInfo.versionLine
              : "Build it from the pinned llama.cpp release recorded in docs/phase-2."}
          </p>
          {runtimeInfo && !runtimeInfo.pinnedBuildMatched ? (
            <p className="field__hint field__hint--warning">
              This build does not report the pinned target (llama.cpp b11714).
              It may work, but it has not been verified.
            </p>
          ) : null}
        </div>

        <div className="field">
          <label className="field__label" htmlFor="model-path">
            Model file (.gguf)
          </label>
          <div className="field__row">
            <input
              id="model-path"
              className="field__input"
              value={modelPath}
              spellCheck={false}
              placeholder="D:\\Models\\qwen3.8-27b\\model.gguf"
              onChange={(event) => {
                setModelPath(event.target.value);
                setModelInfo(null);
              }}
            />
            <button
              type="button"
              className="button button--ghost"
              onClick={() => void checkModel()}
              disabled={busy !== null || modelPath.trim() === ""}
            >
              {busy === "model" ? "Checking…" : "Check"}
            </button>
          </div>
          <p className="field__hint">
            {modelInfo
              ? `${modelInfo.fileName} · ${modelInfo.sizeGib} GiB · GGUF v${modelInfo.ggufVersion} · needs ≈ ${modelInfo.estimatedMinRamGib} GiB RAM${modelInfo.fitsAvailableRam === false ? " (may not fit)" : ""}`
              : "The file header is read and checked before the engine starts."}
          </p>
        </div>

        <div className="field field--short">
          <label className="field__label" htmlFor="context-size">
            Context window
          </label>
          <select
            id="context-size"
            className="field__input field__input--select"
            value={contextSize}
            onChange={(event) => {
              setContextSize(Number(event.target.value));
              setModelInfo(null);
            }}
          >
            {CONTEXT_CHOICES.map((choice) => (
              <option key={choice} value={choice}>
                {choice / 1024}K tokens
              </option>
            ))}
          </select>
          <p className="field__hint">
            {system && system.totalRamGib
              ? `${system.availableRamGib ?? "?"} of ${system.totalRamGib} GiB RAM available`
              : "Memory detection unavailable"}
          </p>
        </div>
      </div>

      {modelInfo && modelInfo.warnings.length > 0 ? (
        <ul className="setup__warnings">
          {modelInfo.warnings.map((warning) => (
            <li key={warning}>
              <Icon name="alert" size={13} />
              {warning}
            </li>
          ))}
        </ul>
      ) : null}

      {error ? (
        <p className="setup__error" role="alert">
          <Icon name="alert" size={14} />
          {error}
        </p>
      ) : null}

      {note && !error ? <p className="setup__note">{note}</p> : null}

      <div className="setup__actions">
        <button
          type="button"
          className="button button--primary"
          onClick={() => void startEngine()}
          disabled={busy !== null || modelPath.trim() === "" || runtimePath.trim() === ""}
        >
          {busy === "load" ? "Loading model…" : configured ? "Save and reload" : "Save and load model"}
        </button>
        <span className="setup__footnote">
          The engine listens on 127.0.0.1 only, with a per-session key.
        </span>
      </div>
    </section>
  );
}
