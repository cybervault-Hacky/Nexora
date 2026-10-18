import { Icon } from "./Icon";
import type { ContextPlan } from "../features/workspace/contextBudget";
import { formatBudgetLabel } from "../features/workspace/contextBudget";

interface Props {
  plan: ContextPlan | null;
  loading?: boolean;
}

export function ContextBudgetBar({ plan, loading }: Props) {
  if (!plan) {
    return (
      <div className="budget-bar budget-bar--empty" role="status">
        <span className="budget-bar__label">No context selected</span>
      </div>
    );
  }

  const pct = plan.availableTokens > 0 ? Math.min(100, Math.round((plan.usedTokens / plan.availableTokens) * 100)) : 0;
  const tone = pct > 90 ? "budget-bar__fill--danger" : pct > 70 ? "budget-bar__fill--warning" : "budget-bar__fill--ok";
  const included = plan.filePlans.filter((f) => f.included);
  const omitted = plan.filePlans.filter((f) => !f.included);

  return (
    <div className="budget-bar" role="status" aria-label="Context budget">
      <div className="budget-bar__header">
        <span className="budget-bar__title">
          <Icon name={plan.estimated ? "alert" : "check"} size={12} />
          {formatBudgetLabel(plan)}
        </span>
        <span className="budget-bar__mode" title={plan.estimated ? "Estimated tokens — conservative ~4 chars/token, not exact" : "Measured tokens from active model tokenizer"}>
          {plan.estimated ? "Estimated" : "Measured"}
        </span>
      </div>

      <div className="budget-bar__track" aria-hidden="true">
        <div className={`budget-bar__fill ${tone}`} style={{ width: `${pct}%` }} />
      </div>

      <div className="budget-bar__details">
        <span>
          {included.length} file{included.length !== 1 ? "s" : ""} included
          {omitted.length > 0 ? `, ${omitted.length} omitted` : ""}
          {plan.historyOmitted > 0 ? `, ${plan.historyOmitted} older turn(s) omitted` : ""}
        </span>
        {loading ? <span className="budget-bar__loading">Updating…</span> : null}
      </div>

      {included.length > 0 ? (
        <ul className="budget-bar__included" aria-label="Included files">
          {included.map((f) => (
            <li key={f.path} className="budget-bar__file budget-bar__file--included" title={`${f.tokens} tokens`}>
              <Icon name="file" size={11} />
              <span>{f.path}</span>
              <span className="budget-bar__tokens">{f.tokens} tok</span>
            </li>
          ))}
        </ul>
      ) : null}

      {omitted.length > 0 ? (
        <details className="budget-bar__omitted">
          <summary>Omitted files ({omitted.length})</summary>
          <ul>
            {omitted.map((f) => (
              <li key={f.path} className="budget-bar__file budget-bar__file--omitted">
                <span>{f.path}</span>
                <span className="budget-bar__reason">{f.omittedReason ?? "Budget exceeded"}</span>
                <span className="budget-bar__tokens">{f.tokens} tok</span>
              </li>
            ))}
          </ul>
        </details>
      ) : null}

      {plan.warnings.length > 0 ? (
        <ul className="budget-bar__warnings" role="note">
          {plan.warnings.map((w, i) => (
            <li key={i} className="budget-bar__warning">
              <Icon name="alert" size={11} />
              <span>{w}</span>
            </li>
          ))}
        </ul>
      ) : null}

      <p className="budget-bar__note">
        Only user-selected, eligible files are counted. Per-file 256 KiB and {plan.budgetTokens} token budget enforced. File boundaries and fences are preserved — files are included fully or omitted entirely.
      </p>
    </div>
  );
}
