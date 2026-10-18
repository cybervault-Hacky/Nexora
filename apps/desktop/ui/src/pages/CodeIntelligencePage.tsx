import { AnalysisPanel } from "../components/AnalysisPanel";
import { CodeIntelligencePanel } from "../components/CodeIntelligencePanel";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import type { SymbolIndex } from "../lib/symbolIndex";
import { PageHeader } from "./PageHeader";

interface CodeIntelligencePageProps {
  workspacePath: string | null;
  onPreview: (path: string) => void;
  symbolIndex: SymbolIndex | null;
  onIndexChange: (index: SymbolIndex | null) => void;
  /** File used as the analysis/impact target: selection, preview, or first context file. */
  analysisTargetPath: string | null;
}

/**
 * The Code Intelligence page: indexed symbol search and the dependency /
 * relationship / impact analysis. All data comes from the bounded local
 * index and graph — nothing is invented or fetched from a network.
 */
export function CodeIntelligencePage({
  workspacePath,
  onPreview,
  symbolIndex,
  onIndexChange,
  analysisTargetPath,
}: CodeIntelligencePageProps) {
  return (
    <div className="page page--code">
      <PageHeader
        eyebrow="Local index"
        title="Code intelligence"
        copy="Search symbols indexed from the open project and inspect verified import relationships. The index is bounded and read-only — unsupported languages are reported honestly, never guessed."
      />

      {symbolIndex ? (
        <p className="page-note" role="status">
          <Icon name="shield" size={12} />
          Index: {symbolIndex.fileCount} files · {symbolIndex.symbols.length} symbols
          {symbolIndex.truncated ? " · truncated at limits" : ""}
          {symbolIndex.unsupported > 0 ? ` · ${symbolIndex.unsupported} unsupported` : ""}
          {symbolIndex.parseErrors > 0 ? ` · ${symbolIndex.parseErrors} parse errors` : ""}
        </p>
      ) : null}

      <div className="page-stack">
        <CodeIntelligencePanel
          workspacePath={workspacePath}
          onPreview={onPreview}
          onIndexChange={onIndexChange}
        />
        <AnalysisPanel
          workspacePath={workspacePath}
          targetPath={analysisTargetPath}
          onPreview={onPreview}
        />
      </div>

      {!workspacePath ? (
        <p className="page-note" role="status">
          No project is open yet.{" "}
          <RouterLink to="projects">Open one on the Projects page</RouterLink> to
          build the symbol index and dependency graph.
        </p>
      ) : null}
    </div>
  );
}
