//! Account provisioning and plans. Signup (`routes::signup`) creates accounts
//! too when the operator opens it; this CLI is the other way, and the only
//! one on a server where signup is off. It prints one invite string, once;
//! the database keeps only its hash.

use crate::billing::{self, Actor, Status};
use crate::invite::{self, Invite, INVITE_TTL_DAYS};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use clap::Subcommand;
use deadpool_postgres::Pool;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum AdminCommand {
    /// Create an account and print its single-use invite string.
    NewAccount {
        #[arg(long)]
        email: String,
        /// The public URL clients should talk to, carried in the invite.
        #[arg(long = "server-url")]
        server_url: String,
        /// Start a 14-day trial at activation instead of a complimentary
        /// plan with no end date.
        #[arg(long)]
        trial: bool,
    },
    /// List accounts: id, email, status, plan status, activation time.
    ListAccounts,
    /// Delete an account and everything it owns. Irreversible.
    DeleteAccount {
        #[arg(long)]
        email: String,
    },
    /// Set an account's plan status, recording who did it.
    SetPlan {
        #[arg(long)]
        email: String,
        #[arg(long, value_enum)]
        status: Status,
        /// When the status ends: a date (`2027-01-31`, midnight UTC) or an
        /// RFC 3339 time. Read as the trial end for `trialing`, the paid
        /// period end for `active`, the grace end for `past_due`.
        #[arg(long)]
        until: Option<String>,
    },
}

/// Returns what the operator should see on stdout. Returning it rather than
/// printing keeps the invite out of this crate's logs and lets a test read it.
pub async fn run(cmd: AdminCommand, pool: &Pool) -> Result<String, String> {
    match cmd {
        AdminCommand::NewAccount {
            email,
            server_url,
            trial,
        } => new_account(pool, &email, &server_url, trial).await,
        AdminCommand::ListAccounts => list_accounts(pool).await,
        AdminCommand::DeleteAccount { email } => delete_account(pool, &email).await,
        AdminCommand::SetPlan {
            email,
            status,
            until,
        } => set_plan(pool, &email, status, until.as_deref()).await,
    }
}

/// Create an invited account with its plan row and return its invite string.
async fn new_account(
    pool: &Pool,
    email: &str,
    server_url: &str,
    trial: bool,
) -> Result<String, String> {
    let email = crate::email::normalize(email)?.to_string();
    let server_url = crate::config::check_public_url(server_url)?;
    let account = Uuid::new_v4();
    let secret = invite::generate_secret();
    let now = Utc::now();
    let mut client = pool.get().await.map_err(|_| "no database".to_string())?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| "could not create the account".to_string())?;
    tx.execute(
        "INSERT INTO accounts
           (id, email_normalized, status, invite_hash, invite_expires_at, created_at, created_by)
         VALUES ($1, $2, 'invited', $3, $4, $5, 'admin')",
        &[
            &account,
            &email,
            &invite::hash(&secret).to_vec(),
            &(now + Duration::days(INVITE_TTL_DAYS)),
            &now,
        ],
    )
    .await
    .map_err(|e| match e.code().map(|c| c.code().to_string()) {
        Some(code) if code == "23505" => "an account with that email already exists".to_string(),
        _ => "could not create the account".to_string(),
    })?;
    let status = if trial {
        Status::Trialing
    } else {
        Status::Complimentary
    };
    billing::set_status(&tx, account, Actor::Admin, status, None, "new-account")
        .await
        .map_err(|_| "could not create the account".to_string())?;
    tx.commit()
        .await
        .map_err(|_| "could not create the account".to_string())?;
    Ok(invite::encode(&Invite {
        server: server_url,
        email,
        account,
        secret: secret.to_string(),
    }))
}

/// One line per account: id, email, status, plan status, activation time.
async fn list_accounts(pool: &Pool) -> Result<String, String> {
    let client = pool.get().await.map_err(|_| "no database".to_string())?;
    let rows = client
        .query(
            "SELECT a.id, a.email_normalized, a.status, coalesce(p.status, '-'), a.activated_at
               FROM accounts a LEFT JOIN subscriptions p ON p.account_id = a.id
              ORDER BY a.created_at",
            &[],
        )
        .await
        .map_err(|_| "could not list accounts".to_string())?;
    Ok(rows
        .iter()
        .map(|row| {
            let id: Uuid = row.get(0);
            let email: String = row.get(1);
            let status: String = row.get(2);
            let plan: String = row.get(3);
            let at: Option<DateTime<Utc>> = row.get(4);
            format!(
                "{id}  {email}  {status}  {plan}  {}",
                at.map(|t| t.to_rfc3339()).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

async fn set_plan(
    pool: &Pool,
    email: &str,
    status: Status,
    until: Option<&str>,
) -> Result<String, String> {
    let email = crate::email::normalize(email)?;
    let until = until.map(parse_until).transpose()?;
    let failed = |_| "could not set the plan".to_string();
    let mut client = pool.get().await.map_err(|_| "no database".to_string())?;
    let tx = client.transaction().await.map_err(failed)?;
    let id: Uuid = tx
        .query_opt(
            "SELECT id FROM accounts WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await
        .map_err(failed)?
        .ok_or_else(|| "no such account".to_string())?
        .get(0);
    billing::set_status(&tx, id, Actor::Admin, status, until, "set-plan")
        .await
        .map_err(failed)?;
    tx.commit().await.map_err(failed)?;
    Ok(match until {
        Some(t) => format!("{} until {}", status.as_str(), t.to_rfc3339()),
        None => status.as_str().to_string(),
    })
}

/// `YYYY-MM-DD` (midnight UTC) or RFC 3339.
fn parse_until(raw: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(t) = DateTime::parse_from_rfc3339(raw.trim()) {
        return Ok(t.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc())
        .ok_or_else(|| "--until must be a date (2027-01-31) or an RFC 3339 time".to_string())
}

async fn delete_account(pool: &Pool, email: &str) -> Result<String, String> {
    let email = crate::email::normalize(email)?;
    let failed = |_| "could not delete the account".to_string();
    let mut client = pool.get().await.map_err(|_| "no database".to_string())?;
    let tx = client.transaction().await.map_err(failed)?;
    let id: Uuid = tx
        .query_opt(
            "SELECT id FROM accounts WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await
        .map_err(failed)?
        .ok_or_else(|| "no such account".to_string())?
        .get(0);
    crate::erase::erase_account(&tx, id).await.map_err(failed)?;
    tx.commit().await.map_err(failed)?;
    Ok("deleted".into())
}
