import { useState } from "react";
import { AnalysisPanel } from "../components/AnalysisPanel";
import { CodeIntelligencePanel } from "../components/CodeIntelligencePanel";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import type { SymbolIndex } from "../lib/symbolIndex";

interface CodeIntelligencePageProps {
  workspacePath: string | null;
  workspaceName?: string | null;
  onPreview: (path: string) => void;
  symbolIndex: SymbolIndex | null;
  onIndexChange: (index: SymbolIndex | null) => void;
  /** File used as the analysis/impact target: selection, preview, or first context file. */
  analysisTargetPath: string | null;
  onOpenWorkspace?: () => void;
}

type IntelligenceView = "all" | "symbols" | "analysis";

/**
 * The Code Intelligence page: indexed symbol search and the dependency /
 * relationship / impact analysis. All data comes from the bounded local
 * index and graph — nothing is invented or fetched from a network.
 */
export function CodeIntelligencePage({
  workspacePath,
  workspaceName,
  onPreview,
  symbolIndex,
  onIndexChange,
  analysisTargetPath,
  onOpenWorkspace,
}: CodeIntelligencePageProps) {
  const [activeView, setActiveView] = useState<IntelligenceView>("all");

  return (
    <div className="page page--code">
      <header className="page-header page-header--compact">
        <div>
          <p className="eyebrow">Local index</p>
          <h1 className="page-header__title">
            {workspaceName ? `Code intelligence · ${workspaceName}` : "Code intelligence"}
          </h1>
          <p className="page-header__copy">
            {workspacePath
              ? workspacePath
              : "Search symbols indexed from the open project and inspect verified import relationships. The index is bounded and read-only — unsupported languages are reported honestly, never guessed."}
          </p>
        </div>
        {workspacePath && onOpenWorkspace ? (
          <div className="page-header__actions">
            <button
              type="button"
              className="button button--ghost button--sm"
              onClick={onOpenWorkspace}
              title="Open a different project directory"
            >
              <Icon name="folder" size={13} /> Switch project
            </button>
          </div>
        ) : null}
      </header>

      {!workspacePath ? (
        <div className="empty" aria-label="No project open">
          <div className="empty__mark" aria-hidden="true">
            <Icon name="search" size={36} />
          </div>
          <h2 className="empty__title">No project open</h2>
          <p className="empty__copy">
            Open a project directory to index source symbols, map verified import relationships, and inspect dependency impact. Code intelligence is computed entirely on your machine without external network transmission.
          </p>
          <div className="empty__actions">
            {onOpenWorkspace ? (
              <button
                type="button"
                className="button button--primary"
                onClick={onOpenWorkspace}
              >
                <Icon name="folder" size={14} />
                Open project folder
              </button>
            ) : (
              <RouterLink to="projects" className="button button--primary">
                <Icon name="folder" size={14} />
                Go to Projects
              </RouterLink>
            )}
          </div>
          <p className="empty__note">
            Bounded and read-only: files are analyzed with local parser rules and never executed.
          </p>
        </div>
      ) : (
        <>
          {symbolIndex ? (
            <div className="code-intel-summary" role="status" aria-label="Index summary">
              <div className="code-intel-summary__item">
                <span className="code-intel-summary__value">{symbolIndex.fileCount}</span>
                <span className="code-intel-summary__label">Indexed files</span>
              </div>
              <div className="code-intel-summary__item">
                <span className="code-intel-summary__value">{symbolIndex.symbols.length}</span>
                <span className="code-intel-summary__label">Symbols</span>
              </div>
              {symbolIndex.unsupported > 0 ? (
                <div className="code-intel-summary__item code-intel-summary__item--muted">
                  <span className="code-intel-summary__value">{symbolIndex.unsupported}</span>
                  <span className="code-intel-summary__label">Unsupported</span>
                </div>
              ) : null}
              {symbolIndex.parseErrors > 0 ? (
                <div className="code-intel-summary__item code-intel-summary__item--warn">
                  <span className="code-intel-summary__value">{symbolIndex.parseErrors}</span>
                  <span className="code-intel-summary__label">Parse errors</span>
                </div>
              ) : null}
              {symbolIndex.truncated ? (
                <div className="code-intel-summary__item code-intel-summary__item--warn">
                  <span className="code-intel-summary__value">Limit hit</span>
                  <span className="code-intel-summary__label">Truncated</span>
                </div>
              ) : null}
              <div className="code-intel-summary__meta">
                <span>Generated {new Date(symbolIndex.generatedAt).toLocaleTimeString()}</span>
                <span className="code-intel-summary__sub">Supported: Rust, TS/JS, Python</span>
              </div>
            </div>
          ) : null}

          <div className="code-intel-tabs" role="tablist" aria-label="Code intelligence views">
            <button
              type="button"
              role="tab"
              aria-selected={activeView === "all"}
              className={`code-intel-tabs__btn ${activeView === "all" ? "code-intel-tabs__btn--active" : ""}`}
              onClick={() => setActiveView("all")}
            >
              <Icon name="code" size={13} /> All views
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeView === "symbols"}
              className={`code-intel-tabs__btn ${activeView === "symbols" ? "code-intel-tabs__btn--active" : ""}`}
              onClick={() => setActiveView("symbols")}
            >
              <Icon name="search" size={13} /> Symbol index & search
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeView === "analysis"}
              className={`code-intel-tabs__btn ${activeView === "analysis" ? "code-intel-tabs__btn--active" : ""}`}
              onClick={() => setActiveView("analysis")}
            >
              <Icon name="branch" size={13} /> Dependency & impact analysis
            </button>
          </div>

          <div className="page-stack">
            {activeView === "all" || activeView === "symbols" ? (
              <CodeIntelligencePanel
                workspacePath={workspacePath}
                onPreview={onPreview}
                onIndexChange={onIndexChange}
              />
            ) : null}
            {activeView === "all" || activeView === "analysis" ? (
              <AnalysisPanel
                workspacePath={workspacePath}
                targetPath={analysisTargetPath}
                onPreview={onPreview}
              />
            ) : null}
          </div>
        </>
      )}
    </div>
  );
}
