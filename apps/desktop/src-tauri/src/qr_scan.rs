//! Reads a TOTP setup QR code from the clipboard image or, failing that, the
//! screen (docs/superpowers/specs/2026-09-27-totp-qr-scan-design.md).
//!
//! Pixels and decoded text live only in memory for the length of one scan.
//! Only strings that parse as `otpauth://totp` survive; anything else a QR
//! code on screen might say is dropped without being logged or returned.

use crate::state::CmdError;
use havenkeys_core::totp::{parse_totp_input, TotpAlgorithm};
use havenkeys_core::SecretString;
use zeroize::Zeroizing;

/// Upper bound on the pixels decoded in one pass, all frames together:
/// three 4K monitors are about 25 million.
pub const MAX_PIXELS: u64 = 64_000_000;

/// One RGBA image, 4 bytes per pixel, rows top to bottom.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A TOTP setup found in a QR code. `uri` is the full `otpauth://` text.
pub struct ScannedCode {
    pub uri: SecretString,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScanError {
    /// No QR code in the clipboard image, if any, or on screen.
    NotFound,
    /// QR codes were found, but none is a TOTP setup.
    NotTotp,
    /// The screen could not be captured (no display, portal refused).
    Capture,
}

impl From<ScanError> for CmdError {
    fn from(e: ScanError) -> Self {
        match e {
            // macOS returns only the wallpaper when Screen Recording is
            // denied, so "nothing found" may really be "not allowed".
            ScanError::NotFound if cfg!(target_os = "macos") => CmdError {
                code: "qr_not_found_macos",
                message: "No QR code found. Make sure it's fully visible on screen. If this is the first scan, allow HavenKeys in System Settings → Privacy & Security → Screen Recording.".into(),
            },
            ScanError::NotFound => CmdError {
                code: "qr_not_found",
                message: "No QR code found. Make sure it's fully visible on screen.".into(),
            },
            ScanError::NotTotp => CmdError {
                code: "qr_not_totp",
                message: "The QR code isn't a one-time code setup.".into(),
            },
            ScanError::Capture => CmdError {
                code: "screen_capture",
                message: "Could not capture the screen.".into(),
            },
        }
    }
}

#[derive(Default)]
struct Decoded {
    codes: Vec<ScannedCode>,
    /// Some QR code was decoded, TOTP or not.
    saw_qr: bool,
}

/// Grey level of one RGBA pixel, composited over white so a transparent
/// background reads as paper rather than ink.
fn luma(px: &[u8]) -> u8 {
    let [r, g, b, a] = [px[0], px[1], px[2], px[3]].map(u32::from);
    let y = (r * 299 + g * 587 + b * 114) / 1000;
    ((y * a + 255 * (255 - a)) / 255) as u8
}

/// Every TOTP setup in `frames`, without repeats. Frames that are malformed
/// or would take the total past `max_pixels` are skipped.
fn decode_frames(frames: &[Frame], max_pixels: u64) -> Decoded {
    let mut out = Decoded::default();
    let mut seen: Vec<(Zeroizing<String>, TotpAlgorithm, u32, u32)> = Vec::new();
    let mut budget = max_pixels;
    for frame in frames {
        let (w, h) = (frame.width as usize, frame.height as usize);
        let pixels = u64::from(frame.width) * u64::from(frame.height);
        if w == 0 || h == 0 || pixels > budget {
            continue;
        }
        let Some(len) = w.checked_mul(h).and_then(|n| n.checked_mul(4)) else {
            continue;
        };
        if frame.rgba.len() != len {
            continue;
        }
        budget -= pixels;
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
            let i = (y * w + x) * 4;
            luma(&frame.rgba[i..i + 4])
        });
        for grid in image.detect_grids() {
            let Ok((_, text)) = grid.decode() else {
                continue;
            };
            out.saw_qr = true;
            let mut text = Zeroizing::new(text);
            let is_totp_uri = text
                .get(.."otpauth://totp".len())
                .is_some_and(|p| p.eq_ignore_ascii_case("otpauth://totp"));
            if !is_totp_uri {
                continue;
            }
            let Ok(config) = parse_totp_input(&text) else {
                continue;
            };
            let key = (
                Zeroizing::new(config.secret.expose().to_owned()),
                config.algorithm,
                config.digits,
                config.period,
            );
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            out.codes.push(ScannedCode {
                uri: SecretString::new(std::mem::take(&mut *text)),
                issuer: config.issuer.clone(),
                account: config.account.clone(),
            });
        }
    }
    out
}

