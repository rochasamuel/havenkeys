//! Event names for the server session, and what the browser extension is told when a save fails.

use crate::state::CmdError;

/// A pull finished; the UI re-reads the item list.
pub const SYNCED_EVENT: &str = "vault://synced";
/// Online or offline changed.
pub const CONNECTIVITY_EVENT: &str = "vault://connectivity";
/// The server refused this device's sign-in after an unlock, or refused a
/// session it had accepted. Most often the master password was changed on
/// another device, which ends every other session; the UI explains that
/// instead of a bare "offline".
pub const SIGNED_OUT_EVENT: &str = "vault://signed-out";

/// What the browser extension is told when a save fails. The extension gets
/// a code and a fixed message, never the server's words.
pub fn bridge_error(err: CmdError) -> havenkeys_protocol::ErrorCode {
    use havenkeys_protocol::ErrorCode;
    match err.code {
        "offline" | "signed_out" => ErrorCode::Offline,
        "locked" => ErrorCode::Locked,
        "denied" => ErrorCode::Denied,
        "not_found" | "item_changed_elsewhere" => ErrorCode::NotFound,
        "invalid_input" => ErrorCode::InvalidInput,
        "rate_limited" => ErrorCode::RateLimited,
        _ => ErrorCode::Internal,
    }
}
