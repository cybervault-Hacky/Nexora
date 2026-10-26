import { useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "../components/Icon";
import { checkBlenderInstallation } from "../features/integrations/native";
import { PageHeader } from "./PageHeader";
import {
  APP_INTEGRATIONS,
  CONNECTION_STATUS_LABELS,
  INTEGRATION_CATEGORIES,
  filterAppIntegrations,
  type AppIntegrationDefinition,
  type ConnectionStatus,
  type InstallationStatus,
} from "../features/integrations/catalogue";

function installationLabel(status: InstallationStatus): string {
  switch (status.state) {
    case "unknown":
      return "Unknown — installation has not been checked";
    case "checking":
      return "Checking supported installation locations…";
    case "found": {
      const candidates = status.installations
        .map((installation) =>
          `Found candidate only — ${installation.executablePath}; version ${installation.version ?? "not verified"}. ${installation.validation}`,
        )
        .join(" ");
      const incomplete = status.incompleteLocations.length > 0
        ? ` Incomplete locations: ${status.incompleteLocations.join(", ")}.`
        : "";
      return `${candidates}${incomplete}`;
    }
    case "not_found":
      return `Not found — ${status.checkedLocations.join(", ") || "no standard locations available"} checked`;
    case "unsupported":
      return `Detection unsupported — ${status.reason}`;
    case "error": {
      const incomplete = status.incompleteLocations.length > 0
        ? ` Incomplete locations: ${status.incompleteLocations.join(", ")}.`
        : "";
      return `Detection error — ${status.message}${incomplete}`;
    }
    case "access_denied": {
      const locations = status.incompleteLocations.length > 0
        ? ` Locations: ${status.incompleteLocations.join(", ")}.`
        : "";
      return `Detection access denied — ${status.reason}${locations}`;
    }
  }
}

function taskReadinessLabel(readiness: AppIntegrationDefinition["taskReadiness"]): string {
  switch (readiness.state) {
    case "ready":
      return `Ready — ${readiness.capabilityIds.join(", ")}`;
    case "blocked":
      return `Not ready — ${readiness.reasons.join("; ")}`;
    case "unknown":
      return `Unknown — ${readiness.reason}`;
    case "unavailable":
      return `Unavailable — ${readiness.reason}`;
  }
}

function connectionLabel(status: ConnectionStatus | null): string {
  if (status === null) return "Unavailable — no connector or validated connection handshake";
  const label = CONNECTION_STATUS_LABELS[status.state];
  if (status.state === "error") return `${label}${status.retryable ? " — retry may be available" : ""}`;
  if (status.state === "unsupported") return `${label} — ${status.reason}`;
  if (status.state === "needs_setup") return `${label} — ${status.requirements.join("; ")}`;
  if (status.state === "disconnected") return `${label} — ${status.reason}`;
  return label;
}

function categoryLabel(category: AppIntegrationDefinition["category"]): string {
  switch (category) {
    case "3d":
      return "3D & modeling";
    case "development":
      return "Development";
    case "design":
      return "Design";
  }
}

function IntegrationCard({
  integration,
  onCheckInstallation,
  isCheckingInstallation,
}: {
  integration: AppIntegrationDefinition;
  onCheckInstallation?: () => void;
  isCheckingInstallation?: boolean;
}) {
  const hasConnector = integration.implementationStatus === "implemented";
  const capabilities = integration.capabilities;

  return (
    <article className="integration-card" aria-labelledby={`integration-${integration.id}-title`}>
      <header className="integration-card__header">
        <span className="integration-card__icon" aria-hidden="true">
          <Icon name={integration.icon} size={21} />
        </span>
        <div className="integration-card__identity">
          <p className="integration-card__category">{categoryLabel(integration.category)}</p>
          <h2 id={`integration-${integration.id}-title`} className="integration-card__title">
            {integration.name}
          </h2>
        </div>
        <span className={`integration-status integration-status--${integration.implementationStatus}`}>
          {integration.implementationStatus === "planned" ? "Planned" : "Implemented"}
        </span>
      </header>

      <p className="integration-card__description">{integration.description}</p>

      <section className="integration-card__section" aria-label={`${integration.name} capability goals`}>
        <h3>Capability goals</h3>
        <ul className="integration-card__capabilities">
          {capabilities.map((capability) => (
            <li key={capability.id} className="integration-capability">
              <span className="integration-capability__name">{capability.name}</span>
              <span className="integration-capability__status">
                {capability.status === "planned" ? "Planned" : "Available"}
              </span>
              <p>{capability.description}</p>
            </li>
          ))}
        </ul>
      </section>

      <dl className="integration-card__facts">
        <div>
          <dt>Compatibility</dt>
          <dd>
            {integration.compatibility.platform}; {integration.compatibility.versionRange ?? "version range not validated"}.
            {integration.compatibility.note}
          </dd>
        </div>
        <div>
          <dt>Installation</dt>
          <dd aria-live="polite">{installationLabel(integration.installationStatus)}</dd>
        </div>
        <div>
          <dt>Connection</dt>
          <dd>{connectionLabel(integration.connectionStatus)}</dd>
        </div>
        <div>
          <dt>Task readiness</dt>
          <dd>{taskReadinessLabel(integration.taskReadiness)}</dd>
        </div>
      </dl>

      {onCheckInstallation ? (
        <div className="integration-card__actions">
          <button
            type="button"
            className="button button--ghost button--sm"
            onClick={onCheckInstallation}
            disabled={isCheckingInstallation}
          >
            {isCheckingInstallation ? "Checking…" : "Check Installation"}
          </button>
          <p>Runs a bounded search of supported Windows locations only when selected. It does not launch Blender.</p>
        </div>
      ) : null}

      {integration.setupRequirements.length > 0 ? (
        <section className="integration-card__section integration-card__setup">
          <h3>{hasConnector ? "Setup" : "What is needed"}</h3>
          <ul>
            {integration.setupRequirements.map((requirement) => (
              <li key={requirement}>{requirement}</li>
            ))}
          </ul>
        </section>
      ) : null}

      {integration.limitations.length > 0 ? (
        <details className="integration-card__limitations">
          <summary>Limitations</summary>
          <ul>
            {integration.limitations.map((limitation) => (
              <li key={limitation}>{limitation}</li>
            ))}
          </ul>
        </details>
      ) : null}

      {!hasConnector || integration.connectionActions.length === 0 ? (
        <p className="integration-card__unavailable" role="note">
          Connection controls are unavailable. {integration.id === "blender"
            ? "Installation detection reports candidates only; no validated connection handshake or control path exists."
            : "This catalogue entry does not detect, connect to, or control the application."}
        </p>
      ) : null}
    </article>
  );
}

export function AppIntegrationsPage() {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<(typeof INTEGRATION_CATEGORIES)[number]["id"]>("all");
  const [blenderInstallationStatus, setBlenderInstallationStatus] = useState<InstallationStatus>({
    state: "unknown",
  });
  const detectionRequestId = useRef(0);

  useEffect(() => () => {
    detectionRequestId.current += 1;
  }, []);

  const handleCheckInstallation = () => {
    const requestId = ++detectionRequestId.current;
    setBlenderInstallationStatus({ state: "checking" });
    void checkBlenderInstallation().then((result) => {
      if (requestId === detectionRequestId.current) {
        setBlenderInstallationStatus(result);
      }
    });
  };

  const integrations = useMemo(
    () => APP_INTEGRATIONS.map((integration) =>
      integration.id === "blender"
        ? { ...integration, installationStatus: blenderInstallationStatus }
        : integration,
    ),
    [blenderInstallationStatus],
  );
  const matches = useMemo(
    () => filterAppIntegrations(integrations, { query, category }),
    [integrations, query, category],
  );

  return (
    <div className="page page--integrations" data-testid="app-integrations-page">
      <PageHeader
        eyebrow="Application catalogue"
        title="App Integrations"
        copy="Discover apps being considered for Nexora. When a verified connector is available, setup and connection controls will live here; AI tasks will remain in your existing chat."
      />

      <div className="integrations-catalogue-note" role="status">
        <Icon name="apps" size={16} />
        <p>
          Catalogue only. Blender installation detection runs only after you select Check Installation;
          it does not connect to or control an app. Planned entries are not supported integrations.
        </p>
      </div>

      <section className="integration-controls" aria-label="Filter application catalogue">
        <label className="integration-search">
          <Icon name="search" size={16} />
          <span className="sr-only">Search apps or capabilities</span>
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            placeholder="Search apps or capabilities"
            aria-label="Search apps or capabilities"
          />
        </label>
        <label className="integration-category-filter">
          <span>Category</span>
          <select
            value={category}
            onChange={(event) => setCategory(event.currentTarget.value as typeof category)}
            aria-label="Filter by category"
          >
            {INTEGRATION_CATEGORIES.map((option) => (
              <option key={option.id} value={option.id}>{option.label}</option>
            ))}
          </select>
        </label>
        <p className="integration-results" aria-live="polite">
          {matches.length} {matches.length === 1 ? "candidate" : "candidates"}
        </p>
      </section>

      {matches.length > 0 ? (
        <ul className="integration-grid" aria-label="Application catalogue">
          {matches.map((integration) => (
            <li key={integration.id}>
              <IntegrationCard
                integration={integration}
                onCheckInstallation={integration.id === "blender" ? handleCheckInstallation : undefined}
                isCheckingInstallation={blenderInstallationStatus.state === "checking"}
              />
            </li>
          ))}
        </ul>
      ) : (
        <section className="integration-empty" role="status" aria-label="No matching applications">
          <Icon name="search" size={20} />
          <h2>No matching applications</h2>
          <p>Try a different app name, capability, or category.</p>
          <button
            type="button"
            className="button button--ghost button--sm"
            onClick={() => {
              setQuery("");
              setCategory("all");
            }}
          >
            Clear filters
          </button>
        </section>
      )}

      <aside className="integration-help" aria-labelledby="integration-help-title">
        <div className="integration-help__icon" aria-hidden="true">
          <Icon name="message" size={17} />
        </div>
        <div>
          <h2 id="integration-help-title">Integration help</h2>
          <p>
            Setup guidance and troubleshooting will appear here when an integration has a verified
            connector. For now, no connection or support workflow is available from these entries.
          </p>
        </div>
      </aside>
    </div>
  );
}
