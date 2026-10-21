import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelModelDownload,
  fetchModelProfile,
  fetchModelReadiness,
  fetchSystemInfo,
  loadModel,
  probeRuntime,
  saveSettings,
  startModelDownload,
  toInferenceError,
  validateModel,
  verifyModel,
} from "../lib/inference";
import { isDesktopShell } from "../lib/ipc";
import type {
  AppStatus,
  InferenceError,
  IntegrityReport,
  ModelInfo,
  ModelProfile,
  Readiness,
  RuntimeInfo,
  SystemInfo,
} from "../lib/ipc";
import { Icon } from "./Icon";
import { ModelReadinessCard } from "./ModelReadinessCard";

const CONTEXT_CHOICES = [8192, 16384, 32768, 65536] as const;

const GIB = 1024 ** 3;

function gib(bytes: number): string {
  return (bytes / GIB).toFixed(2);
}

function progressLabel(state: {
  received: number;
  total: number;
  verifying: boolean;
}): string {
  if (state.verifying) {
    return "Verifying the SHA-256 before publishing the file…";
  }

  if (state.total <= 0) {
    return "Starting the download…";
  }

  const percent = Math.min(100, Math.round((state.received / state.total) * 100));

  return `Downloading ${gib(state.received)} of ${gib(state.total)} GiB (${percent}%).`;
}

function integrityLabel(report: IntegrityReport): string {
  if (report.digestMatches) {
    return "This file matches the approved Nexora Mini SHA-256.";
  }

  if (report.sizeMatches) {
    return "The size matches, but the SHA-256 differs. This is not the approved Nexora Mini file.";
  }

  return `This file is ${gib(report.sizeBytes)} GiB; the approved Nexora Mini file is ${gib(
    report.expectedSizeBytes,
  )} GiB. It may still be a valid GGUF, and Nexora will treat it as a custom model.`;
}

interface ModelSetupProps {
  status: AppStatus | null;
  onStatus: (status: AppStatus) => void;
  /** Re-reads the authoritative status, e.g. after a failed load. */
  onRefresh?: () => void;
  /** Selected profile id; a change refetches the profile metadata. */
  profileId?: string | null;
  /**
   * Asks the application to re-read the model catalogue after something that
   * changes a row (a download, a digest check, a load). The selector must not
   * keep claiming "ready to download" for a file that now exists.
   */
  onModelsChanged?: () => void;
}

type Busy = "runtime" | "model" | "load" | "verify" | null;

/**
 * Connects the UI to a runtime executable and a GGUF file the user already has.
 *
 * Nexora does not download weights or binaries. Both paths are validated by the
 * Rust core before anything is launched.
 */
