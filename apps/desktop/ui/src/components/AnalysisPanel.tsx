import { useCallback, useEffect, useState } from "react";
import { Icon } from "./Icon";
import { useAnalysis, useRelationships } from "../features/workspace/useAnalysis";
import { useImpact } from "../features/workspace/useImpact";

interface Props {
  workspacePath: string | null;
  targetPath: string | null; // selected file or preview
  onPreview: (path: string) => void;
}

export function AnalysisPanel({ workspacePath, targetPath, onPreview }: Props) {
  const { analysisState, refresh, cancel: cancelGraph } = useAnalysis(workspacePath);
  const { relState, load: loadRel, cancel: cancelRel } = useRelationships(workspacePath);
  const { impactState, analyze, cancel: cancelImpact } = useImpact(workspacePath);
  const [activeTab, setActiveTab] = useState<"graph" | "file" | "impact">("graph");

  // Auto-load graph when workspace ready
  useEffect(() => {
    if (workspacePath && analysisState.status === "idle") {
      void refresh();
    }
  }, [workspacePath, analysisState.status, refresh]);

  // Auto-load relationships & impact when target changes and graph loaded
  useEffect(() => {
    if (!workspacePath || !targetPath) return;
    if (analysisState.status !== "loaded") return;
    void loadRel(targetPath);
    void analyze(targetPath);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [targetPath, workspacePath, analysisState.status]);

  // Reset tab when workspace changes
  useEffect(() => {
    setActiveTab("graph");
  }, [workspacePath]);

  const handleRefresh = useCallback(() => {
    void refresh();
    if (targetPath) {
      void loadRel(targetPath);
      void analyze(targetPath);
    }
  }, [refresh, loadRel, analyze, targetPath]);

  if (!workspacePath) {
    return (
      <div className="analysis" role="status" aria-label="Deep analysis">
        <p className="analysis__hint">Open a project to see deep codebase analysis.</p>
      </div>
    );
  }

  const isLoadingGraph = analysisState.status === "loading";
  const graphError = analysisState.status === "error" ? analysisState.message : null;

  return (
    <div className="analysis" role="region" aria-label="Deep analysis">
      <header className="analysis__header">
        <div className="analysis__title">
          <Icon name="branch" size={14} />
          <span>Deep Analysis</span>
          {analysisState.status === "loaded" ? (
            <span className="analysis__badge" title="Graph built">
              {analysisState.graph.nodes.length} files · {analysisState.graph.edges.length} edges
            </span>
          ) : null}
        </div>
        <div className="analysis__actions">
          <button
            type="button"
            className="button button--ghost button--sm"
            onClick={handleRefresh}
            disabled={isLoadingGraph}
            title="Rebuild graph"
            aria-label="Refresh analysis"
          >
            <Icon name="refresh" size={12} />
            {isLoadingGraph ? "Analyzing…" : "Refresh"}
          </button>
          {isLoadingGraph ? (
            <button type="button" className="button button--ghost button--sm" onClick={cancelGraph} aria-label="Cancel analysis">
              Cancel
            </button>
          ) : null}
        </div>
      </header>

      {isLoadingGraph ? (
        <p className="analysis__status" role="status">Building dependency graph… Bounded (max 500 files, 5 MiB, 2000 edges)…</p>
      ) : graphError ? (
        <div className="analysis__error" role="alert">
          <Icon name="alert" size={12} />
          <span>{graphError}</span>
          <button type="button" className="button button--ghost button--sm" onClick={handleRefresh}>
            Retry
          </button>
        </div>
      ) : analysisState.status === "loaded" ? (
        <>
          <div className="analysis__overview" role="status" aria-label="Graph overview">
            <div className="analysis__stats">
              <span>{analysisState.graph.nodes.length} files indexed</span>
              <span>{analysisState.graph.edges.length} verified+unresolved edges</span>
              <span className={analysisState.graph.cycles.length > 0 ? "analysis__warn" : ""}>
                {analysisState.graph.cycles.length} circular {analysisState.graph.cycles.length === 1 ? "dependency" : "dependencies"}
              </span>
              {analysisState.graph.truncated ? <span className="analysis__warn">truncated — limits hit</span> : null}
              {analysisState.graph.unsupported > 0 ? <span>{analysisState.graph.unsupported} unsupported skipped</span> : null}
            </div>
            {analysisState.graph.cycles.length > 0 ? (
              <div className="analysis__cycles" aria-label="Circular dependencies">
                <strong>Circular dependencies (potential):</strong>
                <ul>
                  {analysisState.graph.cycles.slice(0, 5).map((cycle, idx) => (
                    <li key={idx} className="analysis__cycle">
                      <span className="analysis__cycle-path">{cycle.join(" → ")}</span>
                      <span className="analysis__cycle-note"> — verified edges only; treat as potential</span>
                    </li>
                  ))}
                </ul>
                <p className="analysis__hint">Cycles detected via verified imports only. Unverified/inferred edges excluded.</p>
              </div>
            ) : (
              <p className="analysis__hint">No circular dependencies detected among verified imports (inferred edges excluded).</p>
            )}
            <p className="analysis__meta" title={analysisState.graph.root}>
              Last analyzed {new Date(analysisState.graph.generatedAt).toLocaleTimeString()}
            </p>
          </div>

          <div className="analysis__tabs" role="tablist" aria-label="Analysis sections">
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === "graph"}
              className={`analysis__tab ${activeTab === "graph" ? "analysis__tab--active" : ""}`}
              onClick={() => setActiveTab("graph")}
            >
              Graph
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === "file"}
              className={`analysis__tab ${activeTab === "file" ? "analysis__tab--active" : ""}`}
              onClick={() => setActiveTab("file")}
              disabled={!targetPath}
              title={!targetPath ? "Select a file to see relationships" : "File relationships"}
            >
              File
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === "impact"}
              className={`analysis__tab ${activeTab === "impact" ? "analysis__tab--active" : ""}`}
              onClick={() => setActiveTab("impact")}
              disabled={!targetPath}
              title={!targetPath ? "Select a file to see impact" : "Impact"}
            >
              Impact
            </button>
          </div>

          {activeTab === "graph" ? (
            <div className="analysis__section" aria-label="Dependency graph">
              {analysisState.graph.nodes.length === 0 ? (
                <p className="analysis__hint">No eligible files indexed. Check that project contains supported languages (Rust, TypeScript, JavaScript, Python).</p>
              ) : (
                <ul className="analysis__nodelist" role="list">
                  {analysisState.graph.nodes.slice(0, 12).map((node) => (
                    <li key={node.path} className="analysis__node" role="listitem">
                      <button type="button" className="analysis__node-button" onClick={() => onPreview(node.path)} title={node.path}>
                        <span className="analysis__node-path">{node.path}</span>
                        <span className="analysis__node-meta">
                          {node.imports.length} imports · {node.importedBy.length} dependents
                        </span>
                      </button>
                      {node.imports.length > 0 ? (
                        <ul className="analysis__edgelist">
                          {node.imports.slice(0, 3).map((edge, i) => (
                            <li key={i} className={`analysis__edge analysis__edge--${edge.verified ? "verified" : "unresolved"}`}>
                              <span className="analysis__edge-raw">{edge.raw}</span>
                              <span className="analysis__edge-to"> → {edge.to}</span>
                              <span className={`analysis__confidence analysis__confidence--${edge.verified ? "verified" : "inferred"}`}>
                                {edge.verified ? "verified" : "unresolved"}
                              </span>
                              <span className="analysis__edge-line">:{edge.line}</span>
                              {edge.note ? <span className="analysis__edge-note"> — {edge.note}</span> : null}
                            </li>
                          ))}
                          {node.imports.length > 3 ? <li className="analysis__hint">+{node.imports.length - 3} more imports</li> : null}
                        </ul>
                      ) : null}
                    </li>
                  ))}
                </ul>
              )}
              <p className="analysis__hint">Showing up to 12 files. Verified = resolved to existing file; unresolved = external or not found (inferred). Never invented paths.</p>
            </div>
          ) : null}

          {activeTab === "file" ? (
            <div className="analysis__section" aria-label="File relationships">
              {!targetPath ? (
                <p className="analysis__hint">Select a file in the explorer or preview to see its relationships.</p>
              ) : relState.status === "loading" ? (
                <p className="analysis__status" role="status">
                  Analyzing relationships for {targetPath}…
                  <button type="button" className="button button--ghost button--sm" onClick={cancelRel}>
                    Cancel
                  </button>
                </p>
              ) : relState.status === "error" ? (
                <div className="analysis__error" role="alert">
                  <span>{relState.message}</span>
                  <button type="button" className="button button--ghost button--sm" onClick={() => void loadRel(targetPath)}>
                    Retry
                  </button>
                </div>
              ) : relState.status === "loaded" ? (
                <div className="analysis__file">
                  <h4 className="analysis__subtitle">{relState.data.path}</h4>
                  <section aria-label="Imports">
                    <strong>Imports ({relState.data.imports.length}):</strong>
                    {relState.data.imports.length === 0 ? (
                      <p className="analysis__hint">No imports detected (unsupported language or no import statements).</p>
                    ) : (
                      <ul className="analysis__list">
                        {relState.data.imports.map((edge, i) => (
                          <li key={i} className="analysis__item">
                            <code className="analysis__code">{edge.raw}</code> → <code>{edge.to}</code>{" "}
                            <span className={`analysis__confidence analysis__confidence--${edge.verified ? "verified" : "inferred"}`}>
                              {edge.verified ? "verified" : "unresolved"}
                            </span>{" "}
                            <span className="analysis__line">:{edge.line}</span>
                            {edge.note ? <span className="analysis__note"> — {edge.note}</span> : null}
                            {edge.verified ? (
                              <button type="button" className="analysis__link" onClick={() => onPreview(edge.to)}>
                                open {edge.to}
                              </button>
                            ) : null}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                  <section aria-label="Imported by">
                    <strong>Imported by ({relState.data.importedBy.length}):</strong>
                    {relState.data.importedBy.length === 0 ? (
                      <p className="analysis__hint">No verified dependents found. Change appears isolated (for verified imports).</p>
                    ) : (
                      <ul className="analysis__list">
                        {relState.data.importedBy.map((p) => (
                          <li key={p} className="analysis__item">
                            <button type="button" className="analysis__link" onClick={() => onPreview(p)}>
                              {p}
                            </button>{" "}
                            <span className="analysis__confidence analysis__confidence--verified">verified</span>
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                  <section aria-label="Defined symbols">
                    <strong>Defined symbols ({relState.data.definedSymbols.length}):</strong>
                    {relState.data.definedSymbols.length === 0 ? (
                      <p className="analysis__hint">No symbols indexed — unsupported language or empty file.</p>
                    ) : (
                      <ul className="analysis__list">
                        {relState.data.definedSymbols.slice(0, 10).map((sym) => (
                          <li key={`${sym.path}:${sym.name}:${sym.startLine}`} className="analysis__item">
                            <code>{sym.name}</code> <span>({sym.kind})</span> <span>:{sym.startLine}</span>
                            {sym.detail ? <span className="analysis__detail"> — {sym.detail}</span> : null}
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                  <section aria-label="Referenced by">
                    <strong>Referenced by ({relState.data.referencedBy.length}):</strong>
                    {relState.data.referencedBy.length === 0 ? (
                      <p className="analysis__hint">No references found via name search (inferred, not verified usage).</p>
                    ) : (
                      <ul className="analysis__list">
                        {relState.data.referencedBy.slice(0, 10).map((ref, i) => (
                          <li key={i} className="analysis__item">
                            <code>{ref.symbol}</code> in <button type="button" className="analysis__link" onClick={() => onPreview(ref.path)}>{ref.path}:{ref.line}</button>{" "}
                            <span className="analysis__confidence analysis__confidence--inferred">inferred</span>
                            <span className="analysis__snippet"> — {ref.snippet}</span>
                          </li>
                        ))}
                      </ul>
                    )}
                    <p className="analysis__hint">Referenced-by is inferred via substring search for symbol names — may include coincidental matches, not proven call graph.</p>
                  </section>
                  {relState.data.unresolved.length > 0 ? (
                    <section aria-label="Unresolved">
                      <strong>Unresolved imports ({relState.data.unresolved.length}):</strong>
                      <ul className="analysis__list">
                        {relState.data.unresolved.map((edge, i) => (
                          <li key={i} className="analysis__item analysis__item--warn">
                            {edge.raw} at :{edge.line} — {edge.note ?? "unresolved"}
                          </li>
                        ))}
                      </ul>
                    </section>
                  ) : null}
                  {relState.data.notes.length > 0 ? (
                    <ul className="analysis__notes">
                      {relState.data.notes.map((n, i) => (
                        <li key={i} className="analysis__note">
                          {n}
                        </li>
                      ))}
                    </ul>
                  ) : null}
                </div>
              ) : (
                <p className="analysis__hint">Select a file to see verified imports, dependents, and evidence-grounded references.</p>
              )}
            </div>
          ) : null}

          {activeTab === "impact" ? (
            <div className="analysis__section" aria-label="Change impact">
              {!targetPath ? (
                <p className="analysis__hint">Select a file to see potential impact of changing it.</p>
              ) : impactState.status === "loading" ? (
                <p className="analysis__status" role="status">
                  Computing impact for {targetPath}… Bounded BFS depth {4}.
                  <button type="button" className="button button--ghost button--sm" onClick={cancelImpact}>
                    Cancel
                  </button>
                </p>
              ) : impactState.status === "error" ? (
                <div className="analysis__error" role="alert">
                  <span>{impactState.message}</span>
                  <button type="button" className="button button--ghost button--sm" onClick={() => void analyze(targetPath)}>
                    Retry
                  </button>
                </div>
              ) : impactState.status === "loaded" ? (
                <div className="analysis__impact">
                  <h4 className="analysis__subtitle">Impact for {impactState.report.target}</h4>
                  <section aria-label="Direct dependents">
                    <strong>Direct dependents ({impactState.report.direct.length}):</strong>
                    {impactState.report.direct.length === 0 ? (
                      <p className="analysis__hint">No direct dependents via verified imports — change appears isolated (potential risk: symbol references may still exist, see inferred).</p>
                    ) : (
                      <ul className="analysis__list">
                        {impactState.report.direct.map((node) => (
                          <li key={node.path} className="analysis__item">
                            <button type="button" className="analysis__link" onClick={() => onPreview(node.path)}>
                              {node.path}
                            </button>{" "}
                            <span>— {node.reason}</span>
                            <span className={`analysis__confidence analysis__confidence--${node.confidence}`}>{node.confidence}</span>
                            <span className="analysis__evidence"> — {node.evidence}</span>
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                  <section aria-label="Transitive dependents">
                    <strong>Transitive / inferred ({impactState.report.transitive.length}):</strong>
                    {impactState.report.transitive.length === 0 ? (
                      <p className="analysis__hint">No transitive dependents within depth {4}.</p>
                    ) : (
                      <ul className="analysis__list">
                        {impactState.report.transitive.map((node) => (
                          <li key={node.path} className="analysis__item">
                            <button type="button" className="analysis__link" onClick={() => onPreview(node.path)}>
                              {node.path}
                            </button>{" "}
                            <span>— {node.reason}</span>
                            <span className="analysis__confidence analysis__confidence--inferred">{node.confidence}</span>
                            <span className="analysis__evidence"> — {node.evidence}</span>
                            <span className="analysis__risk"> — potential risk, not guaranteed</span>
                          </li>
                        ))}
                      </ul>
                    )}
                    {impactState.report.truncated ? <p className="analysis__warn">Truncated — more dependents exist but omitted due to limits.</p> : null}
                  </section>
                  <section aria-label="Risks">
                    <strong>Risks:</strong>
                    <ul className="analysis__list">
                      {impactState.report.risks.map((r, i) => (
                        <li key={i} className="analysis__item analysis__item--risk">
                          {r}
                        </li>
                      ))}
                    </ul>
                  </section>
                  {impactState.report.notes.length > 0 ? (
                    <ul className="analysis__notes">
                      {impactState.report.notes.map((n, i) => (
                        <li key={i} className="analysis__note">
                          {n}
                        </li>
                      ))}
                    </ul>
                  ) : null}
                  <p className="analysis__hint">Static analysis does not prove runtime behavior. Verify with tests before applying.</p>
                </div>
              ) : (
                <p className="analysis__hint">Select a file to see which other files might be affected. Direct = verified import; transitive/inferred = potential risk.</p>
              )}
            </div>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
