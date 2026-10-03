//! Typed messages. Anything that does not parse into these types exactly is
//! rejected: unknown request types, unknown fields, wrong field types, a
//! different protocol version, and over-long URLs.

use crate::secret::WireSecret;
use crate::{
    COSE_ES256, CREDENTIAL_ID_BYTES, MAX_ACCOUNT_BYTES, MAX_CARD_CODE_BYTES, MAX_CARD_FRAMES,
    MAX_CARD_NUMBER_BYTES, MAX_CARD_ROLES, MAX_CARD_VALUE_BYTES, MAX_CHALLENGE_BYTES,
    MAX_CREDENTIAL_LIST, MAX_IDENTITY_ROLES, MAX_IDENTITY_VALUE_BYTES, MAX_MATCHES,
    MAX_PASSWORD_LENGTH, MAX_PROVIDER_ACCOUNTS, MAX_PROVIDER_ORIGINS, MAX_RP_ID_BYTES,
    MAX_SECRET_BYTES, MAX_TITLE_BYTES, MAX_URL_BYTES, MAX_USERNAME_BYTES, MAX_USER_HANDLE_BYTES,
    MIN_PASSWORD_LENGTH, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
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
    /// The desktop generator's saved policy (its generator tab), so the
    /// in-page menu can show it before the user changes anything.
    GeneratorOptions {},
    /// A new random password from the desktop's generator: its saved
    /// policy, or the one in `options`.
    GeneratePassword {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<PasswordOptions>,
    },
    /// Would saving this submitted login add a new item, update one, or do
    /// nothing? The password is compared inside the core, never returned.
    /// `current_password`: what a change-password form held as the current
    /// password; it picks the login to update when no username identifies it.
    CheckLogin {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        username: Option<String>,
        password: WireSecret,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current_password: Option<WireSecret>,
    },
    /// Save a submitted login after the user confirmed it. With `item_id`,
    /// replace that login's password (it must be saved for `url`).
    /// `title`: a new login's name from the save prompt (else the host).
    SaveLogin {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        username: Option<String>,
        password: WireSecret,
        item_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
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
    /// Bring the desktop window forward on `item_id`'s editor, only if it is
    /// a login saved for `url`. Returns nothing.
    OpenItem {
        item_id: Uuid,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// Start signing in with `item_id`'s provider, only if it is a login
    /// saved for `url` that signs in with one. No secrets come back.
    StartSso {
        item_id: Uuid,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// Would saving this "Sign in with" add a login, add the account to one,
    /// or do nothing?
    CheckSso {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        provider: SsoProvider,
        #[serde(deserialize_with = "required")]
        account: Option<String>,
    },
    /// Save a "Sign in with" after the user confirmed it. With `item_id`,
    /// set that login's account (it must be saved for `url`).
    SaveSso {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        provider: SsoProvider,
        #[serde(deserialize_with = "required")]
        account: Option<String>,
        item_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// The account's Identity: title, email and which roles have a value.
    /// No values. Any http(s) page; a frame only when same-site as its tab.
    FindIdentity {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// The identity's values for `roles`. Documents only with `documents`
    /// and on an https page (the core decides).
    FillIdentity {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        roles: Vec<IdentityRole>,
        documents: bool,
    },
    /// Bring the desktop window forward on the identity. Returns nothing.
    OpenIdentity {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// Cards any https page may be offered: overview data only, never a
    /// number or code. `insecure`: the tab's page is not https.
    FindCards {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// One card's values for the tab's card fields, one entry per frame.
    /// Every frame must be https and same-site as `top_url`, or a payment
    /// processor's (the core decides); one bad frame denies the request.
    FillCard {
        item_id: Uuid,
        top_url: String,
        frames: Vec<CardFrame>,
    },
    /// Save a card typed into a checkout after the user confirmed it. A server write.
    SaveCard {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cardholder_name: Option<String>,
        number: WireSecret,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verification_number: Option<WireSecret>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expiry: Option<String>,
    },
}

/// A form field's role for an identity fill. Mirrors
/// havenkeys_core::identity::FillRole; the bridge maps between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdentityRole {
    FullName,
    FirstName,
    MiddleName,
    LastName,
    Email,
    Phone,
    BirthDate,
    BirthDay,
    BirthMonth,
    BirthYear,
    Company,
    Street,
    Number,
    Complement,
    AddressLine1,
    AddressLine2,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
    Username,
    Cpf,
    Rg,
    Passport,
    DriversLicense,
}

/// A password policy. Mirrors havenkeys_core::generator::GeneratorOptions;
/// the bridge maps between them and the core checks them again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PasswordOptions {
    pub length: u32,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub avoid_ambiguous: bool,
}

impl PasswordOptions {
    fn is_valid(&self) -> bool {
        (MIN_PASSWORD_LENGTH..=MAX_PASSWORD_LENGTH).contains(&self.length)
            && (self.uppercase || self.lowercase || self.digits || self.symbols)
    }
}

/// One identity value for a fill.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityValue {
    pub role: IdentityRole,
    pub value: WireSecret,
}

impl fmt::Debug for IdentityValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentityValue")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// A checkout field's role for a card fill. Mirrors
/// havenkeys_core::card_page::CardRole; the bridge maps between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardRole {
    CardholderName,
    CardholderGivenName,
    CardholderFamilyName,
    Number,
    VerificationNumber,
    ExpiryMonth,
    ExpiryYear,
    Brand,
}

/// A card network. Mirrors havenkeys_core::card::CardBrand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardBrandId {
    Visa,
    Mastercard,
    Amex,
    Elo,
    Hipercard,
    Diners,
    Discover,
    Jcb,
    Unionpay,
    Maestro,
    Other,
}

/// One frame of a fill_card: its URL (from the browser) and its fields' roles.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardFrame {
    pub url: String,
    pub roles: Vec<CardRole>,
}

/// A card offered to a page. No number beyond the last four digits.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardMatch {
    pub id: Uuid,
    pub title: String,
    #[serde(deserialize_with = "required")]
    pub brand: Option<CardBrandId>,
    #[serde(deserialize_with = "required")]
    pub last4: Option<String>,
    /// `YYYY-MM`.
    #[serde(deserialize_with = "required")]
    pub expiry: Option<String>,
}

