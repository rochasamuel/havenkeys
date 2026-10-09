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
        let plain = url.starts_with("smtp://") && !url.contains("tls=required");
        if plain {
            return Err("SMTP_URL must use smtps:// or smtp://…?tls=required".into());
        }
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            .map_err(|_| "SMTP_URL is not a valid SMTP URL".to_string())?
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
}

impl Recording {
    pub fn sent(&self) -> Vec<Mail> {
        self.mails.lock().unwrap().clone()
    }
}

impl Mailer for Recording {
    fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>> {
        Box::pin(async move {
            if self.fail.load(Ordering::SeqCst) {
                return Err(MailError);
            }
            self.mails.lock().unwrap().push(mail);
            Ok(())
        })
    }
}
