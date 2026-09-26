//! Typed messages. Anything that does not parse into these types exactly is
//! rejected: unknown request types, unknown fields, wrong field types, a
//! different protocol version, and over-long URLs.

use crate::secret::WireSecret;
use crate::{
    COSE_ES256, CREDENTIAL_ID_BYTES, MAX_CHALLENGE_BYTES, MAX_CREDENTIAL_LIST, MAX_MATCHES,
    MAX_RP_ID_BYTES, MAX_SECRET_BYTES, MAX_URL_BYTES, MAX_USERNAME_BYTES, MAX_USER_HANDLE_BYTES,
    PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroizing;

// ------------------------------------------------------------------ requests

/// `{"v":1,"id":7,"request":{"type":"find_matches","url":"https://…"}}`
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    pub v: u32,
    /// Chosen by the extension; echoed in the response.
    pub id: u32,
    pub request: Request,
}

/// Everything the extension can ask for. There is deliberately no request
/// that unlocks the vault, lists all items, or reads notes: the master
/// password never reaches the browser, and secrets are only ever released
/// for one item on a page that item is saved for.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    /// Lock state. Answered while locked.
    Status {},
    /// Lock the vault. Answered while locked (no-op).
    Lock {},
    /// Logins saved for `url`. IDs, titles and usernames only.
    ///
    /// `top_url` is set when `url` is an iframe: the tab's top-level page.
    /// Items must then match both.
    FindMatches {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// Username and password of `item_id`, only if it is saved for `url`.
    FillItem {
        item_id: Uuid,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// Current TOTP code of `item_id`, only if it is saved for `url`.
    GetTotp {
        item_id: Uuid,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// A new random password from the desktop's generator (default policy).
    GeneratePassword {},
    /// Would saving this submitted login add a new item, update one, or do
    /// nothing? The password is compared inside the core, never returned.
    CheckLogin {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        username: Option<String>,
        password: WireSecret,
    },
    /// Save a submitted login after the user confirmed it. With `item_id`,
    /// replace that login's password (it must be saved for `url`).
    SaveLogin {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        username: Option<String>,
        password: WireSecret,
        item_id: Option<Uuid>,
    },
    /// Passkeys for `rp_id` usable on `url`. Public data only.
    /// `allow_credentials`: the site's allowCredentials (ours only).
    FindPasskeys {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        rp_id: String,
        allow_credentials: Vec<String>,
    },
    /// Sign a WebAuthn assertion with one passkey, only if it is bound to
    /// `rp_id` and `url` may use `rp_id`.
    PasskeyGet {
        item_id: Uuid,
        credential_id: String,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        rp_id: String,
        challenge: String,
    },
    /// Before showing the save card: excluded? which logins can hold it?
    CheckPasskeyCreate {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        rp_id: String,
        user_name: String,
        exclude_credentials: Vec<String>,
        /// mediation: "conditional" (the site's automatic upgrade).
        conditional: bool,
    },
    /// Create a passkey after the user confirmed. A server write.
    PasskeyCreate {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        rp_id: String,
        challenge: String,
        user_handle: String,
        user_name: String,
        display_name: Option<String>,
        item_id: Option<Uuid>,
        /// mediation: "conditional" (the site's automatic upgrade).
        conditional: bool,
    },
    /// Does HavenKeys hold a passkey `url` may use? A yes/no only.
    PasskeyStatus {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
}

/// Decoded length of an unpadded base64url string, or `None`.
fn b64url_len(s: &str) -> Option<usize> {
    if s.len() % 4 == 1
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    Some(s.len() / 4 * 3 + [0, 0, 1, 2][s.len() % 4])
}

fn b64url_ok(s: &str, min: usize, max: usize) -> bool {
    b64url_len(s).is_some_and(|n| (min..=max).contains(&n))
}

fn credential_ok(s: &str) -> bool {
    b64url_ok(s, CREDENTIAL_ID_BYTES, CREDENTIAL_ID_BYTES)
}

fn credential_list_ok(v: &[String]) -> bool {
    v.len() <= MAX_CREDENTIAL_LIST && v.iter().all(|c| credential_ok(c))
}

fn rp_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_RP_ID_BYTES
}

fn name_ok(s: &str) -> bool {
    s.len() <= MAX_USERNAME_BYTES
}

impl Request {
    pub fn kind(&self) -> &'static str {
        match self {
            Request::Status {} => "status",
            Request::Lock {} => "lock",
            Request::FindMatches { .. } => "find_matches",
            Request::FillItem { .. } => "fill_item",
            Request::GetTotp { .. } => "get_totp",
            Request::GeneratePassword {} => "generate_password",
            Request::CheckLogin { .. } => "check_login",
            Request::SaveLogin { .. } => "save_login",
            Request::FindPasskeys { .. } => "find_passkeys",
            Request::PasskeyGet { .. } => "passkey_get",
            Request::CheckPasskeyCreate { .. } => "check_passkey_create",
            Request::PasskeyCreate { .. } => "passkey_create",
            Request::PasskeyStatus { .. } => "passkey_status",
        }
    }

    fn urls(&self) -> [Option<&str>; 2] {
        match self {
            Request::Status {} | Request::Lock {} | Request::GeneratePassword {} => [None, None],
            Request::FindMatches { url, top_url }
            | Request::FillItem { url, top_url, .. }
            | Request::GetTotp { url, top_url, .. }
            | Request::CheckLogin { url, top_url, .. }
            | Request::SaveLogin { url, top_url, .. }
            | Request::FindPasskeys { url, top_url, .. }
            | Request::PasskeyGet { url, top_url, .. }
            | Request::CheckPasskeyCreate { url, top_url, .. }
            | Request::PasskeyCreate { url, top_url, .. }
            | Request::PasskeyStatus { url, top_url } => [Some(url), top_url.as_deref()],
        }
    }

    fn field_sizes_ok(&self) -> bool {
        let urls_ok = self
            .urls()
            .into_iter()
            .flatten()
            .all(|u| !u.is_empty() && u.len() <= MAX_URL_BYTES);
        let login_ok = match self {
            Request::CheckLogin {
                username, password, ..
            }
            | Request::SaveLogin {
                username, password, ..
            } => {
                !password.expose().is_empty()
                    && password.expose().len() <= MAX_SECRET_BYTES
                    && username
                        .as_ref()
                        .is_none_or(|u| u.len() <= MAX_USERNAME_BYTES)
            }
            _ => true,
        };
        let passkey_ok = match self {
            Request::FindPasskeys {
                rp_id,
                allow_credentials,
                ..
            } => rp_ok(rp_id) && credential_list_ok(allow_credentials),
            Request::PasskeyGet {
                credential_id,
                rp_id,
                challenge,
                ..
            } => {
                rp_ok(rp_id)
                    && credential_ok(credential_id)
                    && b64url_ok(challenge, 1, MAX_CHALLENGE_BYTES)
            }
            Request::CheckPasskeyCreate {
                rp_id,
                user_name,
                exclude_credentials,
                ..
            } => rp_ok(rp_id) && name_ok(user_name) && credential_list_ok(exclude_credentials),
            Request::PasskeyCreate {
                rp_id,
                challenge,
                user_handle,
                user_name,
                display_name,
                ..
            } => {
                rp_ok(rp_id)
                    && b64url_ok(challenge, 1, MAX_CHALLENGE_BYTES)
                    && b64url_ok(user_handle, 1, MAX_USER_HANDLE_BYTES)
                    && name_ok(user_name)
                    && display_name.as_deref().is_none_or(name_ok)
            }
            _ => true,
        };
        urls_ok && login_ok && passkey_ok
    }
}

// URLs are not secrets, but they are browsing history: keep them out of Debug.
impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Request({})", self.kind())
    }
}

