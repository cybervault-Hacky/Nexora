import { ApertureMark } from "../components/ApertureMark";
import { Icon } from "../components/Icon";
import { WelcomePreview } from "../components/WelcomePreview";

interface WelcomePageProps {
  /** Deliberate advance into the onboarding flow (Phase 21's sign-in step). */
  onContinue: () => void;
}

const CAPABILITIES = [
  {
    icon: "message" as const,
    title: "AI Chat",
    copy: "Stream responses from a model loaded on this machine.",
  },
  {
    icon: "folder" as const,
    title: "Projects & patches",
    copy: "Read-only browsing, with review-before-apply for every change.",
  },
  {
    icon: "code" as const,
    title: "Code intelligence",
    copy: "A bounded symbol index and verified import graph over your files.",
  },
];

/**
 * The welcome screen: the entry of the onboarding flow, outside the workspace
 * shell. Copy stays concrete and verifiable; the preview beside it is purely
 * decorative and holds no controls.
 */
export function WelcomePage({ onContinue }: WelcomePageProps) {
  return (
    <div className="welcome">
      <section className="welcome__intro">
        <div className="welcome__brand">
          <span className="welcome__mark" aria-hidden="true">
            <ApertureMark size={26} />
          </span>
          <span className="welcome__wordmark">nexora</span>
        </div>

        <h1 className="welcome__title">Code with an AI that runs on your machine.</h1>

        <p className="welcome__lead">
          Nexora is a desktop coding studio: a conversational workspace, read-only
          code intelligence, and a review-before-apply patch flow — powered by a
          llama.cpp runtime and GGUF model you already have.
        </p>

        <div className="welcome__actions">
          <button type="button" className="button button--primary welcome__continue" onClick={onContinue}>
            Continue
            <Icon name="arrow-up" size={14} className="welcome__continue-arrow" />
          </button>
        </div>

        <ul className="welcome__capabilities" aria-label="What Nexora includes">
          {CAPABILITIES.map((item) => (
            <li key={item.title} className="welcome__capability">
              <Icon name={item.icon} size={14} />
              <span>
                <strong>{item.title}</strong>
                <span className="welcome__capability-copy">{item.copy}</span>
              </span>
            </li>
          ))}
        </ul>

        <p className="welcome__note">
          Local-first by design: inference runs on this PC through a llama.cpp
          runtime and a GGUF model you supply, and prompts are processed locally —
          Nexora does not send them to a cloud service.
        </p>
      </section>

      {/* Purely decorative composition — see WelcomePreview. */}
      <div className="welcome__preview" aria-hidden="true">
        <WelcomePreview />
      </div>
    </div>
  );
}
