import { ApertureMark } from "./ApertureMark";

/**
 * A restrained, purely decorative composition of the Nexora workspace —
 * abstract chrome, message, and code shapes drawn with the same tokens as the
 * real UI. It contains no interactive controls and invents no product data:
 * every shape is a labelled visual placeholder, hidden from assistive tech.
 */
export function WelcomePreview() {
  return (
    <div className="welcome-preview" aria-hidden="true" role="presentation">
      <div className="welcome-preview__frame">
        <div className="welcome-preview__titlebar">
          <ApertureMark size={12} />
          <span className="welcome-preview__wordmark">nexora</span>
          <span className="welcome-preview__title-rule" />
          <span className="welcome-preview__title-chip" />
        </div>

        <div className="welcome-preview__body">
          <div className="welcome-preview__rail">
            <span className="welcome-preview__rail-mark">
              <ApertureMark size={14} />
            </span>
            <span className="welcome-preview__rail-item welcome-preview__rail-item--active" />
            <span className="welcome-preview__rail-item" />
            <span className="welcome-preview__rail-item" />
            <span className="welcome-preview__rail-item" />
            <span className="welcome-preview__rail-foot" />
          </div>

          <div className="welcome-preview__main">
            <div className="welcome-preview__bubble welcome-preview__bubble--user">
              <span className="welcome-preview__line" style={{ width: "72%" }} />
            </div>
            <div className="welcome-preview__bubble welcome-preview__bubble--ai">
              <span className="welcome-preview__line" style={{ width: "94%" }} />
              <span className="welcome-preview__line" style={{ width: "86%" }} />
              <span className="welcome-preview__line" style={{ width: "58%" }} />
            </div>
            <div className="welcome-preview__code">
              <span className="welcome-preview__line welcome-preview__line--accent" style={{ width: "64%" }} />
              <span className="welcome-preview__line" style={{ width: "82%" }} />
              <span className="welcome-preview__line" style={{ width: "46%" }} />
              <span className="welcome-preview__line" style={{ width: "74%" }} />
            </div>
          </div>
        </div>

        <div className="welcome-preview__composer">
          <span className="welcome-preview__line" style={{ width: "34%" }} />
          <span className="welcome-preview__send" />
        </div>
      </div>
    </div>
  );
}
