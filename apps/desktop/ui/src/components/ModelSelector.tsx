import { memo, useCallback, useEffect, useRef, useState } from "react";
import type { ModelCatalogEntry } from "../lib/ipc";
import type { ModelCatalog } from "../features/models/useModelCatalog";
import {
  describeModelProfile,
  estimateLines,
  formatDownloadProgress,
  gib,
  isApprovedProfileId,
  readinessLabel,
  readinessTone,
} from "../features/models/readiness";
import {
  cancelModelDownload,
  loadModel,
  startModelDownload,
  toInferenceError,
  verifyModel,
} from "../lib/inference";
import { navigateTo } from "../lib/router";
import { Icon } from "./Icon";

export interface ModelSelectorProps {
  catalog: ModelCatalog;
  /** True while a reply streams: switching mid-answer is not allowed. */
  disabled?: boolean;
  /** Active conversation identifier — closes transient popover cleanly on switch. */
  activeConversationId?: string | null;
  onModelLoaded?: () => void;
}

function describeOption(entry: ModelCatalogEntry): string {
  return `${entry.profile.displayName} — ${readinessLabel(entry.status)}`;
}

/**
 * Dedicated Claude-inspired model selector integrated into the Phase 07 Chat header.
 *
 * Guarantees:
 * - Displays only approved registry profiles (`Nexora Mini`, `Nexora Sonnet`).
 * - Never substitutes another model when `catalog.selected` is null/mismatched.
 * - Never claims readiness before artifact verification and runtime readiness succeed.
 * - Preserves accessible `<select id="model-select">` semantics alongside the
 *   keyboard-navigable dropdown menu (`ArrowUp`/`ArrowDown`/`Home`/`End`/`Enter`/`Escape`),
 *   outside-click dismissal, route-change cleanup, and real download/verify/load actions.
 */
