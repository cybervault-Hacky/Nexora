import { ApertureMark } from "../components/ApertureMark";
import { Icon } from "../components/Icon";

interface OnboardingPageProps {
  /** Returns to the welcome screen. */
  onBack: () => void;
  /** Honest way forward into the existing workspace (no account involved). */
  onEnterWorkspace: () => void;
  /** Continues to the Phase 21 sign-in interface. */
  onSignIn: () => void;
}

/**
 * The onboarding "next step" screen between Welcome and Sign In. Real account
 * authentication does not exist in this build, and this screen says so; the
 * sign-in interface itself is the next step.
 */
export function OnboardingPage({ onBack, onEnterWorkspace, onSignIn }: OnboardingPageProps) {
  return (
    <div className="onboarding">
      <div className="onboarding__card">
        <div className="welcome__brand">
          <span className="welcome__mark" aria-hidden="true">
            <ApertureMark size={22} />
          </span>
          <span className="welcome__wordmark">nexora</span>
        </div>

        <ol className="onboarding__steps" aria-label="Onboarding progress">
          <li className="onboarding__step onboarding__step--done">Welcome</li>
          <li className="onboarding__step onboarding__step--current" aria-current="step">
            Next step
          </li>
        </ol>

        <h1 className="onboarding__title">Sign-in is the next step.</h1>

        <p className="onboarding__lead">
          Account sign-in is planned for a later release step and is not part of
          this build — there is no account to create here yet.
        </p>

        <div className="onboarding__slot" role="note" aria-label="Upcoming sign-in">
          <Icon name="shield" size={16} />
          <div>
            <strong>The sign-in screen is ready for its backend.</strong>
            <p>
              You can walk through it next. It is interface-only for now — Nexora
              still has no authentication, accounts, or sessions of any kind,
              and the workspace works without one.
            </p>
          </div>
        </div>

        <div className="onboarding__actions">
          <button type="button" className="button button--ghost" onClick={onBack}>
            Back to Welcome
          </button>
          <button type="button" className="button button--ghost" onClick={onEnterWorkspace}>
            Open the workspace
          </button>
          <button type="button" className="button button--primary" onClick={onSignIn}>
            Continue to Sign in
          </button>
        </div>
      </div>
    </div>
  );
}