/// The clipboard image first; the screen only if that gave no TOTP code.
/// Both sources are passed in so the order is testable without a display.
pub fn scan(
    clipboard: impl FnOnce() -> Option<Frame>,
    screens: impl FnOnce() -> Result<Vec<Frame>, ScanError>,
) -> Result<Vec<ScannedCode>, ScanError> {
    let mut saw_qr = false;
    if let Some(frame) = clipboard() {
        let found = decode_frames(std::slice::from_ref(&frame), MAX_PIXELS);
        drop(frame);
        if !found.codes.is_empty() {
            return Ok(found.codes);
        }
        saw_qr = found.saw_qr;
    }
    let frames = screens()?;
    let found = decode_frames(&frames, MAX_PIXELS);
    drop(frames);
    if !found.codes.is_empty() {
        Ok(found.codes)
    } else if saw_qr || found.saw_qr {
        Err(ScanError::NotTotp)
    } else {
        Err(ScanError::NotFound)
    }
}

/// The image on the clipboard, if there is one. Only read, never changed; an
/// unreadable clipboard counts as "no image" and the scan moves on.
pub fn clipboard_frame() -> Option<Frame> {
    let image = arboard::Clipboard::new().ok()?.get_image().ok()?;
    Some(Frame {
        width: u32::try_from(image.width).ok()?,
        height: u32::try_from(image.height).ok()?,
        rgba: image.bytes.into_owned(),
    })
}

