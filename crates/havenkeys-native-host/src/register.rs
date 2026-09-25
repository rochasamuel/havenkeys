//! Registers the native host with the user's browsers, so an installed
//! desktop app is all the extension needs.
//!
//! A browser finds a native host through a small JSON manifest (the host's
//! path and the one extension allowed to start it). On Linux and macOS the
//! manifest sits in a per-browser directory; on Windows a registry key under
//! `HKCU` points to it. Everything here is per user: nothing needs
//! administrator rights, and nothing outside our own manifest files and
//! registry keys is touched.
//!
//! The manifests only ever name our own Chrome Web Store and Firefox add-on
//! IDs, the same IDs `caller_allowed` checks again at launch.

use crate::{CHROME_EXTENSION_ORIGIN, FIREFOX_EXTENSION_ID};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The native messaging host name the extension connects to.
pub const HOST_NAME: &str = "com.havenkeys.bridge";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Chromium,
    Firefox,
}

/// The manifest for one browser family. `serde_json` does the escaping, so
/// any path (spaces, backslashes, non-ASCII) is written correctly.
pub fn manifest_json(host: &Path, kind: Kind) -> String {
    let mut m = serde_json::json!({
        "name": HOST_NAME,
        "description": "HavenKeys desktop bridge",
        "path": host.to_string_lossy(),
        "type": "stdio",
    });
    match kind {
        Kind::Chromium => m["allowed_origins"] = serde_json::json!([CHROME_EXTENSION_ORIGIN]),
        Kind::Firefox => m["allowed_extensions"] = serde_json::json!([FIREFOX_EXTENSION_ID]),
    }
    let mut s = serde_json::to_string_pretty(&m).unwrap_or_default();
    s.push('\n');
    s
}

/// A browser profile directory and where its manifest goes, for Linux and
/// macOS. The manifest is written only when `profile` exists, so browsers
/// the user does not have get no directories created for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub browser: &'static str,
    pub kind: Kind,
    pub profile: PathBuf,
    pub manifest: PathBuf,
}

fn chromium(browser: &'static str, profile: PathBuf) -> Target {
    let manifest = profile
        .join("NativeMessagingHosts")
        .join(format!("{HOST_NAME}.json"));
    Target {
        browser,
        kind: Kind::Chromium,
        profile,
        manifest,
    }
}

/// Linux: the XDG config directory (`$XDG_CONFIG_HOME`, else `~/.config`)
/// for Chromium browsers, `~/.mozilla` for Firefox.
pub fn linux_targets(home: &Path, config: &Path) -> Vec<Target> {
    let mut t = vec![
        chromium("Google Chrome", config.join("google-chrome")),
        chromium("Google Chrome Beta", config.join("google-chrome-beta")),
        chromium("Chromium", config.join("chromium")),
        chromium("Brave", config.join("BraveSoftware/Brave-Browser")),
        chromium("Microsoft Edge", config.join("microsoft-edge")),
        chromium("Vivaldi", config.join("vivaldi")),
    ];
    t.push(Target {
        browser: "Firefox",
        kind: Kind::Firefox,
        profile: home.join(".mozilla"),
        manifest: home
            .join(".mozilla/native-messaging-hosts")
            .join(format!("{HOST_NAME}.json")),
    });
    t
}

/// macOS: everything lives under `~/Library/Application Support`.
pub fn macos_targets(home: &Path) -> Vec<Target> {
    let s = home.join("Library/Application Support");
    let mut t = vec![
        chromium("Google Chrome", s.join("Google/Chrome")),
        chromium("Google Chrome Beta", s.join("Google/Chrome Beta")),
        chromium("Chromium", s.join("Chromium")),
        chromium("Brave", s.join("BraveSoftware/Brave-Browser")),
        chromium("Microsoft Edge", s.join("Microsoft Edge")),
        chromium("Vivaldi", s.join("Vivaldi")),
    ];
    t.push(Target {
        browser: "Firefox",
        kind: Kind::Firefox,
        profile: s.join("Mozilla"),
        manifest: s
            .join("Mozilla/NativeMessagingHosts")
            .join(format!("{HOST_NAME}.json")),
    });
    t
}

/// Write `contents` to `path` unless it already holds exactly that. The new
/// file is written beside the old one and renamed over it, so a browser
/// never reads half a manifest.
pub fn write_if_changed(path: &Path, contents: &str) -> io::Result<()> {
    if fs::read(path).is_ok_and(|old| old == contents.as_bytes()) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644))?;
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Register `host` for every target whose browser profile exists. Returns
/// the browsers registered; a target that fails is skipped, not fatal.
pub fn register_targets(host: &Path, targets: &[Target]) -> Vec<&'static str> {
    targets
        .iter()
        .filter(|t| t.profile.is_dir())
        .filter(|t| write_if_changed(&t.manifest, &manifest_json(host, t.kind)).is_ok())
        .map(|t| t.browser)
        .collect()
}

/// Register `host` with this user's browsers. Returns the browsers
/// registered (on Windows, every browser we write a key for).
#[cfg(target_os = "linux")]
pub fn register(host: &Path) -> io::Result<Vec<&'static str>> {
    let home = home()?;
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    Ok(register_targets(host, &linux_targets(&home, &config)))
}

#[cfg(target_os = "macos")]
pub fn register(host: &Path) -> io::Result<Vec<&'static str>> {
    Ok(register_targets(host, &macos_targets(&home()?)))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn home() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))
}

