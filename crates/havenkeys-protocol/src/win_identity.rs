//! Windows: does a process run as the current user? Used on both ends of the
//! bridge pipe, because another account can create the pipe name before
//! HavenKeys does (security-review BR-1). Compares token *user* SIDs, so an
//! elevated desktop app and an unelevated native host still match. Any
//! failure, including a process we may not open, as another user's is,
//! answers `Err`, which callers treat as "not us".

#![allow(unsafe_code)]

use std::io;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a handle this module opened and still owns.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// The process's `TOKEN_USER`, in a buffer aligned for it.
fn token_user(process: HANDLE) -> io::Result<Vec<u64>> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: `process` is a valid process handle; `token` receives a new handle.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Owned(token);
    let mut len = 0u32;
    // SAFETY: a size query with no buffer; it fails and sets `len`.
    unsafe { GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut len) };
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes, aligned for TOKEN_USER.
    if unsafe { GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), len, &mut len) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(buf)
}

pub fn process_is_current_user(pid: u32) -> io::Result<bool> {
    // SAFETY: plain call; a null result is checked.
    let peer = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if peer.is_null() {
        return Err(io::Error::last_os_error());
    }
    let peer = Owned(peer);
    let theirs = token_user(peer.0)?;
    // SAFETY: the current-process pseudo handle needs no closing.
    let ours = token_user(unsafe { GetCurrentProcess() })?;
    let sid = |b: &[u64]| {
        // SAFETY: `b` was filled by GetTokenInformation(TokenUser); the SID
        // it points to lives inside the same buffer.
        unsafe { (*b.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    };
    // SAFETY: two valid SIDs, each inside its live buffer.
    Ok(unsafe { EqualSid(sid(&theirs), sid(&ours)) } != 0)
}
