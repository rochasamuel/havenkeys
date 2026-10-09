//! Where a payment gateway's webhook will land (spec 2026-10-07 §5.5).
//!
//! Nothing calls this yet. When Stripe or Mercado Pago is wired up, its
//! webhook route verifies the signature, maps the event to a
//! `ProviderEvent`, and calls `apply_provider_event` inside a transaction.
//! Routes never touch `subscriptions` directly.

use super::{set_status, Actor, Status};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

pub struct ProviderEvent {
    /// `"stripe"` or `"mercadopago"`.
    pub provider: &'static str,
    pub customer: String,
    /// The gateway's subscription id.
    pub reference: String,
    pub status: Status,
    /// Paid until, for `Active`; the grace end, for `PastDue`.
    pub period_end: Option<DateTime<Utc>>,
    /// The gateway's event name, for the audit trail.
    pub reason: String,
}

pub async fn apply_provider_event(
    db: &impl GenericClient,
    account_id: Uuid,
    event: ProviderEvent,
) -> Result<(), tokio_postgres::Error> {
    set_status(
        db,
        account_id,
        Actor::Provider,
        event.status,
        event.period_end,
        &event.reason,
    )
    .await?;
    db.execute(
        "UPDATE subscriptions
            SET provider = $2, provider_customer = $3, provider_ref = $4
          WHERE account_id = $1",
        &[
            &account_id,
            &event.provider,
            &event.customer,
            &event.reference,
        ],
    )
    .await?;
    Ok(())
}
