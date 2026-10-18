import { ApertureMark } from "../components/ApertureMark";
import { Icon } from "../components/Icon";

interface VerifyEmailPageProps {
  /** Returns to the sign-in page. */
  onBack: () => void;
}

/**
 * The verification-pending experience for the future authentication flow.
 *
 * It states plainly what will be required once account creation is connected
 * and that nothing is functional today: no email is sent, there is no code to
 * enter, no token to redeem, and nothing to resend.
 */
export function VerifyEmailPage({ onBack }: VerifyEmailPageProps) {
  return (
    <div className="onboarding">
      <div className="onboarding__card">
        <div className="welcome__brand">
          <span className="welcome__mark" aria-hidden="true">
            <ApertureMark size={22} />
          </span>
          <span className="welcome__wordmark">nexora</span>
        </div>

        <h1 className="onboarding__title">Email verification is next.</h1>

        <p className="onboarding__lead">
          Once account creation is connected, new accounts will need to verify
          an email address before signing in. That step will appear here.
        </p>

        <div className="onboarding__slot" role="note" aria-label="Verification status">
          <Icon name="shield" size={16} />
          <div>
            <strong>No email has been sent.</strong>
            <p>
              Email verification is not connected in this build: there is no
              code to enter, no token to redeem, and nothing to resend. Nothing
              on this screen performs a real verification.
            </p>
          </div>
        </div>

        <div className="onboarding__actions">
          <button type="button" className="button button--ghost" onClick={onBack}>
            Back to Sign In
          </button>
        </div>
      </div>
    </div>
  );
}
