//! Hashed taxpayer identity used for exclusive ownership.
//!
//! The server never uses raw TIN as a primary key. Clients attest a TIN at
//! create/claim time; we store `tin_last4` for display and an HMAC of
//! `{tin_root}|{branch_code}` for uniqueness and claim matching.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::tin::{BranchCode, TinRoot};

type HmacSha256 = Hmac<Sha256>;

/// HMAC-SHA256 hex digest of the taxpayer registration unit.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TinIdentityHash(pub String);

impl TinIdentityHash {
    pub fn compute(pepper: &[u8], tin_root: &TinRoot, branch_code: &BranchCode) -> Self {
        let mut mac =
            HmacSha256::new_from_slice(pepper).expect("HMAC-SHA256 accepts any pepper length");
        mac.update(tin_root.as_str().as_bytes());
        mac.update(b"|");
        mac.update(branch_code.as_str().as_bytes());
        Self(to_hex(&mac.finalize().into_bytes()))
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim().to_ascii_lowercase();
        if raw.len() != 64 || !raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err("tin identity hash is invalid".to_owned());
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// True when `incoming_root` hashes to the same TIN as an existing personal branch.
///
/// Personal accounts hold one TIN and many branches. We never store raw TIN, so
/// the probe re-hashes the attested root with an existing branch code.
pub fn same_personal_tin(
    pepper: &[u8],
    existing_branch: &BranchCode,
    existing_hash: &TinIdentityHash,
    incoming_root: &TinRoot,
) -> bool {
    &TinIdentityHash::compute(pepper, incoming_root, existing_branch) == existing_hash
}

/// Last four digits of the 9-digit TIN root, for display only.
pub fn tin_last4(tin_root: &TinRoot) -> String {
    let digits = tin_root.as_str();
    digits[digits.len().saturating_sub(4)..].to_string()
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last4_is_suffix_of_root() {
        let root = TinRoot::parse("123456789").unwrap();
        assert_eq!(tin_last4(&root), "6789");
    }

    #[test]
    fn hash_is_stable_and_not_raw_tin() {
        let root = TinRoot::parse("123456789").unwrap();
        let branch = BranchCode::parse("00000").unwrap();
        let hash = TinIdentityHash::compute(b"pepper", &root, &branch);
        assert_eq!(hash.0.len(), 64);
        assert!(!hash.0.contains("123456789"));
        assert_eq!(hash, TinIdentityHash::compute(b"pepper", &root, &branch));
        assert_ne!(hash, TinIdentityHash::compute(b"other", &root, &branch));
    }

    #[test]
    fn personal_tin_probe_matches_existing_branch_hash() {
        let root = TinRoot::parse("123456789").unwrap();
        let other = TinRoot::parse("987654321").unwrap();
        let head = BranchCode::parse("00000").unwrap();
        let annex = BranchCode::parse("00001").unwrap();
        let hash = TinIdentityHash::compute(b"pepper", &root, &head);
        assert!(same_personal_tin(b"pepper", &head, &hash, &root));
        assert!(!same_personal_tin(b"pepper", &head, &hash, &other));
        assert_ne!(
            TinIdentityHash::compute(b"pepper", &root, &head),
            TinIdentityHash::compute(b"pepper", &root, &annex)
        );
    }
}
