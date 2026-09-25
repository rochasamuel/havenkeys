//! Connects the browsers on this computer to the installed app.
//!
//! The installers ship `havenkeys-native-host` next to the app binary
//! (Tauri `externalBin`). At every start the app registers it with the
//! user's browsers (`havenkeys_native_host::register`), so installing the app
//! is all the extension needs, and a moved or updated install fixes its own
//! registration. Registering grants nothing by itself: the host is a relay,
//! the manifests admit only our extension IDs, and the bridge still refuses
//! page requests while locked or while Settings → Browser extension is off.
//!
//! Where the app runs from a path that will not exist later (an AppImage's
//! temporary mount, or a macOS app Gatekeeper translocated because it was
//! opened from Downloads), the host is first copied to the data directory
//! and that copy is registered instead.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const HOST_FILE: &str = if cfg!(windows) {
    "havenkeys-native-host.exe"
} else {
    "havenkeys-native-host"
};

/// Register the bundled host, off the calling thread. Best effort: without
/// it the extension says "not connected", and the app works as before.
pub fn register_in_background(data_dir: PathBuf) {
    let _ = std::thread::Builder::new()
        .name("native-host-register".into())
        .spawn(move || {
            if let Some(host) = host_path(&data_dir) {
                let _ = havenkeys_native_host::register::register(&host);
            }
        });
}

/// The host binary to register, if this install has one.
fn host_path(data_dir: &Path) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundled = exe.parent()?.join(HOST_FILE);
    if !bundled.is_file() {
        // A build without the sidecar (`tauri dev`, `cargo run`): leave
        // whatever the developer registered by hand alone.
        return None;
    }
    if runs_from_transient_path(&exe) {
        copy_host(&bundled, &data_dir.join("native-host").join(HOST_FILE)).ok()
    } else {
        Some(bundled)
    }
}

fn runs_from_transient_path(exe: &Path) -> bool {
    (cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_some())
        || (cfg!(target_os = "macos") && exe.to_string_lossy().contains("/AppTranslocation/"))
}

/// Copy the host to `dest` unless an identical copy is already there.
fn copy_host(src: &Path, dest: &Path) -> io::Result<PathBuf> {
    let bytes = fs::read(src)?;
    if fs::read(dest).ok().as_deref() != Some(bytes.as_slice()) {
        if let Some(dir) = dest.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = dest.with_extension("tmp");
        fs::write(&tmp, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
        }
        fs::rename(&tmp, dest)?;
    }
    Ok(dest.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_host_copies_once_and_replaces_a_different_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src-host");
        let dest = dir.path().join("native-host").join(HOST_FILE);
        fs::write(&src, b"v1").unwrap();

        assert_eq!(copy_host(&src, &dest).unwrap(), dest);
        assert_eq!(fs::read(&dest).unwrap(), b"v1");

        fs::write(&src, b"v2").unwrap();
        copy_host(&src, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"v2");
        assert!(!dest.with_extension("tmp").exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o755);
        }
    }
}