impl fmt::Debug for RequestEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RequestEnvelope(id={}, {:?})", self.id, self.request)
    }
}

/// Why a request was refused before it reached any handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rejection {
    /// The request ID, if one could be read, so the caller can fail the
    /// matching pending request immediately.
    pub id: Option<u32>,
    pub code: ErrorCode,
}

impl Rejection {
    pub fn response(&self) -> Response {
        Response::err(self.id, self.code)
    }
}

/// Only used to recover `v` and `id` from a message that failed to parse.
#[derive(Deserialize)]
struct Probe {
    v: Option<serde_json::Value>,
    id: Option<serde_json::Value>,
}

/// Parse and validate one request. Error details never echo the input.
pub fn parse_request(bytes: &[u8]) -> Result<RequestEnvelope, Rejection> {
    let probe = serde_json::from_slice::<Probe>(bytes).ok();
    let id = probe
        .as_ref()
        .and_then(|p| p.id.as_ref())
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok());
    let reject = |code| Rejection { id, code };

    if let Some(v) = probe.as_ref().and_then(|p| p.v.as_ref()) {
        if v.as_u64() != Some(u64::from(PROTOCOL_VERSION)) {
            return Err(reject(ErrorCode::UnsupportedVersion));
        }
    }
    let env: RequestEnvelope =
        serde_json::from_slice(bytes).map_err(|_| reject(ErrorCode::Malformed))?;
    if !env.request.field_sizes_ok() {
        return Err(reject(ErrorCode::InvalidInput));
    }
    Ok(env)
}