/// One capture per monitor. A monitor that fails is skipped; none at all is
/// an error.
pub fn screen_frames() -> Result<Vec<Frame>, ScanError> {
    let monitors = xcap::Monitor::all().map_err(|_| ScanError::Capture)?;
    let frames: Vec<Frame> = monitors
        .iter()
        .filter_map(|m| m.capture_image().ok())
        .map(|image| Frame {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        })
        .collect();
    if frames.is_empty() {
        Err(ScanError::Capture)
    } else {
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const GITHUB: &str =
        "otpauth://totp/GitHub:alice?secret=JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP&issuer=GitHub";
    const GITLAB: &str =
        "otpauth://totp/GitLab:bob?secret=KRUGKIDROVUWG2ZAMJZG653OEBTG66BA&issuer=GitLab";

    /// A QR code for `text`, `scale` pixels per module, with the standard
    /// 4-module quiet zone, black on white.
    fn qr_frame(text: &str, scale: usize) -> Frame {
        let code = qrcode::QrCode::new(text.as_bytes()).unwrap();
        let n = code.width();
        let quiet = 4;
        let size = (n + 2 * quiet) * scale;
        let colors = code.to_colors();
        let mut rgba = vec![255u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let (mx, my) = (x / scale, y / scale);
                let inside = mx >= quiet && my >= quiet && mx < quiet + n && my < quiet + n;
                if inside && colors[(my - quiet) * n + (mx - quiet)] == qrcode::Color::Dark {
                    let i = (y * size + x) * 4;
                    rgba[i..i + 3].fill(0);
                }
            }
        }
        Frame {
            width: size as u32,
            height: size as u32,
            rgba,
        }
    }

    /// Two frames next to each other on one white canvas.
    fn side_by_side(a: &Frame, b: &Frame) -> Frame {
        let width = (a.width + b.width) as usize;
        let height = a.height.max(b.height) as usize;
        let mut rgba = vec![255u8; width * height * 4];
        for (frame, left) in [(a, 0usize), (b, a.width as usize)] {
            let w = frame.width as usize;
            for y in 0..frame.height as usize {
                let src = &frame.rgba[y * w * 4..(y + 1) * w * 4];
                let dst = (y * width + left) * 4;
                rgba[dst..dst + w * 4].copy_from_slice(src);
            }
        }
        Frame {
            width: width as u32,
            height: height as u32,
            rgba,
        }
    }

    fn blank(size: u32) -> Frame {
        Frame {
            width: size,
            height: size,
            rgba: vec![255u8; (size * size * 4) as usize],
        }
    }

    fn labels(codes: &[ScannedCode]) -> Vec<(Option<&str>, Option<&str>)> {
        codes
            .iter()
            .map(|c| (c.issuer.as_deref(), c.account.as_deref()))
            .collect()
    }

    #[test]
    fn one_totp_code_is_found_with_its_labels() {
        let found = decode_frames(&[qr_frame(GITHUB, 4)], MAX_PIXELS);
        assert!(found.saw_qr);
        assert_eq!(labels(&found.codes), [(Some("GitHub"), Some("alice"))]);
        assert_eq!(found.codes[0].uri.expose(), GITHUB);
    }

    #[test]
    fn upper_case_scheme_is_accepted() {
        let upper = GITHUB.replace("otpauth://totp", "OTPAUTH://TOTP");
        let found = decode_frames(&[qr_frame(&upper, 4)], MAX_PIXELS);
        assert_eq!(found.codes.len(), 1);
    }

    #[test]
    fn two_codes_are_both_found_and_a_repeat_is_dropped() {
        let two = side_by_side(&qr_frame(GITHUB, 4), &qr_frame(GITLAB, 4));
        let found = decode_frames(&[two], MAX_PIXELS);
        let mut got = labels(&found.codes);
        got.sort();
        assert_eq!(
            got,
            [
                (Some("GitHub"), Some("alice")),
                (Some("GitLab"), Some("bob"))
            ]
        );

        let same_twice = side_by_side(&qr_frame(GITHUB, 4), &qr_frame(GITHUB, 4));
        assert_eq!(decode_frames(&[same_twice], MAX_PIXELS).codes.len(), 1);
        let across_frames = [qr_frame(GITHUB, 4), qr_frame(GITHUB, 3)];
        assert_eq!(decode_frames(&across_frames, MAX_PIXELS).codes.len(), 1);
    }

    #[test]
    fn qr_codes_that_are_not_a_totp_setup_are_seen_but_not_kept() {
        for text in [
            "https://github.com/settings/security",
            "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP",
            "otpauth://hotp/GitHub:alice?secret=JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP&counter=1",
            "otpauth-migration://offline?data=CjEKCkhlbGxvId6tvu8SGEV4YW1wbGU",
        ] {
            let found = decode_frames(&[qr_frame(text, 4)], MAX_PIXELS);
            assert!(found.saw_qr, "{text}");
            assert!(found.codes.is_empty(), "{text}");
        }
    }

    #[test]
    fn a_blank_image_has_no_qr_code() {
        let found = decode_frames(&[blank(200)], MAX_PIXELS);
        assert!(!found.saw_qr);
        assert!(found.codes.is_empty());
    }

    #[test]
    fn frames_past_the_pixel_budget_or_malformed_are_skipped() {
        let qr = qr_frame(GITHUB, 4);
        let pixels = u64::from(qr.width) * u64::from(qr.height);
        assert!(decode_frames(&[qr_frame(GITHUB, 4)], pixels - 1)
            .codes
            .is_empty());
        assert_eq!(decode_frames(&[qr_frame(GITHUB, 4)], pixels).codes.len(), 1);

        let mut short = qr_frame(GITHUB, 4);
        short.rgba.truncate(short.rgba.len() - 1);
        assert!(decode_frames(&[short], MAX_PIXELS).codes.is_empty());
        let empty = Frame {
            width: 0,
            height: 0,
            rgba: vec![],
        };
        assert!(decode_frames(&[empty], MAX_PIXELS).codes.is_empty());
    }

    #[test]
    fn transparent_pixels_read_as_white() {
        let mut qr = qr_frame(GITHUB, 4);
        // Make the white background fully transparent black, as a pasted PNG
        // with alpha can arrive.
        for px in qr.rgba.chunks_exact_mut(4) {
            if px[0] == 255 {
                px.copy_from_slice(&[0, 0, 0, 0]);
            }
        }
        assert_eq!(decode_frames(&[qr], MAX_PIXELS).codes.len(), 1);
    }

    #[test]
    fn a_totp_code_on_the_clipboard_means_the_screen_is_never_captured() {
        let captured = Cell::new(false);
        let codes = scan(
            || Some(qr_frame(GITHUB, 4)),
            || {
                captured.set(true);
                Ok(vec![qr_frame(GITLAB, 4)])
            },
        )
        .unwrap();
        assert!(!captured.get());
        assert_eq!(labels(&codes), [(Some("GitHub"), Some("alice"))]);
    }

    #[test]
    fn without_a_totp_code_on_the_clipboard_the_screen_is_used() {
        let from_screen = || Ok(vec![blank(50), qr_frame(GITLAB, 4)]);
        let bob = [(Some("GitLab"), Some("bob"))];
        assert_eq!(labels(&scan(|| None, from_screen).unwrap()), bob);
        assert_eq!(
            labels(&scan(|| Some(blank(100)), from_screen).unwrap()),
            bob
        );
        let url = || Some(qr_frame("https://example.com", 4));
        assert_eq!(labels(&scan(url, from_screen).unwrap()), bob);
    }

    #[test]
    fn scan_errors_say_what_was_seen() {
        let nothing = || Ok(vec![blank(100)]);
        let url = || Some(qr_frame("https://example.com", 4));
        assert_eq!(scan(|| None, nothing).err(), Some(ScanError::NotFound));
        assert_eq!(scan(url, nothing).err(), Some(ScanError::NotTotp));
        let url_on_screen = || Ok(vec![qr_frame("https://example.com", 4)]);
        assert_eq!(scan(|| None, url_on_screen).err(), Some(ScanError::NotTotp));
        assert_eq!(
            scan(|| None, || Err(ScanError::Capture)).err(),
            Some(ScanError::Capture)
        );
    }

    #[test]
    fn scan_errors_never_carry_decoded_text() {
        for e in [ScanError::NotFound, ScanError::NotTotp, ScanError::Capture] {
            let err = CmdError::from(e);
            assert!(!err.message.contains("otpauth"));
            assert!(!err.message.contains("JBSWY3DP"));
        }
    }
}