/// Windows: the manifests live in `%LOCALAPPDATA%\HavenKeys`, and each
/// browser's `HKCU\Software\…\NativeMessagingHosts\<name>` key names one.
/// Keys are written for every supported browser, installed or not: a key
/// for a browser that is absent is never read.
#[cfg(windows)]
pub fn register(host: &Path) -> io::Result<Vec<&'static str>> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const CHROMIUM: &[(&str, &str)] = &[
        ("Google Chrome", r"Software\Google\Chrome"),
        ("Microsoft Edge", r"Software\Microsoft\Edge"),
        ("Brave", r"Software\BraveSoftware\Brave-Browser"),
        ("Chromium", r"Software\Chromium"),
        ("Vivaldi", r"Software\Vivaldi"),
    ];

    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no LOCALAPPDATA"))?
        .join("HavenKeys");
    let chrome_manifest = base.join(format!("{HOST_NAME}.chrome.json"));
    let firefox_manifest = base.join(format!("{HOST_NAME}.firefox.json"));
    write_if_changed(&chrome_manifest, &manifest_json(host, Kind::Chromium))?;
    write_if_changed(&firefox_manifest, &manifest_json(host, Kind::Firefox))?;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let set = |key: &str, manifest: &Path| -> io::Result<()> {
        let (k, _) = hkcu.create_subkey(format!(r"{key}\NativeMessagingHosts\{HOST_NAME}"))?;
        let value = manifest.to_string_lossy().into_owned();
        if k.get_value::<String, _>("").ok().as_deref() != Some(value.as_str()) {
            k.set_value("", &value)?;
        }
        Ok(())
    };
    let mut done = Vec::new();
    for (browser, key) in CHROMIUM {
        if set(key, &chrome_manifest).is_ok() {
            done.push(*browser);
        }
    }
    if set(r"Software\Mozilla", &firefox_manifest).is_ok() {
        done.push("Firefox");
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_name_only_our_extension() {
        let host = Path::new("/opt/Haven Keys/havenkeys-native-host");
        let c: serde_json::Value =
            serde_json::from_str(&manifest_json(host, Kind::Chromium)).unwrap();
        assert_eq!(c["name"], HOST_NAME);
        assert_eq!(c["type"], "stdio");
        assert_eq!(c["path"], "/opt/Haven Keys/havenkeys-native-host");
        assert_eq!(
            c["allowed_origins"],
            serde_json::json!([CHROME_EXTENSION_ORIGIN])
        );
        assert!(c.get("allowed_extensions").is_none());

        let f: serde_json::Value =
            serde_json::from_str(&manifest_json(host, Kind::Firefox)).unwrap();
        assert_eq!(
            f["allowed_extensions"],
            serde_json::json!([FIREFOX_EXTENSION_ID])
        );
        assert!(f.get("allowed_origins").is_none());
    }

    #[test]
    fn paths_that_need_escaping_stay_valid_json() {
        let host = Path::new(r#"C:\Users\A "quoted" name\havenkeys-native-host.exe"#);
        let v: serde_json::Value =
            serde_json::from_str(&manifest_json(host, Kind::Chromium)).unwrap();
        assert_eq!(
            v["path"],
            r#"C:\Users\A "quoted" name\havenkeys-native-host.exe"#
        );
    }

    #[test]
    fn registers_only_browsers_that_are_present() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join(".config");
        fs::create_dir_all(config.join("google-chrome")).unwrap();
        fs::create_dir_all(home.path().join(".mozilla")).unwrap();
        let host = home.path().join("havenkeys-native-host");

        let targets = linux_targets(home.path(), &config);
        let done = register_targets(&host, &targets);
        assert_eq!(done, vec!["Google Chrome", "Firefox"]);

        let chrome = config.join("google-chrome/NativeMessagingHosts/com.havenkeys.bridge.json");
        let firefox = home
            .path()
            .join(".mozilla/native-messaging-hosts/com.havenkeys.bridge.json");
        assert_eq!(
            fs::read_to_string(&chrome).unwrap(),
            manifest_json(&host, Kind::Chromium)
        );
        assert_eq!(
            fs::read_to_string(&firefox).unwrap(),
            manifest_json(&host, Kind::Firefox)
        );
        // No directories were created for absent browsers.
        assert!(!config.join("chromium").exists());
        assert!(!config.join("BraveSoftware").exists());
    }

    #[test]
    fn rewrites_a_stale_manifest_and_leaves_a_current_one_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.json");
        fs::write(&path, "{\"path\":\"/old/host\"}").unwrap();
        write_if_changed(&path, "new\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new\n");

        let before = fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_if_changed(&path, "new\n").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
        assert!(!dir.path().join("m.json.tmp").exists());
    }

    #[cfg(unix)]
    #[test]
    fn manifests_are_world_readable_not_writable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.json");
        write_if_changed(&path, "x\n").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn macos_paths_match_the_browsers_lookup_locations() {
        let t = macos_targets(Path::new("/Users/u"));
        let ff = t.iter().find(|t| t.kind == Kind::Firefox).unwrap();
        assert_eq!(
            ff.manifest,
            Path::new("/Users/u/Library/Application Support/Mozilla/NativeMessagingHosts/com.havenkeys.bridge.json")
        );
        let chrome = t.iter().find(|t| t.browser == "Google Chrome").unwrap();
        assert_eq!(
            chrome.manifest,
            Path::new("/Users/u/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.havenkeys.bridge.json")
        );
    }
}
