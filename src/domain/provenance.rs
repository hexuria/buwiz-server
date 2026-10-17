//! Screening an uploaded Form 2303 before anyone stores it.
//!
//! A Form 2303 upload exists only to prove a TIN belongs to the claimant.
//! It is never used to choose forms. The screen takes plain scans and photos
//! (JPEG, PNG, PDF up to [`MAX_UPLOAD_BYTES`]) and refuses a file that says
//! an AI made it.
//!
//! **What it reads.** Generators that label their output write the IPTC
//! "digital source type" `trainedAlgorithmicMedia` (or the composite form),
//! either in a C2PA manifest (`c2pa.actions`) or in XMP metadata. Both
//! store it as plain text, so the screen looks for those IRIs in the raw
//! bytes. It also notes whether a C2PA manifest is present at all
//! (JPEG/PDF JUMBF `c2pa` label, PNG `caBX` chunk) for the reviewer.
//!
//! **Why not the c2pa crate.** `c2pa` 0.91 does not build for
//! `wasm32-wasip2` on stable Rust (its `typed-path` dependency enables a
//! nightly feature there), and a native helper would be a new service. The
//! screen only ever *refuses*, so it does not need to verify signatures: a
//! forged mark only hurts the person who uploaded it.
//!
//! **Blind spots (accepted).** No mark found proves nothing: metadata is
//! easy to strip, a screenshot drops it, and pixel watermarks such as
//! SynthID have no public detector. Brotli-compressed C2PA assertions and
//! Flate-compressed PDF metadata are not inflated. Staff review every claim
//! by eye; this screen only stops the files that announce themselves.

use sha2::{Digest, Sha256};

/// Largest accepted upload: a phone photo or a 300 dpi scan fits.
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;

/// IPTC digital source types that mean "made by a trained model".
/// `algorithmicallyEnhanced` (phone processing) is deliberately absent.
const AI_SOURCE_TYPES: [&[u8]; 2] = [
    b"compositeWithTrainedAlgorithmicMedia",
    b"trainedAlgorithmicMedia",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadKind {
    Jpeg,
    Png,
    Pdf,
}

impl UploadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Pdf => "pdf",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Pdf => "application/pdf",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "jpeg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    /// From the file's first bytes, never from its name or a header.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(b"%PDF-") {
            Some(Self::Pdf)
        } else {
            None
        }
    }
}

/// What the screen found in an accepted file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenedUpload {
    pub kind: UploadKind,
    pub size: usize,
    /// SHA-256, hex. Kept after the file itself is deleted.
    pub sha256: String,
    /// A C2PA manifest is present but says nothing about AI.
    pub has_c2pa: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UploadRefusal {
    Empty,
    TooLarge,
    /// Not a JPEG, PNG or PDF.
    UnsupportedType,
    /// The file says an AI made it (the matched IPTC source type).
    AiMade { mark: &'static str },
}

impl std::fmt::Display for UploadRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "the file is empty"),
            Self::TooLarge => write!(f, "the file is larger than 5 MB"),
            Self::UnsupportedType => write!(f, "upload a JPEG, PNG or PDF of your Form 2303"),
            Self::AiMade { .. } => write!(
                f,
                "this file is marked as made by AI; upload a clean scan or photo of your Form 2303"
            ),
        }
    }
}

/// Screens one upload. Never panics, whatever the bytes.
pub fn screen_upload(bytes: &[u8]) -> Result<ScreenedUpload, UploadRefusal> {
    if bytes.is_empty() {
        return Err(UploadRefusal::Empty);
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(UploadRefusal::TooLarge);
    }
    let kind = UploadKind::sniff(bytes).ok_or(UploadRefusal::UnsupportedType)?;
    if let Some(mark) = ai_source_type(bytes) {
        return Err(UploadRefusal::AiMade { mark });
    }
    Ok(ScreenedUpload {
        kind,
        size: bytes.len(),
        sha256: hex(&Sha256::digest(bytes)),
        has_c2pa: has_c2pa_manifest(bytes),
    })
}

/// The first AI source type named anywhere in the file.
pub fn ai_source_type(bytes: &[u8]) -> Option<&'static str> {
    AI_SOURCE_TYPES
        .iter()
        .find(|needle| contains(bytes, needle))
        .map(|needle| std::str::from_utf8(needle).expect("ASCII constant"))
}

