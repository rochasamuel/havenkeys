//! The one OPAQUE (RFC 9807) configuration HavenKeys uses, as byte-level
//! calls. Nothing here is new cryptography: it fixes the `opaque-ke` suite,
//! the key-exchange context and the size limits, and turns every library
//! error into one of two answers. See
//! docs/superpowers/specs/2026-10-13-pake-login-design.md.
//!
//! The "password" is the client's 32-byte auth key, already stretched with
//! Argon2id, so the suite's own key-stretching function is the identity.

#![forbid(unsafe_code)]

use opaque_ke::ksf::Identity;
use opaque_ke::{
    CipherSuite, ClientLoginFinishParameters, ClientRegistrationFinishParameters,
    CredentialFinalization, CredentialRequest, CredentialResponse, Identifiers,
    RegistrationRequest, RegistrationResponse, RegistrationUpload, Ristretto255,
    ServerLoginParameters, ServerRegistration, ServerSetup, TripleDh,
};
use rand_core::OsRng;
use zeroize::Zeroizing;

/// Bound into every key exchange, so a transcript from any other protocol
/// using the same suite never verifies here.
pub const CONTEXT: &[u8] = b"havenkeys/opaque/v1";

/// The largest OPAQUE message or stored value, in bytes. Every value this
/// suite produces is well under it (tested); the wire refuses anything
/// longer before parsing.
pub const MAX_MESSAGE_BYTES: usize = 512;

/// Every failure is one of two things, and neither says more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Bytes that are not a message of this suite.
    Malformed,
    /// A wrong key, a fake record, a tampered or replayed message.
    InvalidLogin,
}

impl From<opaque_ke::errors::ProtocolError> for Error {
    fn from(e: opaque_ke::errors::ProtocolError) -> Self {
        match e {
            opaque_ke::errors::ProtocolError::InvalidLoginError
            | opaque_ke::errors::ProtocolError::ReflectedValueError => Error::InvalidLogin,
            _ => Error::Malformed,
        }
    }
}

pub struct Suite;

impl CipherSuite for Suite {
    type OprfCs = Ristretto255;
    type KeyExchange = TripleDh<Ristretto255, sha2::Sha512>;
    type Ksf = Identity;
}

fn checked(bytes: &[u8]) -> Result<&[u8], Error> {
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::Malformed);
    }
    Ok(bytes)
}

fn server_params() -> ServerLoginParameters<'static, 'static> {
    ServerLoginParameters {
        context: Some(CONTEXT),
        identifiers: Identifiers::default(),
    }
}

/// A registration in progress on the client.
pub struct ClientRegistration(opaque_ke::ClientRegistration<Suite>);

impl ClientRegistration {
    pub fn start(password: &[u8]) -> Result<(Self, Vec<u8>), Error> {
        let started = opaque_ke::ClientRegistration::<Suite>::start(&mut OsRng, password)?;
        Ok((Self(started.state), started.message.serialize().to_vec()))
    }

    /// The registration upload the server stores.
    pub fn finish(self, password: &[u8], response: &[u8]) -> Result<Vec<u8>, Error> {
        let response = RegistrationResponse::<Suite>::deserialize(checked(response)?)
            .map_err(|_| Error::Malformed)?;
        let done = self.0.finish(
            &mut OsRng,
            password,
            response,
            ClientRegistrationFinishParameters::default(),
        )?;
        Ok(done.message.serialize().to_vec())
    }
}

impl std::fmt::Debug for ClientRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClientRegistration(<redacted>)")
    }
}

/// A sign-in in progress on the client.
pub struct ClientLogin(opaque_ke::ClientLogin<Suite>);

impl ClientLogin {
    pub fn start(password: &[u8]) -> Result<(Self, Vec<u8>), Error> {
        let started = opaque_ke::ClientLogin::<Suite>::start(&mut OsRng, password)?;
        Ok((Self(started.state), started.message.serialize().to_vec()))
    }

