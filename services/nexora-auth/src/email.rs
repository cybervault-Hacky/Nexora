//! Email-delivery abstraction (Phase 23C).
//!
//! The registration/verification workflow depends on a genuine transport
//! interface (`EmailProvider`), not on any particular vendor. The only
//! implementation shipped today is `UnconfiguredEmailProvider`, which reports
//! honestly that no mail is sent — the service must **never** claim an email
//! was delivered when it was not (and never fabricates verification links).
//!
//! A real transport (SMTP via `lettre`, or a provider API) plugs in later by
//! implementing `EmailProvider`; secrets for it belong in the deployment's
//! secret store (plan §7), never in the repository, logs, or chat.
//!
//! Security: raw verification tokens must appear only inside the email body
//! constructed by the caller. Providers must never log `EmailMessage` values —
//! the body contains the single-use token.

use std::future::Future;
use std::pin::Pin;

/// A fully-formed transactional email. The transport only delivers it.
#[derive(Clone, PartialEq, Eq)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// Redacting `Debug` (Phase 23G): bodies carry raw verification/reset tokens
/// in links (plan §4: "token values appear only in email links") — the body
/// must never reach logs, traces, or panic messages.
impl std::fmt::Debug for EmailMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailMessage")
            .field("to", &crate::scrub::REDACTED)
            .field("subject", &self.subject)
            .field("body", &crate::scrub::REDACTED)
            .finish()
    }
}

/// Delivery failure. `Display` is deliberately generic — it must never echo
/// addresses, credentials, message bodies, or tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailError {
    /// No provider is configured for this deployment. Nothing was sent.
    NotConfigured,
    /// The configured provider refused or failed. Nothing (or only part) was
    /// sent; details stay server-side and are not returned to clients.
    Transport,
}

impl std::fmt::Display for EmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmailError::NotConfigured => {
                write!(f, "Email delivery is not configured; no message was sent.")
            }
            EmailError::Transport => {
                write!(f, "The email provider could not deliver the message.")
            }
        }
    }
}

/// Object-safe transport interface (boxed futures — no `async-trait` dep).
pub trait EmailProvider: Send + Sync {
    /// Deliver one message. Errors never include the message body or token.
    fn send(
        &self,
        message: EmailMessage,
    ) -> Pin<Box<dyn Future<Output = Result<(), EmailError>> + Send + '_>>;
}

/// Honest default: no mail is ever sent. Callers surface this as a
/// not-configured delivery status — never as success.
pub struct UnconfiguredEmailProvider;

impl EmailProvider for UnconfiguredEmailProvider {
    fn send(
        &self,
        _message: EmailMessage,
    ) -> Pin<Box<dyn Future<Output = Result<(), EmailError>> + Send + '_>> {
        Box::pin(async { Err(EmailError::NotConfigured) })
    }
}

/// Capability state returned to clients (config-level; identical for every
/// account, so it cannot leak whether an account exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EmailDelivery {
    Configured,
    Unconfigured,
}

impl EmailDelivery {
    pub fn from_provider_configured(configured: bool) -> Self {
        if configured {
            EmailDelivery::Configured
        } else {
            EmailDelivery::Unconfigured
        }
    }
}

/// Log only transport-level failures. Provider error text is deliberately
/// generic; no recipient, message body, token, or credential is included.
pub(crate) fn report_delivery_failure(flow: &'static str, error: &EmailError) {
    if matches!(error, EmailError::NotConfigured) {
        return;
    }
    eprintln!(
        "{}",
        crate::scrub::scrub(&format!(
            "nexora-auth: {flow} email delivery failed ({error}); message details withheld."
        ))
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_provider_never_claims_delivery() {
        let provider = UnconfiguredEmailProvider;
        let message = EmailMessage {
            to: "someone@example.test".to_string(),
            subject: "Verify".to_string(),
            body: "contains a token we never log".to_string(),
        };
        let result = futures_executor_block_on(provider.send(message));
        assert_eq!(result, Err(EmailError::NotConfigured));
    }

    #[test]
    fn error_display_leaks_nothing() {
        let rendered = EmailError::NotConfigured.to_string();
        assert!(rendered.contains("not configured"));
        assert!(!rendered.contains('@'));
        let rendered = EmailError::Transport.to_string();
        assert!(!rendered.contains('@'));
        assert!(!rendered.to_lowercase().contains("token"));
    }

    /// Tiny blocking driver for the async trait without extra test deps.
    fn futures_executor_block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::sync::Arc;
        use std::task::{Context, Poll, Wake, Waker};

        struct ThreadWaker(std::thread::Thread);
        impl Wake for ThreadWaker {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }

        let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }
}