/// A C2PA manifest store: the JUMBF `c2pa` label (JPEG APP11, PDF) or a PNG
/// `caBX` chunk.
pub fn has_c2pa_manifest(bytes: &[u8]) -> bool {
    contains(bytes, b"jumb") && contains(bytes, b"c2pa") || contains(bytes, b"caBX")
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|window| window == needle)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F'];

    fn jpeg_with(extra: &[u8]) -> Vec<u8> {
        let mut file = JPEG.to_vec();
        file.extend_from_slice(extra);
        file.extend_from_slice(&[0xFF, 0xD9]);
        file
    }

    #[test]
    fn plain_scans_pass_with_their_hash() {
        let file = jpeg_with(b"scanner output");
        let screened = screen_upload(&file).unwrap();
        assert_eq!(screened.kind, UploadKind::Jpeg);
        assert_eq!(screened.size, file.len());
        assert_eq!(screened.sha256, hex(&Sha256::digest(&file)));
        assert_eq!(screened.sha256.len(), 64);
        assert!(!screened.has_c2pa);
        assert_eq!(
            screen_upload(b"\x89PNG\r\n\x1a\nIHDR").unwrap().kind,
            UploadKind::Png
        );
        assert_eq!(screen_upload(b"%PDF-1.7\n%").unwrap().kind, UploadKind::Pdf);
    }

    #[test]
    fn the_type_comes_from_the_bytes() {
        for bad in [
            &b"GIF89a"[..],
            b"<svg/>",
            b"PK\x03\x04",
            b"\xFF\xD8",
            b"\x89PNG\r\n\x1a",
            b"%PDF",
            b" %PDF-1.7",
        ] {
            assert_eq!(screen_upload(bad), Err(UploadRefusal::UnsupportedType), "{bad:?}");
        }
    }

    #[test]
    fn size_limits_are_exact() {
        assert_eq!(screen_upload(&[]), Err(UploadRefusal::Empty));
        let mut at_limit = JPEG.to_vec();
        at_limit.resize(MAX_UPLOAD_BYTES, 0);
        assert!(screen_upload(&at_limit).is_ok());
        at_limit.push(0);
        assert_eq!(screen_upload(&at_limit), Err(UploadRefusal::TooLarge));
    }

    #[test]
    fn an_ai_source_type_in_xmp_or_c2pa_is_refused() {
        let xmp = jpeg_with(
            b"<Iptc4xmpExt:DigitalSourceType>http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia</Iptc4xmpExt:DigitalSourceType>",
        );
        assert_eq!(
            screen_upload(&xmp),
            Err(UploadRefusal::AiMade { mark: "trainedAlgorithmicMedia" })
        );
        let c2pa = jpeg_with(
            b"\xFF\xEB..JP..jumb..jumdc2pa..c2pa.actions..digitalSourceType..http://cv.iptc.org/newscodes/digitalsourcetype/compositeWithTrainedAlgorithmicMedia",
        );
        assert_eq!(
            screen_upload(&c2pa),
            Err(UploadRefusal::AiMade { mark: "compositeWithTrainedAlgorithmicMedia" })
        );
        let png = b"\x89PNG\r\n\x1a\n....caBX....trainedAlgorithmicMedia".to_vec();
        assert!(matches!(screen_upload(&png), Err(UploadRefusal::AiMade { .. })));
        let pdf = b"%PDF-1.7\n<x:xmpmeta>trainedAlgorithmicMedia</x:xmpmeta>".to_vec();
        assert!(matches!(screen_upload(&pdf), Err(UploadRefusal::AiMade { .. })));
    }

    #[test]
    fn a_phone_photo_with_a_c2pa_manifest_passes_and_is_flagged() {
        let camera = jpeg_with(
            b"\xFF\xEB..JP..jumb..jumdc2pa..c2pa.actions..digitalSourceType..http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture",
        );
        let screened = screen_upload(&camera).unwrap();
        assert!(screened.has_c2pa);
        let enhanced = jpeg_with(b"digitalsourcetype/algorithmicallyEnhanced");
        assert!(screen_upload(&enhanced).is_ok());
        assert!(screen_upload(&b"\x89PNG\r\n\x1a\n..caBX..".to_vec()).unwrap().has_c2pa);
        // "c2pa" text alone, without a JUMBF box, is not a manifest.
        assert!(!screen_upload(&jpeg_with(b"c2pa")).unwrap().has_c2pa);
        assert!(!screen_upload(&jpeg_with(b"jumb")).unwrap().has_c2pa);
    }

    #[test]
    fn kinds_round_trip() {
        for kind in [UploadKind::Jpeg, UploadKind::Png, UploadKind::Pdf] {
            assert_eq!(UploadKind::parse(kind.as_str()), Some(kind));
            assert!(kind.mime().contains('/'));
        }
        assert_eq!(UploadKind::parse("gif"), None);
        assert_eq!(UploadKind::Pdf.mime(), "application/pdf");
    }

    #[test]
    fn refusals_explain_themselves() {
        assert!(UploadRefusal::AiMade { mark: "trainedAlgorithmicMedia" }
            .to_string()
            .contains("clean scan"));
        assert!(UploadRefusal::TooLarge.to_string().contains("5 MB"));
        assert!(UploadRefusal::UnsupportedType.to_string().contains("JPEG"));
        assert!(UploadRefusal::Empty.to_string().contains("empty"));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2048))]

        #[test]
        fn screening_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = screen_upload(&bytes);
        }

        /// Wherever the mark sits, in any accepted file type, it is found.
        #[test]
        fn a_mark_anywhere_is_found(
            prefix in proptest::collection::vec(any::<u8>(), 0..256),
            suffix in proptest::collection::vec(any::<u8>(), 0..256),
            which in 0usize..2,
            header in 0usize..3,
        ) {
            let headers: [&[u8]; 3] = [JPEG, b"\x89PNG\r\n\x1a\n", b"%PDF-1.4"];
            let mut file = headers[header].to_vec();
            file.extend_from_slice(&prefix);
            file.extend_from_slice(AI_SOURCE_TYPES[which]);
            file.extend_from_slice(&suffix);
            let refused = matches!(screen_upload(&file), Err(UploadRefusal::AiMade { .. }));
            prop_assert!(refused);
        }

        /// Accepted files keep their exact hash and size.
        #[test]
        fn accepted_files_hash_like_sha2(body in proptest::collection::vec(0u8..=0x7f, 0..256)) {
            let file = jpeg_with(&body);
            prop_assume!(ai_source_type(&file).is_none());
            let screened = screen_upload(&file).unwrap();
            prop_assert_eq!(screened.size, file.len());
            prop_assert_eq!(screened.sha256, hex(&Sha256::digest(&file)));
        }
    }
}
