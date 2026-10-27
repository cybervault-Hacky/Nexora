import ParticleField from "../components/sections/ParticleField";
import { HeroSection } from "../components/sections";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";

import "./HomePage.css";

/**
 * Home page.
 *
 * A single hero and nothing else. The product's detail lives on its own pages —
 * the home page states what Nexora is, the state of the release, and then stops.
 * The lower "meet Nexora" block that previously sat beneath the hero (a second
 * headline, a repeated description, a comparison list, and a workflow diagram)
 * has been removed rather than hidden.
 *
 * Phase 11 adds the particle background: a decorative layer behind the hero,
 * confined to this page and to this wrapper. It is `aria-hidden`, ignores
 * pointer events, and renders nothing at all when reduced motion is requested.
 */
export default function HomePage() {
  usePageMetadata(ROUTES.home);

  return (
    <div className="home">
      <ParticleField />
      <HeroSection />
    </div>
  );
}