// ------------------------------------------------------------------ responses

/// `{"v":1,"id":7,"result":{…}}` or `{"v":1,"id":7,"error":{…}}`.
///
/// `id` is null only when a request was so malformed that its ID could not
/// be read. Exactly one of `result` and `error` is present.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub v: u32,
    pub id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
}

impl Response {
    pub fn ok(id: u32, result: ResultBody) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Option<u32>, code: ErrorCode) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id,
            result: None,
            error: Some(code.into()),
        }
    }

    fn is_valid(&self) -> bool {
        if self.v != PROTOCOL_VERSION || self.result.is_some() == self.error.is_some() {
            return false;
        }
        if self.result.is_some() && self.id.is_none() {
            return false;
        }
        match &self.result {
            Some(ResultBody::FindMatches { matches }) => matches.len() <= MAX_MATCHES,
            Some(ResultBody::CheckLogin { action, item_id }) => {
                (*action == SaveAction::Update) == item_id.is_some()
            }
            Some(ResultBody::FindPasskeys { passkeys }) => {
                passkeys.len() <= MAX_MATCHES
                    && passkeys.iter().all(|p| credential_ok(&p.credential_id))
            }
            Some(ResultBody::CheckPasskeyCreate {
                excluded,
                candidates,
                upgrade,
            }) => {
                candidates.len() <= MAX_MATCHES
                    && !(*excluded && !candidates.is_empty())
                    && !(*excluded && *upgrade != UpgradeHint::None {})
            }
            Some(ResultBody::PasskeyCreate {
                credential_id,
                public_key_algorithm,
                ..
            }) => credential_ok(credential_id) && *public_key_algorithm == COSE_ES256,
            Some(ResultBody::PasskeyGet { credential_id, .. }) => credential_ok(credential_id),
            _ => true,
        }
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("id", &self.id)
            .field("result", &self.result)
            .field("error", &self.error)
            .finish()
    }
}