    /// The finalization to send. `InvalidLogin` means a wrong key or a
    /// response the server could not have made for this account; nothing
    /// should be sent then.
    pub fn finish(self, password: &[u8], response: &[u8]) -> Result<Vec<u8>, Error> {
        let response = CredentialResponse::<Suite>::deserialize(checked(response)?)
            .map_err(|_| Error::Malformed)?;
        let done = self.0.finish(
            &mut OsRng,
            password,
            response,
            ClientLoginFinishParameters::new(Some(CONTEXT), Identifiers::default(), None),
        )?;
        Ok(done.message.serialize().to_vec())
    }
}

impl std::fmt::Debug for ClientLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClientLogin(<redacted>)")
    }
}

/// The server's long-term OPAQUE secret: the OPRF seed and key pair.
pub struct ServerKeys(ServerSetup<Suite>);

impl ServerKeys {
    pub fn generate() -> Self {
        Self(ServerSetup::<Suite>::new(&mut OsRng))
    }

    pub fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.0.serialize().to_vec())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        ServerSetup::<Suite>::deserialize(bytes)
            .map(Self)
            .map_err(|_| Error::Malformed)
    }

    /// The registration response for `id`. Stateless.
    pub fn register(&self, request: &[u8], id: &[u8]) -> Result<Vec<u8>, Error> {
        let request = RegistrationRequest::<Suite>::deserialize(checked(request)?)
            .map_err(|_| Error::Malformed)?;
        let started = ServerRegistration::<Suite>::start(&self.0, request, id)
            .map_err(|_| Error::Malformed)?;
        Ok(started.message.serialize().to_vec())
    }

    /// The pending state to keep and the response to send. With no record,
    /// `opaque-ke` answers with a fake response derived from `id`, the same
    /// shape as a real one.
    pub fn login_start(
        &self,
        record: Option<&[u8]>,
        request: &[u8],
        id: &[u8],
    ) -> Result<(Zeroizing<Vec<u8>>, Vec<u8>), Error> {
        let record = match record {
            Some(r) => Some(
                ServerRegistration::<Suite>::deserialize(checked(r)?)
                    .map_err(|_| Error::Malformed)?,
            ),
            None => None,
        };
        let request = CredentialRequest::<Suite>::deserialize(checked(request)?)
            .map_err(|_| Error::Malformed)?;
        let started = opaque_ke::ServerLogin::start(
            &mut OsRng,
            &self.0,
            record,
            request,
            id,
            server_params(),
        )?;
        Ok((
            Zeroizing::new(started.state.serialize().to_vec()),
            started.message.serialize().to_vec(),
        ))
    }
}

impl std::fmt::Debug for ServerKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServerKeys(<redacted>)")
    }
}

/// What the server stores for an account, from the client's upload.
pub fn record_from_upload(upload: &[u8]) -> Result<Vec<u8>, Error> {
    let upload =
        RegistrationUpload::<Suite>::deserialize(checked(upload)?).map_err(|_| Error::Malformed)?;
    Ok(ServerRegistration::<Suite>::finish(upload)
        .serialize()
        .to_vec())
}

