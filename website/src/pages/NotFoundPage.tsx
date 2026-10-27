import { Button, Container, Eyebrow, Section } from "../components/ui";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";

import "./NotFoundPage.css";

/**
 * Fallback route.
 *
 * This is a client-side site: the server cannot return a 404 status for an
 * unknown path, so this page renders instead of silently showing the home
 * page. A real 404 status code has to be configured on the host.
 *
 * Phase 14: the route is also marked `noindex, follow` in the document head, so
 * a crawler that renders JavaScript does not treat an arbitrary mistyped URL as
 * a product page. That directive is guidance, not access control, and it cannot
 * replace a real 404 status.
 */
export default function NotFoundPage() {
  // `null` selects the not-found metadata entry, which is never indexable.
  usePageMetadata(null);

  return (
    <Section>
      <Container>
        <div className="not-found">
          <Eyebrow>Error 404</Eyebrow>
          <h1 className="not-found__title">This page doesn&rsquo;t exist.</h1>
          <p className="not-found__lead">
            The address you requested isn&rsquo;t part of this website. It may
            have been mistyped, or the page may not have been built yet.
          </p>
          <p className="not-found__actions">
            <Button to={ROUTES.home}>Back to home</Button>
          </p>
        </div>
      </Container>
    </Section>
  );
}