impl fmt::Debug for CardMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardMatch")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// One card value for a fill.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardValue {
    pub role: CardRole,
    pub value: WireSecret,
}

impl fmt::Debug for CardValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardValue")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// The values for one frame of a fill_card.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardFrameValues {
    pub values: Vec<CardValue>,
}

/// `YYYY-MM` in shape (the core checks the month).
fn expiry_shape_ok(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7
        && b[4] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || c.is_ascii_digit())
}

/// Requires the key to be present, unlike a bare `Option<T>` field (whose
/// key serde treats as optional even without `#[serde(default)]`). Still
/// accepts an explicit JSON `null`.
fn required<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
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

/// Not empty, and at most `max` bytes.
fn text_ok(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max
}

/// A password in a request.
fn secret_ok(s: &WireSecret) -> bool {
    text_ok(s.expose(), MAX_SECRET_BYTES)
}

/// No value appears twice.
fn all_unique<T: Eq + Hash>(items: impl IntoIterator<Item = T>) -> bool {
    let mut seen = HashSet::new();
    items.into_iter().all(|item| seen.insert(item))
}

fn credential_ok(s: &str) -> bool {
    b64url_ok(s, CREDENTIAL_ID_BYTES, CREDENTIAL_ID_BYTES)
}

fn credential_list_ok(v: &[String]) -> bool {
    v.len() <= MAX_CREDENTIAL_LIST && v.iter().all(|c| credential_ok(c))
}

fn rp_ok(s: &str) -> bool {
    text_ok(s, MAX_RP_ID_BYTES)
}

fn name_ok(s: &str) -> bool {
    s.len() <= MAX_USERNAME_BYTES
}

fn account_ok(a: &Option<String>) -> bool {
    a.as_deref().is_none_or(|a| text_ok(a, MAX_ACCOUNT_BYTES))
}

/// A title only names a new item; an update keeps its own.
fn title_ok(title: &Option<String>, item_id: &Option<Uuid>) -> bool {
    title
        .as_deref()
        .is_none_or(|t| item_id.is_none() && text_ok(t, MAX_TITLE_BYTES))
}

