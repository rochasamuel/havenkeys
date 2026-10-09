//! Outbound email: the signup code, the invite, and account notices.
//!
//! Everything goes through the `Mailer` trait so the routes and the daily
//! task can be tested with the in-memory `Recording` sender, and so the
//! SMTP library stays in one file. Bodies are plain text; nothing in them
//! is tracked, and no address, code or invite is ever logged (CLAUDE.md
//! §40): a send failure is logged by kind only.

pub mod templates;

use lettre::message::Mailbox;
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::{AsyncTransport, Message, Tokio1Executor};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// The SMTP error can quote the recipient, so it is never carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailError;

pub trait Mailer: Send + Sync {
    fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>>;
}

pub struct Smtp {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl Smtp {
    /// `url` is what `SMTP_URL` holds: `smtps://user:pass@host:465` for
    /// implicit TLS or `smtp://user:pass@host:587?tls=required` for
    /// STARTTLS. Plain `smtp://` without `tls=` is refused: the code it
    /// carries is a credential for the account.
    pub fn new(url: &str, from: &str) -> Result<Self, String> {
        // Decided on the parsed URL, as lettre will read it: the parser
        // lowercases the scheme, and only a real `tls` query pair counts.
        let encrypted = match url::Url::parse(url) {
            Ok(parsed) => match parsed.scheme() {
                "smtps" => true,
                "smtp" => parsed
                    .query_pairs()
                    .any(|(key, value)| key == "tls" && value == "required"),
                _ => false,
            },
            Err(_) => false,
        };
        if !encrypted {
            return Err("SMTP_URL must use smtps:// or smtp://…?tls=required".into());
        }
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            .map_err(|_| "SMTP_URL is not a valid SMTP URL".to_string())?
            .timeout(Some(std::time::Duration::from_secs(10)))
            .build();
        let from = from
            .parse::<Mailbox>()
            .map_err(|_| "SMTP_FROM is not a valid address".to_string())?;
        Ok(Self { transport, from })
    }
}

impl Mailer for Smtp {
    fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>> {
        Box::pin(async move {
            let to = mail.to.parse::<Mailbox>().map_err(|_| MailError)?;
            let message = Message::builder()
                .from(self.from.clone())
                .to(to)
                .subject(mail.subject)
                .body(mail.body)
                .map_err(|_| MailError)?;
            match self.transport.send(message).await {
                Ok(_) => Ok(()),
                Err(err) => {
                    // Kind only: lettre's message can carry the address.
                    let kind = if err.is_transient() {
                        "transient"
                    } else if err.is_permanent() {
                        "permanent"
                    } else {
                        "transport"
                    };
                    tracing::warn!(kind, "mail not sent");
                    Err(MailError)
                }
            }
        })
    }
}

/// Keeps every mail in memory. Tests read the code and the invite from it;
/// `fail` makes every send fail, to test the routes' behaviour when SMTP
/// is down.
#[derive(Default)]
pub struct Recording {
    mails: Mutex<Vec<Mail>>,
    pub fail: AtomicBool,
    /// When set, `send` waits for a notification before recording.
    pub block: Mutex<Option<std::sync::Arc<tokio::sync::Notify>>>,
}

impl Recording {
    pub fn sent(&self) -> Vec<Mail> {
        self.mails.lock().unwrap().clone()
    }
}

impl Mailer for Recording {
    fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>> {
        Box::pin(async move {
            let block = self.block.lock().unwrap().clone();
            if let Some(n) = block {
                n.notified().await;
            }
            if self.fail.load(Ordering::SeqCst) {
                return Err(MailError);
            }
            self.mails.lock().unwrap().push(mail);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Smtp;

    const FROM: &str = "HavenKeys <no-reply@havenkeys.net>";

    #[test]
    fn smtp_without_tls_is_refused_in_any_spelling() {
        assert!(Smtp::new("smtp://u:p@host:587", FROM).is_err());
        assert!(Smtp::new("SMTP://u:p@host:587", FROM).is_err());
        assert!(Smtp::new("smtp://u:tls=required@host", FROM).is_err());
        assert!(Smtp::new("not a url", FROM).is_err());
    }

    #[test]
    fn encrypted_smtp_is_accepted() {
        assert!(Smtp::new("smtp://u:p@host:587?tls=required", FROM).is_ok());
        assert!(Smtp::new("smtps://u:p@host:465", FROM).is_ok());
    }

    #[test]
    fn a_bad_sender_address_is_refused() {
        assert!(Smtp::new("smtps://u:p@host:465", "not an address").is_err());
    }
}
