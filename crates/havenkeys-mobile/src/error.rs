use havenkeys_client::ClientError;

/// What Kotlin sees: `MobileException.Failed(code, detail)`. The code is
/// stable (the app matches on it); the detail is a fixed sentence. Neither
/// ever carries a secret.
#[derive(Debug, uniffi::Error)]
pub enum MobileError {
    Failed { code: String, detail: String },
}

pub type MobileResult<T> = Result<T, MobileError>;

impl std::fmt::Display for MobileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed { detail, .. } => f.write_str(detail),
        }
    }
}

impl std::error::Error for MobileError {}

impl From<ClientError> for MobileError {
    fn from(e: ClientError) -> Self {
        Self::Failed {
            code: e.code.to_owned(),
            detail: e.message,
        }
    }
}

impl From<havenkeys_core::Error> for MobileError {
    fn from(e: havenkeys_core::Error) -> Self {
        ClientError::from(e).into()
    }
}

impl MobileError {
    pub(crate) fn internal() -> Self {
        ClientError::internal().into()
    }
}
