//! The WebAuthn JSON Android's Credential Manager carries
//! (`PublicKeyCredentialCreationOptionsJSON`, `…RequestOptionsJSON` and the
//! two responses). Requests come from the calling app or browser and are
//! untrusted: bounded, strictly decoded, and anything an authenticator does
//! not need is ignored. The core re-checks every value it uses.

use crate::error::{MobileError, MobileResult};
use data_encoding::BASE64URL_NOPAD;
use havenkeys_core::passkey::{
    encode_b64url, Assertion, Registration, COSE_ALG_ES256, MAX_CREDENTIAL_LIST,
};
use serde::Deserialize;

pub(crate) const MAX_REQUEST_JSON_BYTES: usize = 64 * 1024;
const PUBLIC_KEY: &str = "public-key";

pub(crate) struct CreationOptions {
    pub rp_id: Option<String>,
    pub user_handle: Vec<u8>,
    pub user_name: String,
    pub display_name: Option<String>,
    pub challenge: Vec<u8>,
    pub exclude: Vec<Vec<u8>>,
}

pub(crate) struct RequestOptions {
    pub rp_id: Option<String>,
    pub challenge: Vec<u8>,
    pub allow: Vec<Vec<u8>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCreation {
    rp: RawRp,
    user: RawUser,
    challenge: String,
    #[serde(default)]
    pub_key_cred_params: Vec<RawParam>,
    #[serde(default)]
    exclude_credentials: Vec<RawDescriptor>,
}

#[derive(Deserialize)]
struct RawRp {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUser {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct RawParam {
    #[serde(rename = "type")]
    kind: String,
    alg: i64,
}

#[derive(Deserialize)]
struct RawDescriptor {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRequest {
    challenge: String,
    #[serde(default)]
    rp_id: Option<String>,
    #[serde(default)]
    allow_credentials: Vec<RawDescriptor>,
}

pub(crate) fn invalid_request() -> MobileError {
    MobileError::Failed {
        code: "invalid_input".into(),
        detail: "The passkey request is not valid.".into(),
    }
}

fn unsupported() -> MobileError {
    MobileError::Failed {
        code: "unsupported_algorithm".into(),
        detail: "The site asked for a passkey type HavenKeys cannot create.".into(),
    }
}

fn from_json<'a, T: Deserialize<'a>>(json: &'a str) -> MobileResult<T> {
    if json.len() > MAX_REQUEST_JSON_BYTES {
        return Err(invalid_request());
    }
    serde_json::from_str(json).map_err(|_| invalid_request())
}

/// Unpadded base64url; trailing padding is tolerated, nothing else.
fn bytes(s: &str) -> MobileResult<Vec<u8>> {
    BASE64URL_NOPAD
        .decode(s.trim_end_matches('=').as_bytes())
        .map_err(|_| invalid_request())
}

fn credential_ids(list: &[RawDescriptor]) -> MobileResult<Vec<Vec<u8>>> {
    if list.len() > MAX_CREDENTIAL_LIST {
        return Err(invalid_request());
    }
    list.iter()
        .filter(|d| d.kind == PUBLIC_KEY)
        .map(|d| bytes(&d.id))
        .collect()
}

pub(crate) fn parse_creation(json: &str) -> MobileResult<CreationOptions> {
    let raw: RawCreation = from_json(json)?;
    let es256 = raw.pub_key_cred_params.is_empty()
        || raw
            .pub_key_cred_params
            .iter()
            .any(|p| p.kind == PUBLIC_KEY && p.alg == COSE_ALG_ES256);
    if !es256 {
        return Err(unsupported());
    }
    Ok(CreationOptions {
        rp_id: raw.rp.id,
        user_handle: bytes(&raw.user.id)?,
        user_name: raw.user.name,
        display_name: raw.user.display_name,
        challenge: bytes(&raw.challenge)?,
        exclude: credential_ids(&raw.exclude_credentials)?,
    })
}

pub(crate) fn parse_request(json: &str) -> MobileResult<RequestOptions> {
    let raw: RawRequest = from_json(json)?;
    Ok(RequestOptions {
        rp_id: raw.rp_id,
        challenge: bytes(&raw.challenge)?,
        allow: credential_ids(&raw.allow_credentials)?,
    })
}

pub(crate) fn registration_json(r: &Registration) -> String {
    let id = encode_b64url(&r.credential_id);
    serde_json::json!({
        "id": id,
        "rawId": id,
        "type": PUBLIC_KEY,
        "authenticatorAttachment": "platform",
        "response": {
            "clientDataJSON": encode_b64url(&r.client_data_json),
            "attestationObject": encode_b64url(&r.attestation_object),
            "authenticatorData": encode_b64url(&r.authenticator_data),
            "publicKey": encode_b64url(&r.public_key),
            "publicKeyAlgorithm": COSE_ALG_ES256,
            "transports": ["internal"],
        },
        "clientExtensionResults": { "credProps": { "rk": true } },
    })
    .to_string()
}

pub(crate) fn assertion_json(a: &Assertion) -> String {
    let id = encode_b64url(&a.credential_id);
    serde_json::json!({
        "id": id,
        "rawId": id,
        "type": PUBLIC_KEY,
        "authenticatorAttachment": "platform",
        "response": {
            "clientDataJSON": encode_b64url(&a.client_data_json),
            "authenticatorData": encode_b64url(&a.authenticator_data),
            "signature": encode_b64url(&a.signature),
            "userHandle": encode_b64url(&a.user_handle),
        },
        "clientExtensionResults": {},
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATE: &str = r#"{
        "rp": {"id": "github.com", "name": "GitHub"},
        "user": {"id": "AQID", "name": "octo", "displayName": "Octo Cat"},
        "challenge": "BwcHBw",
        "pubKeyCredParams": [{"type": "public-key", "alg": -257}, {"type": "public-key", "alg": -7}],
        "excludeCredentials": [{"type": "public-key", "id": "CQkJ"}],
        "authenticatorSelection": {"residentKey": "required", "userVerification": "required"},
        "attestation": "none",
        "extensions": {"credProps": true}
    }"#;

    fn code(e: MobileError) -> String {
        match e {
            MobileError::Failed { code, .. } => code,
        }
    }

    #[test]
    fn a_creation_request_is_decoded() {
        let o = parse_creation(CREATE).unwrap();
        assert_eq!(o.rp_id.as_deref(), Some("github.com"));
        assert_eq!(o.user_handle, vec![1, 2, 3]);
        assert_eq!(o.user_name, "octo");
        assert_eq!(o.display_name.as_deref(), Some("Octo Cat"));
        assert_eq!(o.challenge, vec![7, 7, 7, 7]);
        assert_eq!(o.exclude, vec![vec![9, 9, 9]]);
    }

    #[test]
    fn es256_is_required_when_algorithms_are_listed() {
        let rsa_only = CREATE.replace(r#", {"type": "public-key", "alg": -7}"#, "");
        assert_eq!(
            code(parse_creation(&rsa_only).err().unwrap()),
            "unsupported_algorithm"
        );
        // No list means WebAuthn's defaults, which include ES256.
        let none = CREATE.replace(r#""pubKeyCredParams": [{"type": "public-key", "alg": -257}, {"type": "public-key", "alg": -7}],"#, "");
        assert!(parse_creation(&none).is_ok());
    }

    #[test]
    fn padded_base64url_is_accepted_and_bad_base64_is_not() {
        assert_eq!(
            parse_creation(&CREATE.replace("BwcHBw", "BwcHBw=="))
                .unwrap()
                .challenge,
            vec![7, 7, 7, 7]
        );
        assert_eq!(
            code(
                parse_creation(&CREATE.replace("BwcHBw", "not base64!"))
                    .err()
                    .unwrap()
            ),
            "invalid_input"
        );
        assert_eq!(
            code(
                parse_creation(&CREATE.replace("BwcHBw", "Bw+/Bw"))
                    .err()
                    .unwrap()
            ),
            "invalid_input"
        );
    }

    #[test]
    fn malformed_and_oversized_requests_are_refused() {
        for bad in [
            "",
            "null",
            "[]",
            "{}",
            r#"{"rp":{},"user":{"id":"AQ"}}"#,
            "{\"rp\":",
        ] {
            assert_eq!(
                code(parse_creation(bad).err().unwrap()),
                "invalid_input",
                "{bad}"
            );
        }
        let huge = CREATE.replace("GitHub", &"x".repeat(MAX_REQUEST_JSON_BYTES));
        assert_eq!(code(parse_creation(&huge).err().unwrap()), "invalid_input");
        let many: Vec<String> = (0..=havenkeys_core::passkey::MAX_CREDENTIAL_LIST)
            .map(|_| r#"{"type":"public-key","id":"AQ"}"#.to_owned())
            .collect();
        let long = format!(
            r#"{{"challenge":"AQ","allowCredentials":[{}]}}"#,
            many.join(",")
        );
        assert_eq!(code(parse_request(&long).err().unwrap()), "invalid_input");
    }

    #[test]
    fn a_get_request_is_decoded_and_rp_id_may_be_absent() {
        let r = parse_request(r#"{"challenge":"AQID","rpId":"github.com","allowCredentials":[{"type":"public-key","id":"CQkJ"}],"userVerification":"preferred"}"#).unwrap();
        assert_eq!(r.rp_id.as_deref(), Some("github.com"));
        assert_eq!(r.challenge, vec![1, 2, 3]);
        assert_eq!(r.allow, vec![vec![9, 9, 9]]);
        assert_eq!(
            parse_request(r#"{"challenge":"AQID"}"#).unwrap().rp_id,
            None
        );
    }

    #[test]
    fn credentials_of_another_type_are_ignored() {
        let r = parse_request(
            r#"{"challenge":"AQ","allowCredentials":[{"type":"other","id":"CQkJ"}]}"#,
        )
        .unwrap();
        assert!(r.allow.is_empty());
    }

    #[test]
    fn responses_carry_the_webauthn_fields() {
        let reg = Registration {
            credential_id: vec![1; 16],
            client_data_json: b"{}".to_vec(),
            authenticator_data: vec![2],
            attestation_object: vec![3],
            public_key: vec![4],
        };
        let v: serde_json::Value = serde_json::from_str(&registration_json(&reg)).unwrap();
        assert_eq!(v["id"], "AQEBAQEBAQEBAQEBAQEBAQ");
        assert_eq!(v["rawId"], v["id"]);
        assert_eq!(v["type"], "public-key");
        assert_eq!(v["response"]["clientDataJSON"], "e30");
        assert_eq!(v["response"]["attestationObject"], "Aw");
        assert_eq!(v["response"]["publicKeyAlgorithm"], -7);
        assert_eq!(v["clientExtensionResults"]["credProps"]["rk"], true);

        let a = Assertion {
            credential_id: vec![1; 16],
            client_data_json: b"{}".to_vec(),
            authenticator_data: vec![2],
            signature: vec![5],
            user_handle: vec![6],
        };
        let v: serde_json::Value = serde_json::from_str(&assertion_json(&a)).unwrap();
        assert_eq!(v["response"]["signature"], "BQ");
        assert_eq!(v["response"]["userHandle"], "Bg");
        assert_eq!(v["authenticatorAttachment"], "platform");
    }
}