/// The result type always names the request it answers.
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResultBody {
    Status {
        state: LockState,
        vault_exists: bool,
    },
    Lock {},
    FindMatches {
        matches: Vec<Match>,
    },
    FillItem {
        username: Option<String>,
        password: Option<WireSecret>,
        /// Rust's decision that the extension may press the sign-in button.
        auto_submit: bool,
    },
    GetTotp {
        code: WireSecret,
        period: u32,
        seconds_remaining: u32,
        auto_submit: bool,
    },
    GeneratePassword {
        password: WireSecret,
    },
    CheckLogin {
        action: SaveAction,
        /// The login `update` would change.
        item_id: Option<Uuid>,
    },
    SaveLogin {
        item_id: Uuid,
    },
    FindPasskeys {
        passkeys: Vec<PasskeyMatch>,
    },
    PasskeyGet {
        credential_id: String,
        authenticator_data: String,
        client_data_json: String,
        signature: String,
        user_handle: String,
    },
    CheckPasskeyCreate {
        excluded: bool,
        candidates: Vec<PasskeyCandidate>,
        upgrade: UpgradeHint,
    },
    PasskeyCreate {
        credential_id: String,
        attestation_object: String,
        client_data_json: String,
        authenticator_data: String,
        /// SPKI DER, base64url.
        public_key: String,
        public_key_algorithm: i64,
    },
    PasskeyStatus {
        has_passkey: bool,
    },
}

/// What saving a submitted login would do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveAction {
    Add,
    Update,
    Unchanged,
}

// Usernames and titles are not secrets, but they are personal: Debug shows
// only the result type.
impl fmt::Debug for ResultBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            ResultBody::Status { .. } => "status",
            ResultBody::Lock {} => "lock",
            ResultBody::FindMatches { .. } => "find_matches",
            ResultBody::FillItem { .. } => "fill_item",
            ResultBody::GetTotp { .. } => "get_totp",
            ResultBody::GeneratePassword { .. } => "generate_password",
            ResultBody::CheckLogin { .. } => "check_login",
            ResultBody::SaveLogin { .. } => "save_login",
            ResultBody::FindPasskeys { .. } => "find_passkeys",
            ResultBody::PasskeyGet { .. } => "passkey_get",
            ResultBody::CheckPasskeyCreate { .. } => "check_passkey_create",
            ResultBody::PasskeyCreate { .. } => "passkey_create",
            ResultBody::PasskeyStatus { .. } => "passkey_status",
        };
        write!(f, "ResultBody({kind})")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockState {
    Locked,
    Unlocking,
    Unlocked,
    Locking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStrength {
    ExactUrl,
    SameHost,
    SameSite,
}

/// One suggestion. No secrets.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Match {
    pub id: Uuid,
    pub title: String,
    pub username: Option<String>,
    pub has_totp: bool,
    pub strength: MatchStrength,
}

impl fmt::Debug for Match {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Match")
            .field("id", &self.id)
            .field("strength", &self.strength)
            .finish_non_exhaustive()
    }
}

/// A passkey offered for a page. No secrets.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PasskeyMatch {
    pub item_id: Uuid,
    pub credential_id: String,
    pub title: String,
    pub user_name: String,
}

impl fmt::Debug for PasskeyMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasskeyMatch")
            .field("item_id", &self.item_id)
            .finish_non_exhaustive()
    }
}

/// A login that could hold a new passkey. No secrets.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PasskeyCandidate {
    pub item_id: Uuid,
    pub title: String,
    pub username: Option<String>,
}

impl fmt::Debug for PasskeyCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasskeyCandidate")
            .field("item_id", &self.item_id)
            .finish_non_exhaustive()
    }
}

/// The core's decision on a site's automatic passkey upgrade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UpgradeHint {
    None {},
    Ask { item_id: Uuid },
    Auto { item_id: Uuid },
}

// ------------------------------------------------------------------ errors

/// Stable error codes. Messages are fixed strings chosen here, never text
/// from the other side, so no input or secret can be echoed through an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Locked,
    Busy,
    NoVault,
    NotFound,
    Denied,
    InvalidInput,
    Decryption,
    Corrupted,
    Malformed,
    TooLarge,
    UnsupportedVersion,
    RateLimited,
    IntegrationDisabled,
    DesktopUnavailable,
    Offline,
    Internal,
}

