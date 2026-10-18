import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";

interface NotFoundPageProps {
  /** The path the user asked for, shown verbatim — never silently swallowed. */
  path: string;
}

/** Graceful fallback for unknown routes: honest, and always a way back. */
export function NotFoundPage({ path }: NotFoundPageProps) {
  return (
    <div className="page page--not-found">
      <div className="empty">
        <div className="empty__mark" aria-hidden="true">
          <Icon name="alert" size={28} />
        </div>
        <h1 className="empty__title">Page not found.</h1>
        <p className="empty__copy">
          Nexora has no page at{" "}
          <code className="not-found__path">{path || "(empty)"}</code>. The link
          may be out of date.
        </p>
        <RouterLink to="home" className="button button--primary empty__cta">
          <Icon name="home" size={14} />
          Return to Home
        </RouterLink>
      </div>
    </div>
  );
}