export function ModelSetup({
  status,
  onStatus,
  onRefresh,
  profileId = null,
  onModelsChanged,
}: ModelSetupProps) {
  const [runtimePath, setRuntimePath] = useState("");
  const [modelPath, setModelPath] = useState("");
  const [contextSize, setContextSize] = useState<number>(32768);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeInfo | null>(null);
  const [modelInfo, setModelInfo] = useState<ModelInfo | null>(null);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [profile, setProfile] = useState<ModelProfile | null>(null);
  const [download, setDownload] = useState<{
    received: number;
    total: number;
    verifying: boolean;
  } | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [integrity, setIntegrity] = useState<IntegrityReport | null>(null);
  const [readiness, setReadiness] = useState<Readiness | null>(null);
  const [readinessError, setReadinessError] = useState<string | null>(null);
  const [readinessBusy, setReadinessBusy] = useState(false);

  const desktop = isDesktopShell();

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

  useEffect(() => {
    let active = true;

    fetchModelProfile()
      .then((value) => {
        if (active) {
          setProfile(value);
        }
      })
      .catch(() => {
        // The profile is descriptive metadata; setup still works without it.
      });

    return () => {
      active = false;
    };
  }, [profileId]);

  /**
   * Re-read the core's readiness verdict. Never derived here: the card shows
   * exactly what the backend measured, including its "unknown" answers.
   */
  const refreshReadiness = useCallback(async () => {
    if (!isDesktopShell()) {
      return;
    }

    setReadinessBusy(true);

    try {
      const next = await fetchModelReadiness();
      setReadiness(next);
      setReadinessError(null);
    } catch (cause) {
      setReadiness(null);
      setReadinessError(toInferenceError(cause).message);
    } finally {
      setReadinessBusy(false);
    }
  }, []);

  useEffect(() => {
    void refreshReadiness();
  }, [profileId, refreshReadiness]);

  /** Size of the approved artefact, or null while it is not approved. */
  const approved = profile?.artifact.state === "approved" ? profile.artifact : null;
  const approvedBytes = approved ? approved.sizeBytes : null;

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

  const checkIntegrity = useCallback(async () => {
    setBusy("verify");
    setError(null);
    setNote(null);
    setIntegrity(null);

    try {
      const report = await verifyModel(modelPath, profile?.id);
      setIntegrity(report);
    } catch (cause) {
      setError(toInferenceError(cause).message);
    } finally {
      setBusy(null);
      void refreshReadiness();
      onModelsChanged?.();
    }
  }, [modelPath, onModelsChanged, profile, refreshReadiness]);

  const downloadModel = useCallback(async () => {
    setDownloadError(null);
    setIntegrity(null);
    setNote(null);
    setDownload({ received: 0, total: approvedBytes ?? 0, verifying: false });

    try {
      const outcome = await startModelDownload(null, profile?.id ?? "", (event) => {
        if (event.type === "progress") {
          setDownload({ received: event.received, total: event.total, verifying: false });
        } else {
          setDownload((current) => (current ? { ...current, verifying: true } : current));
        }
      });

      if (outcome.cancelled) {
        setNote("Download cancelled. Nothing was installed.");
      } else if (outcome.path) {
        // Prefill only: the user still reviews the path and saves it.
        setModelPath(outcome.path);
        setModelInfo(null);
        setNote(
          "Downloaded and verified against the approved SHA-256. Review the path, then save and load.",
        );
      }
    } catch (cause) {
      setDownloadError(toInferenceError(cause).message);
    } finally {
      setDownload(null);
      // The download either published a verified file, left a partial one, or
      // wrote nothing; every one of those changes the row's state.
      void refreshReadiness();
      onModelsChanged?.();
    }
  }, [profile, approvedBytes, refreshReadiness, onModelsChanged]);

  const stopDownload = useCallback(() => {
    void cancelModelDownload();
  }, []);

  /**
   * Loads through the core's verification gate.
   *
   * `load_model` refuses a published artefact whose digest has not been
   * confirmed in this session. Rather than making the user find the Verify
   * button first, this hashes the chosen file once and retries — but the core
   * still decides both times, and a mismatch is reported as such.
   */
  const loadForThisSession = useCallback(async (): Promise<AppStatus> => {
    try {
      return await loadModel();
    } catch (cause) {
      const failure = toInferenceError(cause);

      if (failure.code !== "model_unverified") {
        throw failure;
      }

      setNote("This file has not been verified in this session. Hashing it before loading…");

      const report = await verifyModel(modelPath, profile?.id);

      if (!report.sizeMatches || !report.digestMatches) {
        const mismatch: InferenceError = {
          code: "model_unverified",
          message: report.sizeMatches
            ? "The SHA-256 differs from the approved file, so Nexora will not load it."
            : `This file is ${gib(report.sizeBytes)} GiB, not the approved ${gib(
                report.expectedSizeBytes,
              )} GiB. Nexora will not load it as ${profile?.displayName ?? "this model"}.`,
        };

        setIntegrity(report);
        throw mismatch;
      }

      setIntegrity(report);
      setNote("Verified. Starting the engine…");

      return await loadModel();
    }
  }, [modelPath, profile]);

  const startEngine = useCallback(async () => {
    setBusy("load");
    setError(null);
    setNote("Starting the engine. Loading a large model can take a few minutes.");

    try {
      await saveSettings({
        modelPath: modelPath.trim() === "" ? null : modelPath,
        runtimePath: runtimePath.trim() === "" ? null : runtimePath,
        contextSize,
        modelId: profile?.id ?? null,
      });

      const next = await loadForThisSession();
      onStatus(next);
      setNote(null);
    } catch (cause) {
      setError(toInferenceError(cause).message);
      setNote(null);
      // The core keeps its own failure state (exit status and the engine's
      // last words). Pull it in so the sidebar and chip say "Engine failed"
      // instead of silently staying on "No model loaded".
      onRefresh?.();
    } finally {
      setBusy(null);
      void refreshReadiness();
      onModelsChanged?.();
    }
  }, [
    contextSize,
    loadForThisSession,
    modelPath,
    onModelsChanged,
    onRefresh,
    onStatus,
    profile,
    runtimePath,
  ]);

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
        Nexora runs a model on this machine. Download the approved{" "}
        <strong>Nexora Mini</strong> file, or choose a{" "}
        <strong>llama-server</strong> executable and a <strong>GGUF</strong> model
        on this machine. Downloads are explicit and verified; nothing is fetched
        at startup.
      </p>

      <section className="setup__mini" aria-label="Approved model profile">
        <div className="setup__mini-head">
          <div>
            <p className="eyebrow">Selected model</p>
            <h3 className="setup__mini-title">
              {profile?.displayName ?? "Nexora Mini"}
            </h3>
          </div>
          <span className="chip">
            {profile
              ? `${profile.quantization} · ${profile.licence.id}`
              : "Reading profile…"}
          </span>
        </div>

        <p className="field__hint">
          {profile
            ? `${profile.upstreamModel} · ${
                approvedBytes === null
                  ? "artefact not approved"
                  : `${gib(approvedBytes)} GiB`
              } · recommended context ${profile.context.recommended.toLocaleString("en-US")} tokens`
            : "The selected model profile is read from the desktop core."}
        </p>

        <div className="setup__mini-actions">
          <button
            type="button"
            className="button button--ghost"
            onClick={() => void downloadModel()}
            disabled={
              !desktop ||
              busy !== null ||
              download !== null ||
              approved === null ||
              (readiness !== null && !readiness.canDownload)
            }
            title={
              approved === null
                ? "This model has no approved artefact yet. Nexora will not download a file it cannot verify."
                : readiness !== null && !readiness.canDownload
                  ? readiness.detail
                  : `Download ${profile?.displayName ?? "the model"}`
            }
          >
            {download ? "Downloading…" : `Download ${profile?.displayName ?? "model"}`}
          </button>

          {download ? (
            <button
              type="button"
              className="button button--ghost"
              onClick={stopDownload}
            >
              Cancel download
            </button>
          ) : null}

          <button
            type="button"
            className="button button--ghost"
            onClick={() => void checkIntegrity()}
            disabled={
              busy !== null || download !== null || modelPath.trim() === "" || approved === null
            }
            title={
              approved === null
                ? "Verification needs an approved digest for this model."
                : "Hash this file and compare it with the approved digest"
            }
          >
            {busy === "verify" ? "Hashing…" : "Verify my file"}
          </button>
        </div>

        <p className="field__hint">
          {download
            ? progressLabel(download)
            : profile?.artifact.state === "pending"
              ? profile.artifact.note
              : "A download starts only when you ask for it and is verified before it is published."}
        </p>

        {downloadError ? (
          <p className="setup__error" role="alert">
            <Icon name="alert" size={14} />
            {downloadError}
          </p>
        ) : null}

        {integrity ? <p className="setup__note">{integrityLabel(integrity)}</p> : null}
      </section>

      <ModelReadinessCard
        readiness={readiness}
        error={readinessError}
        onRefresh={desktop ? () => void refreshReadiness() : undefined}
        refreshing={readinessBusy}
      />

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
              ? `${modelInfo.fileName} · ${modelInfo.sizeGib} GiB · GGUF v${modelInfo.ggufVersion}${
                  modelInfo.contextLength
                    ? ` · context ${modelInfo.contextLength.toLocaleString("en-US")}`
                    : ""
                } · needs ≈ ${modelInfo.estimatedMinRamGib} GiB RAM${modelInfo.fitsAvailableRam === false ? " (may not fit)" : ""}`
              : "The file header and metadata are read before the engine starts."}
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
