//! Session lock flag from the Windows Terminal Services API.
//!
//! `WTSQuerySessionInformationW(WTSSessionInfoEx)` allocates a `WTSINFOEXW`
//! whose level-1 `SessionFlags` is `WTS_SESSIONSTATE_LOCK` while the
//! workstation is locked. (Windows 7 reports the two values swapped; Windows
//! 7 cannot run the app's WebView2 runtime anyway.)

#![allow(unsafe_code)]

use std::mem::size_of;
use std::ptr;
use windows_sys::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW,
    WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK, WTS_SESSIONSTATE_UNLOCK,
};

pub struct Probe;

impl Probe {
    pub fn new() -> Self {
        Probe
    }

    pub fn locked(&mut self) -> Option<bool> {
        let mut buffer: *mut u16 = ptr::null_mut();
        let mut len: u32 = 0;
        // SAFETY: documented call with valid out-pointers. On success the
        // system allocates `buffer` (`len` bytes), which we free below.
        let ok = unsafe {
            WTSQuerySessionInformationW(
                WTS_CURRENT_SERVER_HANDLE,
                WTS_CURRENT_SESSION,
                WTSSessionInfoEx,
                &mut buffer,
                &mut len,
            )
        };
        if ok == 0 || buffer.is_null() {
            return None;
        }
        let state = if len as usize >= size_of::<WTSINFOEXW>() {
            // SAFETY: the buffer holds at least one WTSINFOEXW (checked
            // above); read it unaligned so no alignment is assumed.
            let info = unsafe { ptr::read_unaligned(buffer.cast::<WTSINFOEXW>()) };
            if info.Level == 1 {
                // SAFETY: Level 1 means the level-1 union member is active.
                let flags = unsafe { info.Data.WTSInfoExLevel1.SessionFlags } as u32;
                match flags {
                    WTS_SESSIONSTATE_LOCK => Some(true),
                    WTS_SESSIONSTATE_UNLOCK => Some(false),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        // SAFETY: `buffer` came from WTSQuerySessionInformationW and is freed once.
        unsafe { WTSFreeMemory(buffer.cast()) };
        state
    }
}
