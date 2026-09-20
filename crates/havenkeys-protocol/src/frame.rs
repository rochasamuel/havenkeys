//! Length-prefixed framing.
//!
//! A frame is a 4-byte unsigned length in native byte order followed by that
//! many bytes. This is the native messaging wire format; the local socket
//! reuses it so both hops share one implementation. Every platform HavenKeys
//! targets is little-endian.

use std::io::{self, Read, Write};
use zeroize::Zeroizing;

#[derive(Debug)]
pub enum FrameError {
    /// The stream failed or ended in the middle of a frame.
    Io,
    /// The declared length exceeds the limit. The payload has not been read.
    TooLarge(u32),
}

impl From<io::Error> for FrameError {
    fn from(_: io::Error) -> Self {
        FrameError::Io
    }
}

/// Read one frame. `Ok(None)` means the stream ended cleanly between frames.
///
/// The length is checked against `max` *before* anything is allocated, so a
/// peer cannot make us reserve memory by announcing a huge frame.
pub fn read_frame<R: Read>(
    r: &mut R,
    max: usize,
) -> Result<Option<Zeroizing<Vec<u8>>>, FrameError> {
    let mut len_buf = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut len_buf[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(FrameError::Io),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(FrameError::Io),
        }
    }
    let len = u32::from_ne_bytes(len_buf);
    if len as usize > max {
        return Err(FrameError::TooLarge(len));
    }
    let mut buf = Zeroizing::new(vec![0u8; len as usize]);
    r.read_exact(&mut buf)?;
    Ok(Some(buf))
}

/// Read and throw away `len` bytes (the payload of a rejected frame), so the
/// stream stays in sync. Uses a fixed buffer regardless of `len`.
pub fn discard<R: Read>(r: &mut R, len: u32) -> Result<(), FrameError> {
    let copied = io::copy(&mut r.take(u64::from(len)), &mut io::sink())?;
    if copied == u64::from(len) {
        Ok(())
    } else {
        Err(FrameError::Io)
    }
}

/// Write one frame and flush. Refuses payloads over `max`.
pub fn write_frame<W: Write>(w: &mut W, payload: &[u8], max: usize) -> Result<(), FrameError> {
    if payload.len() > max {
        return Err(FrameError::TooLarge(
            u32::try_from(payload.len()).unwrap_or(u32::MAX),
        ));
    }
    let len = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge(u32::MAX))?;
    // One buffer, one write: a frame is never split across two writes that
    // another thread could interleave with.
    let mut out = Zeroizing::new(Vec::with_capacity(4 + payload.len()));
    out.extend_from_slice(&len.to_ne_bytes());
    out.extend_from_slice(payload);
    w.write_all(&out)?;
    w.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn framed(payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        write_frame(&mut v, payload, usize::MAX).unwrap();
        v
    }

    #[test]
    fn round_trip() {
        let mut c = Cursor::new(framed(b"{\"a\":1}"));
        assert_eq!(
            read_frame(&mut c, 64).unwrap().unwrap().as_slice(),
            b"{\"a\":1}"
        );
        assert!(read_frame(&mut c, 64).unwrap().is_none());
    }

    #[test]
    fn empty_stream_is_clean_eof() {
        assert!(read_frame(&mut Cursor::new(Vec::new()), 64)
            .unwrap()
            .is_none());
    }

    #[test]
    fn truncated_length_or_payload_is_an_error() {
        assert!(matches!(
            read_frame(&mut Cursor::new(vec![5, 0]), 64),
            Err(FrameError::Io)
        ));
        let mut short = framed(b"hello");
        short.truncate(6);
        assert!(matches!(
            read_frame(&mut Cursor::new(short), 64),
            Err(FrameError::Io)
        ));
    }

    #[test]
    fn oversized_frame_rejected_before_allocation() {
        // Announces 4 GiB but carries nothing: must fail on the length alone.
        let mut c = Cursor::new(u32::MAX.to_ne_bytes().to_vec());
        assert!(matches!(
            read_frame(&mut c, 1024),
            Err(FrameError::TooLarge(u32::MAX))
        ));
    }

    #[test]
    fn discard_resynchronizes() {
        let mut bytes = framed(&[b'x'; 100]);
        bytes.extend(framed(b"next"));
        let mut c = Cursor::new(bytes);
        let Err(FrameError::TooLarge(n)) = read_frame(&mut c, 10) else {
            panic!("expected TooLarge");
        };
        discard(&mut c, n).unwrap();
        assert_eq!(read_frame(&mut c, 10).unwrap().unwrap().as_slice(), b"next");
    }

    #[test]
    fn write_refuses_oversized_payload() {
        let mut out = Vec::new();
        assert!(matches!(
            write_frame(&mut out, &[0; 11], 10),
            Err(FrameError::TooLarge(11))
        ));
        assert!(out.is_empty());
    }
}
