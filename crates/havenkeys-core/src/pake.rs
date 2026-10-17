//! The client's half of OPAQUE sign-in (spec 2026-10-13-pake-login). The
//! auth key's bytes go into `havenkeys-opaque` from here and nowhere else,
//! so `AuthKey::as_bytes` stays private to this crate.

use crate::crypto::keys::AuthKey;
pub use havenkeys_opaque::{ClientLogin, ClientRegistration, Error as PakeError, MAX_MESSAGE_BYTES};

pub fn login_start(key: &AuthKey) -> Result<(ClientLogin, Vec<u8>), PakeError> {
    ClientLogin::start(key.as_bytes())
}

pub fn login_finish(
    state: ClientLogin,
    key: &AuthKey,
    response: &[u8],
) -> Result<Vec<u8>, PakeError> {
    state.finish(key.as_bytes(), response)
}

pub fn register_start(key: &AuthKey) -> Result<(ClientRegistration, Vec<u8>), PakeError> {
    ClientRegistration::start(key.as_bytes())
}

pub fn register_finish(
    state: ClientRegistration,
    key: &AuthKey,
    response: &[u8],
) -> Result<Vec<u8>, PakeError> {
    state.finish(key.as_bytes(), response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::AuthKey;
    use havenkeys_opaque::{login_finish as server_finish, record_from_upload, ServerKeys};

    #[test]
    fn an_auth_key_registers_and_signs_in() {
        let key = AuthKey::from_bytes([4u8; 32]);
        let server = ServerKeys::generate();
        let id = [2u8; 16];
        let (reg, request) = register_start(&key).unwrap();
        let upload = register_finish(reg, &key, &server.register(&request, &id).unwrap()).unwrap();
        let record = record_from_upload(&upload).unwrap();
        let (login, request) = login_start(&key).unwrap();
        let (state, response) = server.login_start(Some(&record), &request, &id).unwrap();
        let finalization = login_finish(login, &key, &response).unwrap();
        assert_eq!(server_finish(&state, &finalization), Ok(()));
    }
}
