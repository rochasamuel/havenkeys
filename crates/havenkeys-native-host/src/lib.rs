//! Native messaging host: relays between the browser extension (stdio) and
//! the running desktop app (local socket).
//!
//! The host holds no keys and cannot open the vault. It exists because
//! browsers only talk to native code through a process they launch. It
//! still validates everything that passes through, in both directions:
//!
//! * requests are parsed into the typed protocol and re-encoded, so only
//!   well-formed, known requests reach the desktop;
//! * desktop messages are parsed and re-encoded too, with error text replaced
//!   by fixed strings, so the extension only ever receives typed messages.
//!
//! The desktop re-validates every request; nothing here is trusted for
//! authorization. Nothing in this crate logs, and stdout carries only frames.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

use havenkeys_protocol::frame::{discard, read_frame, write_frame, FrameError};
use havenkeys_protocol::{
    parse_request, ErrorCode, Event, Outgoing, Response, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
};
use interprocess::local_socket::{prelude::*, SendHalf, Stream};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// Extension IDs allowed to launch the host. The browser enforces the same
/// list through `allowed_origins` / `allowed_extensions` in the host
/// manifest; this is a second check.
pub const CHROME_EXTENSION_ORIGIN: &str = "chrome-extension://olbclkanfbmilnmfhoojgcnpgdmilfmf/";
pub const FIREFOX_EXTENSION_ID: &str = "havenkeys@havenkeys.app";

/// Chrome passes the caller's origin as the first argument; Firefox passes
/// the manifest path and then the extension ID.
pub fn caller_allowed<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> bool {
    args.iter()
        .skip(1)
        .take(2)
        .any(|a| a.as_ref() == CHROME_EXTENSION_ORIGIN || a.as_ref() == FIREFOX_EXTENSION_ID)
}

type Output<W> = Arc<Mutex<W>>;

fn emit<W: Write>(out: &Output<W>, msg: Outgoing) -> bool {
    let Some(bytes) = msg.to_bytes() else {
        return false;
    };
    let Ok(mut w) = out.lock() else {
        return false;
    };
    write_frame(&mut *w, &bytes, MAX_RESPONSE_BYTES).is_ok()
}

struct Upstream {
    send: SendHalf,
    alive: Arc<AtomicBool>,
}

/// Relay until the extension closes stdin. `connect` opens a connection to
/// the desktop app; it is retried on each request while disconnected.
pub fn run<R, W, C>(mut input: R, output: W, connect: C)
where
    R: Read,
    W: Write + Send + 'static,
    C: Fn() -> io::Result<Stream>,
{
    let out: Output<W> = Arc::new(Mutex::new(output));
    let mut upstream: Option<Upstream> = None;

    loop {
        let bytes = match read_frame(&mut input, MAX_REQUEST_BYTES) {
            Ok(Some(b)) => b,
            Ok(None) | Err(FrameError::Io) => return,
            // A6: skip the payload without buffering it, answer, carry on.
            Err(FrameError::TooLarge(n)) => {
                if discard(&mut input, n).is_err()
                    || !emit(&out, Response::err(None, ErrorCode::TooLarge).into())
                {
                    return;
                }
                continue;
            }
        };
        // A5: anything that is not a known, well-formed request stops here.
        let env = match parse_request(&bytes) {
            Ok(env) => env,
            Err(rejection) => {
                if !emit(&out, rejection.response().into()) {
                    return;
                }
                continue;
            }
        };
        let Ok(canonical) = serde_json::to_vec(&env).map(Zeroizing::new) else {
            continue;
        };

        if upstream.as_ref().is_some_and(|u| !u.alive.load(Ordering::SeqCst)) {
            upstream = None;
        }
        if upstream.is_none() {
            upstream = connect().ok().and_then(|s| start_downstream(s, &out));
        }
        let sent = upstream
            .as_mut()
            .is_some_and(|u| write_frame(&mut u.send, &canonical, MAX_REQUEST_BYTES).is_ok());
        if !sent {
            upstream = None;
            if !emit(&out, Response::err(Some(env.id), ErrorCode::DesktopUnavailable).into()) {
                return;
            }
        }
    }
}

/// Split the connection and forward validated desktop messages to `out` on
/// a background thread.
fn start_downstream<W: Write + Send + 'static>(stream: Stream, out: &Output<W>) -> Option<Upstream> {
    let (mut recv, send) = stream.split();
    let alive = Arc::new(AtomicBool::new(true));
    let (flag, out) = (alive.clone(), out.clone());
    std::thread::Builder::new()
        .name("host-downstream".into())
        .spawn(move || {
            while let Ok(Some(bytes)) = read_frame(&mut recv, MAX_RESPONSE_BYTES) {
                // Invalid messages from the desktop are dropped, not forwarded.
                if let Some(msg) = Outgoing::parse(&bytes) {
                    if !emit(&out, msg) {
                        break;
                    }
                }
            }
            flag.store(false, Ordering::SeqCst);
            emit(&out, Event::Disconnected {}.into());
        })
        .ok()?;
    Some(Upstream { send, alive })
}
