//! Account provisioning.
//!
//! There is no public signup (design §2), so this CLI is the only way an
//! account comes into existence. It prints one invite string, once; the
//! database keeps only its hash.

use crate::invite::{self, Invite, INVITE_TTL_DAYS};
use chrono::{Duration, Utc};
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
    },
    /// List accounts: id, email, status, activation time.
    ListAccounts,
    /// Delete an account and everything it owns. Irreversible.
    DeleteAccount {
        #[arg(long)]
        email: String,
    },
}

/// Returns what the operator should see on stdout. Returning it rather than
/// printing keeps the invite out of this crate's logs and lets a test read it.
pub async fn run(cmd: AdminCommand, pool: &Pool) -> Result<String, String> {
    let client = pool.get().await.map_err(|_| "no database".to_string())?;
    match cmd {
        AdminCommand::NewAccount { email, server_url } => {
            let email = crate::email::normalize(&email)?.to_string();
            let server_url = check_server_url(&server_url)?;
            let account = Uuid::new_v4();
            let secret = invite::generate_secret();
            let now = Utc::now();
            client
                .execute(
                    "INSERT INTO accounts
                       (id, email_normalized, status, invite_hash, invite_expires_at, created_at)
                     VALUES ($1, $2, 'invited', $3, $4, $5)",
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
                    Some(code) if code == "23505" => {
                        "an account with that email already exists".to_string()
                    }
                    _ => "could not create the account".to_string(),
                })?;
            Ok(invite::encode(&Invite {
                server: server_url,
                email,
                account,
                secret: secret.to_string(),
            }))
        }
        AdminCommand::ListAccounts => {
            let rows = client
                .query(
                    "SELECT id, email_normalized, status, activated_at
                       FROM accounts ORDER BY created_at",
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
                    let at: Option<chrono::DateTime<Utc>> = row.get(3);
                    format!(
                        "{id}  {email}  {status}  {}",
                        at.map(|t| t.to_rfc3339()).unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        AdminCommand::DeleteAccount { email } => {
            let email = crate::email::normalize(&email)?;
            let removed = client
                .execute("DELETE FROM accounts WHERE email_normalized = $1", &[&email])
                .await
                .map_err(|_| "could not delete the account".to_string())?;
            if removed == 0 {
                return Err("no such account".into());
            }
            Ok("deleted".into())
        }
    }
}

/// The invite carries this URL and a client will send its master-password
/// proof there, so it must be one the client can safely trust: HTTPS, or an
/// explicit localhost for development.
fn check_server_url(raw: &str) -> Result<String, String> {
    let url = raw.trim().trim_end_matches('/').to_string();
    let local = url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1");
    if !url.starts_with("https://") && !local {
        return Err("the server URL must be https (or http on localhost)".into());
    }
    if url.len() > 512 {
        return Err("the server URL is too long".into());
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::check_server_url;

    #[test]
    fn a_plain_http_server_url_is_refused() {
        assert!(check_server_url("http://vault.example.com").is_err());
        assert_eq!(
            check_server_url("https://vault.example.com/").unwrap(),
            "https://vault.example.com"
        );
        assert!(check_server_url("http://localhost:8080").is_ok());
    }
}
