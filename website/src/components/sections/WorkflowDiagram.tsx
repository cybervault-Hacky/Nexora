import ApertureMark from "../brand/ApertureMark";

import "./WorkflowDiagram.css";

/**
 * Conceptual left-to-right workflow diagram: application → model file →
 * local runtime.
 *
 * Deliberately different from the home page's machine-boundary diagram, while
 * sharing its visual language (hairlines, a dashed accent connector, small
 * labels). Like that diagram this is **not** a screenshot and **not** a
 * simulation of the application — no window chrome, menus, or code is drawn.
 *
 * The ordered list on the page already carries the full explanation, so the
 * graphic itself is decorative and hidden from assistive technology; the
 * visible caption states what it is.
 */
export default function WorkflowDiagram() {
  return (
    <figure className="flow">
      <div className="flow__canvas" aria-hidden="true">
        <div className="flow__stage">
          <span className="flow__mark">
            <ApertureMark size={20} />
          </span>
          <span className="flow__label">Windows application</span>
          <span className="flow__meta">Nexora desktop</span>
        </div>

        <div className="flow__arrow" />

        <div className="flow__stage">
          <span className="flow__label">Local model</span>
          <span className="flow__meta">Downloaded, digest-checked</span>
        </div>

        <div className="flow__arrow" />

        <div className="flow__stage flow__stage--accent">
          <span className="flow__label">Local inference</span>
          <span className="flow__meta">127.0.0.1 only</span>
        </div>
      </div>

      <figcaption className="flow__caption">
        Conceptual workflow — not a screenshot or evidence of a verified Windows
        runtime.
      </figcaption>
    </figure>
  );
}
