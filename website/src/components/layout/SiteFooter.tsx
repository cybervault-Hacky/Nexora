import { Link } from "react-router-dom";

import ApertureMark from "../brand/ApertureMark";
import { Container } from "../ui";
import { ROUTES } from "../../config/routes";

import "./SiteFooter.css";

/**
 * Site footer.
 *
 * Deliberately not a second navigation: the header already lists every page, so
 * repeating it here would only add noise. What remains is identity, one honest
 * sentence about the product, and — below a hairline rule — the two pages a
 * reader looks for at the bottom of a site (privacy and security).
 *
 * No company details, social accounts, contact addresses, release claims,
 * development-status paragraph, or copyright line, because none of those have
 * been established.
 */
export default function SiteFooter() {
  return (
    <footer className="site-footer">
      <Container>
        <div className="site-footer__brand">
          <Link className="site-footer__mark-row" to={ROUTES.home}>
            <ApertureMark size={20} className="site-footer__mark" />
            <span className="site-footer__wordmark">Nexora</span>
          </Link>
          <p className="site-footer__description">
            A Windows desktop AI coding assistant built around local models.
          </p>
        </div>

        <nav className="site-footer__legal" aria-label="Privacy and security">
          <Link className="site-footer__link" to={ROUTES.privacy}>
            Privacy
          </Link>
          <Link className="site-footer__link" to={ROUTES.security}>
            Security
          </Link>
        </nav>
      </Container>
    </footer>
  );
}
