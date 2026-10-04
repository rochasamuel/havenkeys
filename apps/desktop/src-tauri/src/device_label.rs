//! The name this computer shows in the account's device list and on the
//! phone that approves it: "DESKTOP-SAMS (Windows)".

/// The computer's name with the OS after it. A fully qualified name
/// ("laptop.local", "host.example.com") keeps only its first label.
pub fn device_label() -> String {
    let host = gethostname::gethostname();
    let host = host.to_string_lossy();
    let name = host.split('.').next().unwrap_or_default();
    havenkeys_client::device_label(name, "Desktop", os_name())
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "linux" => "Linux",
        "windows" => "Windows",
        "macos" => "macOS",
        other => other,
    }
}
