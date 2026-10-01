use zeroize::Zeroizing;

/// The text of the first QR code in a greyscale frame. rxing, as on the
/// desktop (`qr_scan.rs` explains why not rqrr).
pub(crate) fn decode(luma: Vec<u8>, width: u32, height: u32) -> Option<Zeroizing<String>> {
    if width == 0 || height == 0 || luma.len() != (width as usize) * (height as usize) {
        return None;
    }
    let mut hints = rxing::DecodeHints {
        PossibleFormats: Some([rxing::BarcodeFormat::QR_CODE].into_iter().collect()),
        TryHarder: Some(true),
        ..Default::default()
    };
    rxing::helpers::detect_in_luma_with_hints(luma, width, height, None, &mut hints)
        .ok()
        .map(|r| Zeroizing::new(r.getText().to_owned()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn frame_of(text: &str) -> (Vec<u8>, u32, u32) {
        let code = qrcode::QrCode::new(text.as_bytes()).unwrap();
        let modules = code.to_colors();
        let w = code.width() as u32;
        let scale = 8u32;
        let quiet = 4u32;
        let size = (w + 2 * quiet) * scale;
        let mut img = vec![255u8; (size * size) as usize];
        for (i, c) in modules.iter().enumerate() {
            if *c == qrcode::Color::Dark {
                let (mx, my) = (i as u32 % w + quiet, i as u32 / w + quiet);
                for dy in 0..scale {
                    for dx in 0..scale {
                        img[((my * scale + dy) * size + mx * scale + dx) as usize] = 0;
                    }
                }
            }
        }
        (img, size, size)
    }

    #[test]
    fn a_rendered_code_reads_back() {
        let (img, w, h) = frame_of("havenkeys://kit/v2?x=1");
        assert_eq!(
            decode(img, w, h).as_deref().map(|s| s.as_str()),
            Some("havenkeys://kit/v2?x=1")
        );
    }

    #[test]
    fn a_blank_or_malformed_frame_reads_nothing() {
        assert!(decode(vec![255; 100 * 100], 100, 100).is_none());
        assert!(decode(vec![0; 10], 100, 100).is_none());
    }
}