impl ErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            ErrorCode::Locked => "HavenKeys is locked.",
            ErrorCode::Busy => "HavenKeys is busy. Try again in a moment.",
            ErrorCode::NoVault => "No vault has been created yet.",
            ErrorCode::NotFound => "Item not found.",
            ErrorCode::Denied => "This item is not saved for this website.",
            ErrorCode::InvalidInput => "Invalid request.",
            ErrorCode::Decryption => "Failed to decrypt vault item.",
            ErrorCode::Corrupted => "The vault item is damaged.",
            ErrorCode::Malformed => "Malformed message.",
            ErrorCode::TooLarge => "Message too large.",
            ErrorCode::UnsupportedVersion => "Unsupported protocol version.",
            ErrorCode::RateLimited => "Too many requests. Try again shortly.",
            ErrorCode::IntegrationDisabled => {
                "Browser integration is turned off in HavenKeys settings."
            }
            ErrorCode::DesktopUnavailable => "The HavenKeys app is not running.",
            ErrorCode::Offline => {
                "HavenKeys is offline. The vault is read-only until it reconnects."
            }
            ErrorCode::Internal => "Internal error.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireError {
    pub code: ErrorCode,
    pub message: String,
}

impl From<ErrorCode> for WireError {
    fn from(code: ErrorCode) -> Self {
        Self {
            code,
            message: code.message().to_owned(),
        }
    }
}

// ------------------------------------------------------------------ events

/// Pushed without a request: `{"v":1,"event":{"type":"locked"}}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub v: u32,
    pub event: Event,
}

impl EventEnvelope {
    pub fn new(event: Event) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            event,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// The vault locked. Drop every cached suggestion.
    Locked {},
    /// The vault unlocked.
    Unlocked {},
    /// Sent by the native host when the desktop app goes away.
    Disconnected {},
}

/// Anything the desktop (or the native host) sends toward the extension.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Outgoing {
    Response(Response),
    Event(EventEnvelope),
}

/// Every top-level key either message kind may carry. Parsing through this
/// (rather than an untagged enum) avoids serde buffering a copy of the whole
/// message, secrets included, that would never be wiped.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutgoing {
    v: u32,
    /// `None`: key absent. `Some(None)`: `"id": null`.
    #[serde(default, deserialize_with = "present")]
    id: Option<Option<u32>>,
    #[serde(default)]
    result: Option<ResultBody>,
    #[serde(default)]
    error: Option<WireError>,
    #[serde(default)]
    event: Option<Event>,
}

fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<u32>>, D::Error> {
    Option::<u32>::deserialize(d).map(Some)
}

impl Outgoing {
    /// Serialize into a zeroize-on-drop buffer (responses can hold secrets).
    pub fn to_bytes(&self) -> Option<Zeroizing<Vec<u8>>> {
        // Pre-sized so a message holding a secret is written without
        // reallocating, which would leave un-wiped copies in freed memory.
        let mut buf = Zeroizing::new(Vec::with_capacity(4096));
        serde_json::to_writer(&mut *buf, self).ok()?;
        Some(buf)
    }

    /// Parse and validate a message coming from the desktop. Error messages
    /// are replaced by the fixed text for their code.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let raw: RawOutgoing = serde_json::from_slice(bytes).ok()?;
        if raw.v != PROTOCOL_VERSION {
            return None;
        }
        match (raw.id, raw.event) {
            (None, Some(event)) if raw.result.is_none() && raw.error.is_none() => {
                Some(Outgoing::Event(EventEnvelope::new(event)))
            }
            (Some(id), None) => {
                let r = Response {
                    v: raw.v,
                    id,
                    result: raw.result,
                    // Replace the other side's text with our fixed message.
                    error: raw.error.map(|e| e.code.into()),
                };
                r.is_valid().then_some(Outgoing::Response(r))
            }
            _ => None,
        }
    }
}

impl From<Response> for Outgoing {
    fn from(r: Response) -> Self {
        Outgoing::Response(r)
    }
}

impl From<Event> for Outgoing {
    fn from(e: Event) -> Self {
        Outgoing::Event(EventEnvelope::new(e))
    }
}
