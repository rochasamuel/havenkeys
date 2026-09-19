//! Entry point launched by the browser. See `lib.rs`.

#![forbid(unsafe_code)]

use havenkeys_native_host::{caller_allowed, run};
use havenkeys_protocol::endpoint::Endpoint;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Panic messages would go to stderr, which browsers write to their logs.
    // Ours never contain secrets, but say nothing anyway.
    std::panic::set_hook(Box::new(|_| {}));

    let args: Vec<_> = std::env::args_os().collect();
    if !caller_allowed(&args) {
        return ExitCode::from(2);
    }
    let Ok(endpoint) = Endpoint::for_current_user() else {
        return ExitCode::from(3);
    };
    run(std::io::stdin().lock(), std::io::stdout(), || endpoint.connect());
    ExitCode::SUCCESS
}
