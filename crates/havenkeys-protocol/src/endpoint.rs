//! Where the desktop app listens for the native host.
//!
//! * **Unix:** a Unix domain socket at `<runtime dir>/havenkeys/bridge.sock`.
//!   The `havenkeys` directory must be owned by the current user and have no
//!   group/other permissions; both the server and the client check this, so
//!   neither will use a directory another user prepared. The server also
//!   checks each peer's effective UID.
//! * **Windows:** a named pipe `\\.\pipe\havenkeys-bridge-<hash of profile
//!   path>` with an explicit DACL that grants access to the pipe's owner only
//!   (the default DACL would let every user open it for reading). See
//!   docs/native-messaging.md for the pipe-squatting caveat.
//!
//! Linux abstract-namespace sockets are deliberately not used: they have no
//! file permissions, so any local user could connect.

use interprocess::local_socket::{prelude::*, ListenerOptions, Name, Stream};
use std::io;
#[cfg(unix)]
use std::path::PathBuf;

/// Debug builds only: overrides the endpoint (used by integration tests).
/// Ignored in release builds so a modified environment cannot redirect the
/// native host.
pub const ENDPOINT_OVERRIDE_ENV: &str = "HAVENKEYS_BRIDGE_ENDPOINT";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    #[cfg(unix)]
    path: PathBuf,
    #[cfg(windows)]
    pipe: String,
}

fn override_from_env() -> Option<String> {
    if cfg!(debug_assertions) {
        std::env::var(ENDPOINT_OVERRIDE_ENV)
            .ok()
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

#[cfg(unix)]
impl Endpoint {
    /// The endpoint for the current user.
    pub fn for_current_user() -> io::Result<Self> {
        if let Some(p) = override_from_env() {
            return Ok(Self::at(PathBuf::from(p)));
        }
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                // macOS: per-user, mode 0700, set by launchd for GUI apps.
                cfg!(target_os = "macos")
                    .then(|| std::env::var_os("TMPDIR").map(PathBuf::from))
                    .flatten()
                    .filter(|p| p.is_absolute())
            })
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|h| PathBuf::from(h).join(".cache"))
                    .filter(|p| p.is_absolute())
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no runtime directory"))?;
        Ok(Self::at(base.join("havenkeys").join("bridge.sock")))
    }

    /// An endpoint at an explicit socket path (tests).
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn name(&self) -> io::Result<Name<'_>> {
        self.path
            .as_path()
            .to_fs_name::<interprocess::local_socket::GenericFilePath>()
    }

    /// Listener options for serving on this endpoint. The socket's
    /// directory must already be private (the server prepares it).
    pub fn listener_options(&self) -> io::Result<ListenerOptions<'_>> {
        Ok(ListenerOptions::new().name(self.name()?).reclaim_name(true))
    }

    /// Connect as a client, after checking the socket's directory is private.
    pub fn connect(&self) -> io::Result<Stream> {
        let dir = self
            .path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no parent"))?;
        check_private_dir(dir)?;
        Stream::connect(self.name()?)
    }
}

#[cfg(windows)]
impl Endpoint {
    /// The endpoint for the current user.
    pub fn for_current_user() -> io::Result<Self> {
        if let Some(p) = override_from_env() {
            return Ok(Self { pipe: p });
        }
        // Named after the profile directory, which is unique per user on a
        // machine and, unlike a filtered USERNAME, never collapses to the same
        // string for two users. Not a secret and not a security boundary: the
        // pipe's DACL is.
        use sha2::{Digest, Sha256};
        let profile = std::env::var_os("USERPROFILE")
            .filter(|p| !p.is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no user profile"))?;
        let digest = Sha256::digest(profile.to_string_lossy().to_lowercase().as_bytes());
        let tag: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        Ok(Self {
            pipe: format!("havenkeys-bridge-{tag}"),
        })
    }

    /// Listener options: the pipe grants access to its owner (the user
    /// running the desktop app) and nobody else, not even read access.
    pub fn listener_options(&self) -> io::Result<ListenerOptions<'_>> {
        use interprocess::os::windows::local_socket::ListenerOptionsExt;
        use interprocess::os::windows::security_descriptor::SecurityDescriptor;
        // P = protected (no inherited ACEs); OW = OWNER RIGHTS.
        let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;OW)")
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "sddl"))?;
        let sd = SecurityDescriptor::deserialize(&sddl)?;
        Ok(ListenerOptions::new()
            .name(self.name()?)
            .reclaim_name(true)
            .security_descriptor(sd))
    }

    pub fn name(&self) -> io::Result<Name<'_>> {
        self.pipe
            .as_str()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    }

    pub fn connect(&self) -> io::Result<Stream> {
        Stream::connect(self.name()?)
    }
}

/// Our effective UID.
#[cfg(unix)]
pub fn current_euid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// `dir` must be a real directory (not a symlink), owned by us, with no
/// group or other permission bits.
#[cfg(unix)]
pub fn check_private_dir(dir: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(dir)?;
    if !meta.is_dir() || meta.uid() != current_euid() || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket directory is not private",
        ));
    }
    Ok(())
}