/// Checks the client's proof against a pending state from `login_start`.
pub fn login_finish(state: &[u8], finalization: &[u8]) -> Result<(), Error> {
    let state = opaque_ke::ServerLogin::<Suite>::deserialize(checked(state)?)
        .map_err(|_| Error::Malformed)?;
    let finalization = CredentialFinalization::<Suite>::deserialize(checked(finalization)?)
        .map_err(|_| Error::Malformed)?;
    state.finish(finalization, server_params())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [9u8; 32];
    const ID: &[u8] = &[1u8; 16];

    fn enrolled(server: &ServerKeys, key: &[u8]) -> Vec<u8> {
        let (state, request) = ClientRegistration::start(key).unwrap();
        let response = server.register(&request, ID).unwrap();
        let upload = state.finish(key, &response).unwrap();
        record_from_upload(&upload).unwrap()
    }

    fn login(server: &ServerKeys, record: Option<&[u8]>, key: &[u8]) -> Result<(), Error> {
        let (client, request) = ClientLogin::start(key)?;
        let (state, response) = server.login_start(record, &request, ID)?;
        let finalization = client.finish(key, &response)?;
        login_finish(&state, &finalization)
    }

    #[test]
    fn the_right_key_signs_in() {
        let server = ServerKeys::generate();
        let record = enrolled(&server, &KEY);
        assert_eq!(login(&server, Some(&record), &KEY), Ok(()));
    }

    #[test]
    fn a_wrong_key_fails_on_the_client() {
        let server = ServerKeys::generate();
        let record = enrolled(&server, &KEY);
        assert_eq!(
            login(&server, Some(&record), &[8u8; 32]),
            Err(Error::InvalidLogin)
        );
    }

    #[test]
    fn an_unknown_account_gets_a_fake_response_that_fails_like_a_wrong_key() {
        let server = ServerKeys::generate();
        let (_, request) = ClientLogin::start(&KEY).unwrap();
        let (_, fake) = server.login_start(None, &request, ID).unwrap();
        let record = enrolled(&server, &KEY);
        let (_, real) = server.login_start(Some(&record), &request, ID).unwrap();
        assert_eq!(fake.len(), real.len(), "same shape for known and unknown");
        assert_eq!(login(&server, None, &KEY), Err(Error::InvalidLogin));
    }

    #[test]
    fn a_finalization_replayed_into_another_exchange_fails() {
        let server = ServerKeys::generate();
        let record = enrolled(&server, &KEY);
        let (client, request) = ClientLogin::start(&KEY).unwrap();
        let (_, response) = server.login_start(Some(&record), &request, ID).unwrap();
        let finalization = client.finish(&KEY, &response).unwrap();
        let (_, request2) = ClientLogin::start(&KEY).unwrap();
        let (state2, _) = server.login_start(Some(&record), &request2, ID).unwrap();
        assert_eq!(
            login_finish(&state2, &finalization),
            Err(Error::InvalidLogin)
        );
    }

    #[test]
    fn server_keys_round_trip_through_bytes() {
        let server = ServerKeys::generate();
        let record = enrolled(&server, &KEY);
        let again = ServerKeys::from_bytes(&server.to_bytes()).unwrap();
        assert_eq!(login(&again, Some(&record), &KEY), Ok(()));
    }

    #[test]
    fn malformed_messages_are_refused_without_panicking() {
        let server = ServerKeys::generate();
        for junk in [&[][..], &[0u8; 3][..], &[0xffu8; 96][..], &[0u8; 4096][..]] {
            assert_eq!(server.register(junk, ID), Err(Error::Malformed));
            assert!(server.login_start(None, junk, ID).is_err());
            assert_eq!(record_from_upload(junk), Err(Error::Malformed));
            assert!(login_finish(junk, junk).is_err());
            assert!(ServerKeys::from_bytes(junk).is_err());
        }
        let (client, _) = ClientLogin::start(&KEY).unwrap();
        assert!(client.finish(&KEY, &[0u8; 10]).is_err());
    }

    #[test]
    fn every_message_fits_the_wire_limit() {
        let server = ServerKeys::generate();
        let (state, request) = ClientRegistration::start(&KEY).unwrap();
        let response = server.register(&request, ID).unwrap();
        let upload = state.finish(&KEY, &response).unwrap();
        let record = record_from_upload(&upload).unwrap();
        let (client, credential_request) = ClientLogin::start(&KEY).unwrap();
        let (pending, credential_response) = server
            .login_start(Some(&record), &credential_request, ID)
            .unwrap();
        let finalization = client.finish(&KEY, &credential_response).unwrap();
        for m in [
            &request,
            &response,
            &upload,
            &record,
            &credential_request,
            &credential_response,
            &finalization,
            &*pending,
        ] {
            assert!(m.len() <= MAX_MESSAGE_BYTES, "{} bytes", m.len());
        }
    }

    #[test]
    fn debug_never_prints_state() {
        let (client, _) = ClientLogin::start(&KEY).unwrap();
        assert_eq!(format!("{client:?}"), "ClientLogin(<redacted>)");
        assert_eq!(
            format!("{:?}", ServerKeys::generate()),
            "ServerKeys(<redacted>)"
        );
    }
}