function ModelSelectorInner({
  catalog,
  disabled = false,
  activeConversationId = null,
  onModelLoaded,
}: ModelSelectorProps) {
  const { entries: rawEntries, selected, selecting, error, selectionError, switchError } = catalog;
  const entries = rawEntries.filter((entry) => isApprovedProfileId(entry.profile.id));

  const [popoverOpen, setPopoverOpen] = useState(false);
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [loadingEngine, setLoadingEngine] = useState(false);
  const [verifyingDigest, setVerifyingDigest] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionNote, setActionNote] = useState<string | null>(null);
  const [activeDownload, setActiveDownload] = useState<{
    profileId: string;
    received: number;
    total: number;
    verifying: boolean;
  } | null>(null);
  const [downloadFailedProfileId, setDownloadFailedProfileId] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const mountedRef = useRef(true);
  const actionSeqRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Close transient dropdown cleanly when switching active conversations (§8).
  useEffect(() => {
    setPopoverOpen(false);
  }, [activeConversationId]);

  // Attach outside-pointer, Escape, and route-change listeners ONLY while open (§8).
  useEffect(() => {
    if (!popoverOpen) return;

    const handlePointerDown = (event: MouseEvent | PointerEvent) => {
      const target = event.target as Node | null;
      if (target && containerRef.current && !containerRef.current.contains(target)) {
        setPopoverOpen(false);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        setPopoverOpen(false);
        triggerRef.current?.focus();
      }
    };

    const handleRouteChange = () => {
      setPopoverOpen(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("hashchange", handleRouteChange);

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("hashchange", handleRouteChange);
    };
  }, [popoverOpen]);

  // Never fall back to `entries[0]` when `selected` is null: surface the missing selection honestly.
  const current: ModelCatalogEntry | null =
    selected && isApprovedProfileId(selected.profile.id) ? selected : null;

  const openMenu = useCallback(() => {
    const selectedIdx = current
      ? Math.max(
          0,
          entries.findIndex((item) => item.profile.id === current.profile.id),
        )
      : 0;
    setFocusedIndex(selectedIdx);
    setPopoverOpen(true);
    queueMicrotask(() => {
      itemRefs.current[selectedIdx]?.focus();
    });
  }, [current, entries]);

  if (entries.length === 0 && !selectionError) {
    return null;
  }

  const busy = disabled || selecting || loadingEngine || verifyingDigest;
  const isDownloadingCurrent =
    current !== null && activeDownload?.profileId === current.profile.id;

  const liveProgressText =
    activeDownload && current && activeDownload.profileId === current.profile.id
      ? formatDownloadProgress(activeDownload)
      : null;

  const status =
    liveProgressText ??
    (current
      ? (current.readiness?.detail ?? current.detail ?? null)
      : "No approved model profile is selected. Choose Nexora Mini or Nexora Sonnet.");

  const stateValue = isDownloadingCurrent
    ? activeDownload.verifying
      ? "verifying"
      : "downloading"
    : current?.readiness?.state ?? current?.status;

  const tone = stateValue ? readinessTone(stateValue) : "warning";
  const notice = actionError ?? switchError ?? selectionError ?? null;

  const handleSelectProfile = async (profileId: string) => {
    if (disabled) {
      setActionError(
        "A reply is still streaming. Stop generation before switching models.",
      );
      return;
    }
    setActionError(null);
    setActionNote(null);

    if (current?.profile.id === profileId) {
      setPopoverOpen(false);
      triggerRef.current?.focus();
      return;
    }

    await catalog.select(profileId);
    setPopoverOpen(false);
    triggerRef.current?.focus();
  };

  const handleLoadModel = async () => {
    if (!current || busy) return;
    const seq = ++actionSeqRef.current;
    setActionError(null);
    setActionNote(null);
    setLoadingEngine(true);

    try {
      await loadModel();
      if (!mountedRef.current || seq !== actionSeqRef.current) return;
      catalog.reload();
      onModelLoaded?.();
    } catch (cause: unknown) {
      if (!mountedRef.current || seq !== actionSeqRef.current) return;
      setActionError(toInferenceError(cause).message);
      catalog.reload();
    } finally {
      if (mountedRef.current && seq === actionSeqRef.current) {
        setLoadingEngine(false);
      }
    }
  };

  const handleStartDownload = async (profileId: string) => {
    const target = entries.find((item) => item.profile.id === profileId);
    if (!target || activeDownload !== null) return;

    const seq = ++actionSeqRef.current;
    const expectedTotal =
      target.profile.artifact.state === "approved" ? target.profile.artifact.sizeBytes : 0;

    setActionError(null);
    setActionNote(null);
    setDownloadFailedProfileId(null);
    setActiveDownload({
      profileId,
      received: 0,
      total: expectedTotal,
      verifying: false,
    });

    try {
      const outcome = await startModelDownload(null, profileId, (event) => {
        if (!mountedRef.current || seq !== actionSeqRef.current) return;
        if (event.type === "progress") {
          setActiveDownload({
            profileId,
            received: event.received,
            total: event.total,
            verifying: false,
          });
        } else {
          setActiveDownload((prev) =>
            prev ? { ...prev, total: event.total, verifying: true } : null,
          );
        }
      });

      if (!mountedRef.current || seq !== actionSeqRef.current) return;

      if (outcome.cancelled) {
        setActionNote("Download cancelled. Partial bytes were cleaned up safely.");
      } else if (outcome.path) {
        setActionNote(
          `${target.profile.displayName} downloaded and verified against its approved SHA-256.`,
        );
      }
    } catch (cause: unknown) {
      if (!mountedRef.current || seq !== actionSeqRef.current) return;
      setDownloadFailedProfileId(profileId);
      setActionError(toInferenceError(cause).message);
    } finally {
      if (mountedRef.current && seq === actionSeqRef.current) {
        setActiveDownload(null);
        catalog.reload();
        onModelLoaded?.();
      }
    }
  };

  const handleCancelDownload = () => {
    void cancelModelDownload();
  };

  const handleVerifyInstalled = async () => {
    if (!current?.installedPath || busy) return;
    const seq = ++actionSeqRef.current;
    setActionError(null);
    setActionNote(null);
    setVerifyingDigest(true);

    try {
      const report = await verifyModel(current.installedPath, current.profile.id);
      if (!mountedRef.current || seq !== actionSeqRef.current) return;
      if (report.sizeMatches && report.digestMatches) {
        setActionNote(
          `${current.profile.displayName} SHA-256 verified (${gib(report.sizeBytes)} GiB).`,
        );
      } else {
        setActionError(
          `Integrity check failed for ${current.profile.displayName}: file size or SHA-256 does not match the approved artifact.`,
        );
      }
    } catch (cause: unknown) {
      if (!mountedRef.current || seq !== actionSeqRef.current) return;
      setActionError(toInferenceError(cause).message);
    } finally {
      if (mountedRef.current && seq === actionSeqRef.current) {
        setVerifyingDigest(false);
        catalog.reload();
      }
    }
  };

  const openModelSetupPage = () => {
    setPopoverOpen(false);
    navigateTo("models");
  };

  const canQuickLoad =
    current !== null &&
    (current.status === "ready_to_load" ||
      current.status === "downloaded_verified" ||
      current.status === "runtime_failure") &&
    Boolean(current.readiness?.canLoad) &&
    !busy &&
    activeDownload === null;

  const canStartDownload =
    current !== null &&
    current.downloadable &&
    Boolean(current.readiness?.canDownload) &&
    activeDownload === null &&
    (current.status === "ready_to_download" ||
      current.status === "download_cancelled" ||
      current.status === "download_incomplete" ||
      current.status === "verification_failed" ||
      downloadFailedProfileId === current.profile.id);

  const isRetryDownload =
    current !== null &&
    (current.status === "download_incomplete" ||
      current.status === "verification_failed" ||
      downloadFailedProfileId === current.profile.id);

  const canVerifyFile =
    current !== null &&
    current.status === "unverified" &&
    Boolean(current.installedPath) &&
    !busy &&
    activeDownload === null;

  const selectedEstimates = current?.readiness?.estimate
    ? estimateLines(current.readiness.estimate)
    : null;

  const nonOkChecks =
    current?.readiness?.checks.filter((check) => check.status !== "ok") ?? [];

  const handleMenuKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (entries.length === 0) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      const next = (focusedIndex + 1) % entries.length;
      setFocusedIndex(next);
      itemRefs.current[next]?.focus();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      const prev = (focusedIndex - 1 + entries.length) % entries.length;
      setFocusedIndex(prev);
      itemRefs.current[prev]?.focus();
    } else if (event.key === "Home") {
      event.preventDefault();
      setFocusedIndex(0);
      itemRefs.current[0]?.focus();
    } else if (event.key === "End") {
      event.preventDefault();
      const last = entries.length - 1;
      setFocusedIndex(last);
      itemRefs.current[last]?.focus();
    }
  };

  return (
    <div className="model-select" ref={containerRef} data-testid="model-selector">
      <div className="model-select__row">
        {/* Accessible native select control kept synchronized with core catalog */}
        <label className="sr-only" htmlFor="model-select">
          Model
        </label>
        <select
          id="model-select"
          className="sr-only model-select__native"
          tabIndex={-1}
          value={current?.profile.id ?? ""}
          disabled={busy}
          aria-describedby="model-select-status"
          title={
            disabled
              ? "Stop generating before switching models"
              : "Choose the local model"
          }
          onChange={(event) => {
            setActionError(null);
            setActionNote(null);
            void catalog.select(event.target.value);
          }}
        >
          {current === null ? (
            <option value="" disabled>
              Select an approved model…
            </option>
          ) : null}
          {entries.map((entry) => (
            <option key={entry.profile.id} value={entry.profile.id}>
              {describeOption(entry)}
            </option>
          ))}
        </select>

        {/* Primary Claude-inspired interactive model selector pill */}
        <button
          ref={triggerRef}
          type="button"
          className={`model-select__trigger${popoverOpen ? " model-select__trigger--open" : ""}`}
          data-testid="model-selector-trigger"
          data-state={stateValue}
          data-streaming={disabled ? "true" : undefined}
          aria-haspopup="menu"
          aria-expanded={popoverOpen}
          aria-controls="model-selector-popover"
          aria-describedby="model-select-status"
          title={
            disabled
              ? "A reply is still streaming — stop generation before switching models"
              : "Select local model profile and inspect readiness"
          }
          onClick={() => {
            if (popoverOpen) {
              setPopoverOpen(false);
            } else {
              openMenu();
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown" && !popoverOpen) {
              event.preventDefault();
              openMenu();
            }
          }}
        >
          <span
            className={`model-select__dot model-select__dot--${tone}`}
            aria-hidden="true"
          />
          <span className="model-select__name">
            {current ? current.profile.displayName : "Select model"}
          </span>
          <span className={`model-select__badge model-select__badge--${tone}`}>
            {selecting
              ? "Switching…"
              : loadingEngine
                ? "Loading…"
                : disabled
                  ? "Streaming"
                  : stateValue
                    ? readinessLabel(stateValue)
                    : "Unselected"}
          </span>
          <Icon name="chevron-down" size={13} className="model-select__chevron" />
        </button>

        {canQuickLoad ? (
          <button
            type="button"
            className="button button--ghost button--sm"
            data-testid="selector-quick-load-btn"
            onClick={() => void handleLoadModel()}
            disabled={busy}
          >
            {loadingEngine
              ? "Loading…"
              : current?.status === "runtime_failure"
                ? "Retry load"
                : "Load model"}
          </button>
        ) : null}

        {activeDownload ? (
          <button
            type="button"
            className="button button--ghost button--sm"
            data-testid="selector-inline-cancel-btn"
            onClick={handleCancelDownload}
          >
            Cancel download
          </button>
        ) : null}
      </div>

      {popoverOpen ? (
        <div
          id="model-selector-popover"
          className="model-popover"
          data-testid="model-selector-popover"
          onKeyDown={handleMenuKeyDown}
        >
          <div className="model-popover__header">
            <span className="model-popover__heading">Local Models</span>
            <span className="model-popover__privacy">Runs on device · 127.0.0.1</span>
          </div>

          {disabled ? (
            <p className="model-popover__streaming-note" role="status">
              Generation in progress — stop the current reply before switching models.
            </p>
          ) : null}

          <div
            className="model-popover__menu"
            role="menu"
            aria-label="Approved local models"
          >
            {entries.map((entry, idx) => {
              const isSelected = entry.profile.id === current?.profile.id;
              const isDownloadingThis = activeDownload?.profileId === entry.profile.id;
              const entryState = isDownloadingThis
                ? activeDownload.verifying
                  ? "verifying"
                  : "downloading"
                : entry.status;
              const entryTone = readinessTone(entryState);
              const rowDisabled = busy && !isSelected;

              return (
                <button
                  key={entry.profile.id}
                  ref={(el) => {
                    itemRefs.current[idx] = el;
                  }}
                  type="button"
                  role="menuitemradio"
                  aria-checked={isSelected}
                  aria-disabled={rowDisabled || undefined}
                  disabled={rowDisabled}
                  data-testid={`model-option-${entry.profile.id}`}
                  data-state={entryState}
                  className={`model-popover__item${
                    isSelected ? " model-popover__item--active" : ""
                  }`}
                  onFocus={() => setFocusedIndex(idx)}
                  onClick={() => void handleSelectProfile(entry.profile.id)}
                >
                  <div className="model-popover__item-top">
                    <div className="model-popover__item-title-group">
                      <span className="model-popover__item-name">
                        {entry.profile.displayName}
                      </span>
                      <span className="model-popover__meta">
                        {entry.profile.quantization} ·{" "}
                        {Math.round(entry.profile.context.recommended / 1024)}k ctx
                      </span>
                    </div>

                    <div className="model-popover__item-right">
                      <span
                        className={`model-select__badge model-select__badge--${entryTone}`}
                      >
                        {readinessLabel(entryState)}
                      </span>
                      {isSelected ? (
                        <Icon
                          name="check"
                          size={14}
                          className="model-popover__check"
                        />
                      ) : null}
                    </div>
                  </div>

                  <span className="model-popover__desc">
                    {describeModelProfile(entry.profile)}
                  </span>

                  <span className="model-popover__detail">
                    {isDownloadingThis
                      ? formatDownloadProgress(activeDownload)
                      : (entry.readiness?.detail ?? entry.detail)}
                  </span>
                </button>
              );
            })}
          </div>

          {current ? (
            <div
              className="model-popover__details"
              data-testid="model-selector-details"
            >
              <div className="model-popover__details-head">
                <span className="model-popover__details-title">
                  {current.profile.displayName} readiness &amp; specs
                </span>
                <span className="model-popover__meta">
                  {current.profile.format} · {current.profile.licence.id}
                </span>
              </div>

              {selectedEstimates ? (
                <dl className="model-popover__specs">
                  <div>
                    <dt>Approved artifact</dt>
                    <dd>
                      {current.profile.artifact.state === "approved"
                        ? `${selectedEstimates.fileGib} GiB`
                        : "Pending"}
                    </dd>
                  </div>
                  <div>
                    <dt>Est. memory</dt>
                    <dd>≈{selectedEstimates.memoryGib} GiB</dd>
                  </div>
                  <div>
                    <dt>Disk needed</dt>
                    <dd>{selectedEstimates.requiredFreeGib} GiB</dd>
                  </div>
                  <div>
                    <dt>Local benchmark</dt>
                    <dd>
                      {current.profile.locallyVerified
                        ? "Verified"
                        : "Unverified on host"}
                    </dd>
                  </div>
                </dl>
              ) : null}

              {nonOkChecks.length > 0 ? (
                <ul className="model-popover__checks" aria-label="Readiness checks">
                  {nonOkChecks.map((check, index) => (
                    <li
                      key={`${index}-${check.id}`}
                      className={`model-popover__check-item model-popover__check-item--${check.status}`}
                    >
                      <Icon name="alert" size={12} />
                      <span>{check.message}</span>
                    </li>
                  ))}
                </ul>
              ) : null}

              {actionNote ? (
                <p className="model-popover__note" role="status">
                  {actionNote}
                </p>
              ) : null}

              <div className="model-popover__actions">
                {activeDownload ? (
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    data-testid="selector-cancel-download-btn"
                    onClick={handleCancelDownload}
                  >
                    Cancel download
                  </button>
                ) : canStartDownload ? (
                  <button
                    type="button"
                    className="button button--primary button--sm"
                    data-testid={
                      isRetryDownload
                        ? "selector-retry-download-btn"
                        : "selector-download-btn"
                    }
                    onClick={() => void handleStartDownload(current.profile.id)}
                  >
                    <Icon name="download" size={12} />
                    {isRetryDownload
                      ? `Retry ${current.profile.displayName} download`
                      : `Download ${current.profile.displayName}`}
                  </button>
                ) : null}

                {canVerifyFile ? (
                  <button
                    type="button"
                    className="button button--primary button--sm"
                    data-testid="selector-verify-btn"
                    onClick={() => void handleVerifyInstalled()}
                    disabled={busy}
                  >
                    {verifyingDigest ? "Verifying…" : "Verify SHA-256"}
                  </button>
                ) : null}

                {canQuickLoad ? (
                  <button
                    type="button"
                    className="button button--primary button--sm"
                    data-testid="selector-load-btn"
                    onClick={() => void handleLoadModel()}
                    disabled={busy}
                  >
                    {loadingEngine
                      ? "Loading…"
                      : current.status === "runtime_failure"
                        ? "Retry load"
                        : "Load model"}
                  </button>
                ) : null}

                <button
                  type="button"
                  className="button button--ghost button--sm"
                  data-testid="selector-refresh-btn"
                  onClick={() => catalog.reload()}
                  disabled={catalog.loading}
                >
                  <Icon name="refresh" size={12} />
                  Refresh
                </button>

                <button
                  type="button"
                  className="button button--ghost button--sm"
                  data-testid="selector-inspect-btn"
                  onClick={openModelSetupPage}
                >
                  <Icon name="settings" size={12} />
                  Inspect setup
                </button>
              </div>
            </div>
          ) : (
            <div
              className="model-popover__details"
              data-testid="model-selector-details"
            >
              <div className="model-popover__actions">
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  data-testid="selector-inspect-btn"
                  onClick={openModelSetupPage}
                >
                  <Icon name="settings" size={12} />
                  Open model setup
                </button>
              </div>
            </div>
          )}
        </div>
      ) : null}

      <p
        className={`model-select__status${
          notice ? " model-select__status--warning" : ""
        }`}
        id="model-select-status"
        aria-live="polite"
        data-state={stateValue}
        role={notice ? "status" : undefined}
      >
        {error ? `Model list unavailable: ${error}` : (notice ?? status ?? "")}
      </p>
    </div>
  );
}

export const ModelSelector = memo(ModelSelectorInner);