impl Request {
    pub fn kind(&self) -> &'static str {
        match self {
            Request::Status {} => "status",
            Request::Lock {} => "lock",
            Request::FindMatches { .. } => "find_matches",
            Request::FillItem { .. } => "fill_item",
            Request::GetTotp { .. } => "get_totp",
            Request::GeneratorOptions {} => "generator_options",
            Request::GeneratePassword { .. } => "generate_password",
            Request::CheckLogin { .. } => "check_login",
            Request::SaveLogin { .. } => "save_login",
            Request::FindPasskeys { .. } => "find_passkeys",
            Request::PasskeyGet { .. } => "passkey_get",
            Request::CheckPasskeyCreate { .. } => "check_passkey_create",
            Request::PasskeyCreate { .. } => "passkey_create",
            Request::PasskeyStatus { .. } => "passkey_status",
            Request::OpenItem { .. } => "open_item",
            Request::StartSso { .. } => "start_sso",
            Request::CheckSso { .. } => "check_sso",
            Request::SaveSso { .. } => "save_sso",
            Request::FindIdentity { .. } => "find_identity",
            Request::FillIdentity { .. } => "fill_identity",
            Request::OpenIdentity { .. } => "open_identity",
            Request::FindCards { .. } => "find_cards",
            Request::FillCard { .. } => "fill_card",
            Request::SaveCard { .. } => "save_card",
        }
    }

    fn urls(&self) -> Vec<&str> {
        match self {
            Request::Status {}
            | Request::Lock {}
            | Request::GeneratorOptions {}
            | Request::GeneratePassword { .. } => Vec::new(),
            Request::FillCard {
                top_url, frames, ..
            } => std::iter::once(top_url.as_str())
                .chain(frames.iter().map(|f| f.url.as_str()))
                .collect(),
            Request::FindMatches { url, top_url }
            | Request::FillItem { url, top_url, .. }
            | Request::GetTotp { url, top_url, .. }
            | Request::CheckLogin { url, top_url, .. }
            | Request::SaveLogin { url, top_url, .. }
            | Request::FindPasskeys { url, top_url, .. }
            | Request::PasskeyGet { url, top_url, .. }
            | Request::CheckPasskeyCreate { url, top_url, .. }
            | Request::PasskeyCreate { url, top_url, .. }
            | Request::PasskeyStatus { url, top_url }
            | Request::OpenItem { url, top_url, .. }
            | Request::StartSso { url, top_url, .. }
            | Request::CheckSso { url, top_url, .. }
            | Request::SaveSso { url, top_url, .. }
            | Request::FindIdentity { url, top_url }
            | Request::FillIdentity { url, top_url, .. }
            | Request::OpenIdentity { url, top_url }
            | Request::FindCards { url, top_url }
            | Request::SaveCard { url, top_url, .. } => [Some(url.as_str()), top_url.as_deref()]
                .into_iter()
                .flatten()
                .collect(),
        }
    }

    fn field_sizes_ok(&self) -> bool {
        self.urls().into_iter().all(|u| text_ok(u, MAX_URL_BYTES)) && self.own_fields_ok()
    }

    /// Each request's fields other than its URLs.
    fn own_fields_ok(&self) -> bool {
        match self {
            Request::CheckLogin {
                username,
                password,
                current_password,
                ..
            } => {
                secret_ok(password)
                    && current_password.as_ref().is_none_or(secret_ok)
                    && username.as_deref().is_none_or(name_ok)
            }
            Request::SaveLogin {
                username,
                password,
                title,
                item_id,
                ..
            } => {
                secret_ok(password)
                    && title_ok(title, item_id)
                    && username.as_deref().is_none_or(name_ok)
            }
            Request::CheckSso { account, .. } => account_ok(account),
            Request::SaveSso {
                account,
                title,
                item_id,
                ..
            } => account_ok(account) && title_ok(title, item_id),
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
            Request::FillIdentity { roles, .. } => {
                !roles.is_empty() && roles.len() <= MAX_IDENTITY_ROLES && all_unique(roles)
            }
            Request::FillCard { frames, .. } => {
                !frames.is_empty()
                    && frames.len() <= MAX_CARD_FRAMES
                    && frames.iter().all(|f| {
                        !f.roles.is_empty()
                            && f.roles.len() <= MAX_CARD_ROLES
                            && all_unique(&f.roles)
                    })
            }
            Request::SaveCard {
                title,
                cardholder_name,
                number,
                verification_number,
                expiry,
                ..
            } => {
                title_ok(title, &None)
                    && text_ok(number.expose(), MAX_CARD_NUMBER_BYTES)
                    && verification_number
                        .as_ref()
                        .is_none_or(|c| text_ok(c.expose(), MAX_CARD_CODE_BYTES))
                    && cardholder_name
                        .as_deref()
                        .is_none_or(|n| text_ok(n, MAX_CARD_VALUE_BYTES))
                    && expiry.as_deref().is_none_or(expiry_shape_ok)
            }
            Request::GeneratePassword { options } => options.is_none_or(|o| o.is_valid()),
            _ => true,
        }
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
            Some(ResultBody::GeneratorOptions { options }) => options.is_valid(),
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
            Some(ResultBody::StartSso {
                provider_origins, ..
            }) => {
                !provider_origins.is_empty()
                    && provider_origins.len() <= MAX_PROVIDER_ORIGINS
                    && provider_origins.iter().all(|o| o.len() <= MAX_URL_BYTES)
            }
            Some(ResultBody::CheckSso {
                action,
                item_id,
                accounts,
            }) => {
                (*action == SaveAction::Update) == item_id.is_some()
                    && accounts.len() <= MAX_PROVIDER_ACCOUNTS
                    && accounts.iter().all(|a| text_ok(a, MAX_ACCOUNT_BYTES))
            }
            Some(ResultBody::FindIdentity {
                title,
                email,
                roles,
            }) => {
                title.len() <= MAX_TITLE_BYTES
                    && email.as_ref().is_none_or(|e| e.len() <= MAX_USERNAME_BYTES)
                    && roles.len() <= MAX_IDENTITY_ROLES
                    && all_unique(roles)
            }
            Some(ResultBody::FillIdentity { values }) => {
                values.len() <= MAX_IDENTITY_ROLES
                    && all_unique(values.iter().map(|v| v.role))
                    && values
                        .iter()
                        .all(|v| text_ok(v.value.expose(), MAX_IDENTITY_VALUE_BYTES))
            }
            Some(ResultBody::FindCards { insecure, cards }) => {
                cards.len() <= MAX_MATCHES
                    && !(*insecure && !cards.is_empty())
                    && cards.iter().all(|c| {
                        c.title.len() <= MAX_TITLE_BYTES
                            && c.last4.as_deref().is_none_or(|l| {
                                l.len() == 4 && l.bytes().all(|b| b.is_ascii_digit())
                            })
                            && c.expiry.as_deref().is_none_or(expiry_shape_ok)
                    })
            }
            Some(ResultBody::FillCard { frames }) => {
                frames.len() <= MAX_CARD_FRAMES
                    && frames.iter().all(|f| {
                        f.values.len() <= MAX_CARD_ROLES
                            && all_unique(f.values.iter().map(|v| v.role))
                            && f.values
                                .iter()
                                .all(|v| text_ok(v.value.expose(), MAX_CARD_VALUE_BYTES))
                    })
            }
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
    GeneratorOptions {
        options: PasswordOptions,
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
    OpenItem {},
    StartSso {
        provider: SsoProvider,
        account: Option<String>,
        /// Where the run may click the account (exact origins, from Rust).
        provider_origins: Vec<String>,
        /// Rust's decision that the run may click the saved account.
        auto_choose: bool,
    },
    CheckSso {
        action: SaveAction,
        item_id: Option<Uuid>,
        /// The vault's accounts for the provider, offered in the save prompt
        /// (never secrets; empty when `unchanged`). Absent from older desktops.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        accounts: Vec<String>,
    },
    SaveSso {
        item_id: Uuid,
    },
    FindIdentity {
        title: String,
        email: Option<String>,
        /// Roles the identity has a value for; names only.
        roles: Vec<IdentityRole>,
    },
    FillIdentity {
        values: Vec<IdentityValue>,
    },
    OpenIdentity {},
    FindCards {
        insecure: bool,
        cards: Vec<CardMatch>,
    },
    FillCard {
        frames: Vec<CardFrameValues>,
    },
    SaveCard {
        item_id: Uuid,
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
            ResultBody::GeneratorOptions { .. } => "generator_options",
            ResultBody::GeneratePassword { .. } => "generate_password",
            ResultBody::CheckLogin { .. } => "check_login",
            ResultBody::SaveLogin { .. } => "save_login",
            ResultBody::FindPasskeys { .. } => "find_passkeys",
            ResultBody::PasskeyGet { .. } => "passkey_get",
            ResultBody::CheckPasskeyCreate { .. } => "check_passkey_create",
            ResultBody::PasskeyCreate { .. } => "passkey_create",
            ResultBody::PasskeyStatus { .. } => "passkey_status",
            ResultBody::OpenItem {} => "open_item",
            ResultBody::StartSso { .. } => "start_sso",
            ResultBody::CheckSso { .. } => "check_sso",
            ResultBody::SaveSso { .. } => "save_sso",
            ResultBody::FindIdentity { .. } => "find_identity",
            ResultBody::FillIdentity { .. } => "fill_identity",
            ResultBody::OpenIdentity {} => "open_identity",
            ResultBody::FindCards { .. } => "find_cards",
            ResultBody::FillCard { .. } => "fill_card",
            ResultBody::SaveCard { .. } => "save_card",
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

/// A "Sign in with" provider. Mirrors havenkeys_core::sso::SsoProvider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SsoProvider {
    Google,
    Microsoft,
    Github,
    Apple,
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
    #[serde(deserialize_with = "required")]
    pub provider: Option<SsoProvider>,
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
