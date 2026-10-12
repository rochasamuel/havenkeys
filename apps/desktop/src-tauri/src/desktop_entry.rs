//! Gives the AppImage its icon in the GNOME and KDE docks and app menus.
//!
//! A running AppImage is unknown to the desktop: the docks find a window's
//! icon through an installed `.desktop` file whose name (KDE) or
//! `StartupWMClass` (GNOME) matches the window's app id, and on Wayland
//! they ignore the icon the window sets itself. The `.deb` and `.rpm`
//! install those files system-wide; the AppImage carries them inside its
//! image, where nothing looks.
//!
//! So at every start from an AppImage the app writes, under the user's
//! XDG data directory, `applications/havenkeys-desktop.desktop` (starting
//! the image at `$APPIMAGE`) and its icon in `icons/hicolor`. Files already
//! up to date are left alone; a moved image rewrites the entry. `TryExec`
//! hides the entry once the image is deleted. Nothing here is a secret.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The window's app id on Linux (the binary name: GTK's program name) and
/// the icon name, as the bundled `.desktop` file uses them.
const APP_ID: &str = "havenkeys-desktop";

const ICONS: &[(&str, &[u8])] = &[
    ("32x32", include_bytes!("../icons/32x32.png")),
    ("64x64", include_bytes!("../icons/64x64.png")),
    ("128x128", include_bytes!("../icons/128x128.png")),
    ("256x256", include_bytes!("../icons/128x128@2x.png")),
    ("512x512", include_bytes!("../icons/icon.png")),
];

/// Register the AppImage, off the calling thread. Best effort: without it
/// the dock shows a generic icon, and the app works as before.
pub fn install_in_background() {
    let Some(image) = std::env::var_os("APPIMAGE").map(PathBuf::from) else {
        return;
    };
    let Some(data_home) = data_home() else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("desktop-entry".into())
        .spawn(move || {
            let _ = install(&data_home, &image);
        });
}

/// `$XDG_DATA_HOME`, or `~/.local/share` when unset or not absolute.
fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            let home = PathBuf::from(std::env::var_os("HOME")?);
            home.is_absolute().then(|| home.join(".local/share"))
        })
}

fn install(data_home: &Path, image: &Path) -> io::Result<()> {
    // Icons first, so the entry never names an icon that is not there yet.
    for (size, png) in ICONS {
        let path = data_home
            .join("icons/hicolor")
            .join(size)
            .join("apps")
            .join(format!("{APP_ID}.png"));
        write_if_changed(&path, png)?;
    }
    let entry = desktop_entry(image).ok_or_else(|| io::Error::other("unusable path"))?;
    write_if_changed(
        &data_home
            .join("applications")
            .join(format!("{APP_ID}.desktop")),
        entry.as_bytes(),
    )
}

/// The entry for the image at `image`, or `None` for a path the format
/// cannot hold (not UTF-8, or a control character).
fn desktop_entry(image: &Path) -> Option<String> {
    let path = image.to_str()?;
    if path.chars().any(char::is_control) {
        return None;
    }
    Some(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=HavenKeys\n\
         Comment=Local-first password manager\n\
         Categories=Utility;\n\
         Exec={}\n\
         TryExec={}\n\
         Icon={APP_ID}\n\
         StartupWMClass={APP_ID}\n\
         Terminal=false\n",
        escape_value(&quote_exec_arg(path)),
        escape_value(path),
    ))
}

/// One `Exec` argument, quoted as the Desktop Entry spec requires: inside
/// double quotes, `"`, `` ` ``, `$` and `\` take a backslash, and `%` is
/// doubled so it is not read as a field code.
fn quote_exec_arg(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A string value: the spec's escapes apply before `Exec` quoting is read,
/// so every backslash is written twice.
fn escape_value(value: &str) -> String {
    value.replace('\\', "\\\\")
}

/// Write `bytes` to `path` through a temporary file, unless it already
/// holds them.
fn write_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_arguments_are_quoted_and_escaped() {
        assert_eq!(
            quote_exec_arg("/opt/HavenKeys.AppImage"),
            "\"/opt/HavenKeys.AppImage\""
        );
        assert_eq!(
            quote_exec_arg("/home/a b/$x`y\"z\\100%.AppImage"),
            "\"/home/a b/\\$x\\`y\\\"z\\\\100%%.AppImage\""
        );
    }

    #[test]
    fn entry_matches_the_window_and_starts_the_image() {
        let entry = desktop_entry(Path::new("/home/me/Apps/Haven Keys.AppImage")).unwrap();
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("\nExec=\"/home/me/Apps/Haven Keys.AppImage\"\n"));
        assert!(entry.contains("\nTryExec=/home/me/Apps/Haven Keys.AppImage\n"));
        assert!(entry.contains("\nIcon=havenkeys-desktop\n"));
        assert!(entry.contains("\nStartupWMClass=havenkeys-desktop\n"));
    }

    #[test]
    fn backslashes_are_escaped_twice_in_exec() {
        let entry = desktop_entry(Path::new("/a\\b")).unwrap();
        assert!(entry.contains("\nExec=\"/a\\\\\\\\b\"\n"));
        assert!(entry.contains("\nTryExec=/a\\\\b\n"));
    }

    #[test]
    fn paths_with_control_characters_are_refused() {
        assert!(desktop_entry(Path::new("/a\nExec=evil")).is_none());
    }

    #[test]
    fn install_writes_the_entry_and_icons_and_follows_a_moved_image() {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), Path::new("/old/HavenKeys.AppImage")).unwrap();

        let entry_path = dir.path().join("applications/havenkeys-desktop.desktop");
        let entry = fs::read_to_string(&entry_path).unwrap();
        assert!(entry.contains("Exec=\"/old/HavenKeys.AppImage\""));
        for (size, png) in ICONS {
            let icon = dir
                .path()
                .join(format!("icons/hicolor/{size}/apps/havenkeys-desktop.png"));
            assert_eq!(fs::read(icon).unwrap(), *png);
        }

        install(dir.path(), Path::new("/new/HavenKeys.AppImage")).unwrap();
        let entry = fs::read_to_string(&entry_path).unwrap();
        assert!(entry.contains("Exec=\"/new/HavenKeys.AppImage\""));
        assert!(!entry_path.with_extension("tmp").exists());
    }
}
