use crate::error::CryptoError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 32-byte Blake3 hash value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Blake3Hash(#[serde(with = "hex::serde")] [u8; 32]);

impl Blake3Hash {
    pub const ZERO: Self = Self([0u8; 32]);

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, CryptoError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s).map_err(|e| CryptoError::InvalidHex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength {
                expected: 32,
                actual: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for Blake3Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blake3Hash({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for Blake3Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", self.to_hex())
    }
}

impl AsRef<[u8]> for Blake3Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 32]> for Blake3Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl Default for Blake3Hash {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Hash arbitrary data using Blake3.
pub fn hash(data: &[u8]) -> Blake3Hash {
    let h = blake3::hash(data);
    Blake3Hash(*h.as_bytes())
}

/// Hash multiple items with domain separation.
pub fn hash_multiple(items: &[&[u8]]) -> Blake3Hash {
    let mut hasher = blake3::Hasher::new();
    for item in items {
        hasher.update(&(item.len() as u64).to_le_bytes());
        hasher.update(item);
    }
    Blake3Hash(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_deterministic() {
        let h1 = hash(b"hello world");
        let h2 = hash(b"hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_different_inputs() {
        let h1 = hash(b"hello");
        let h2 = hash(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_hex_roundtrip() {
        let h = hash(b"test data");
        let hex_str = h.to_hex();
        let h2 = Blake3Hash::from_hex(&hex_str).unwrap();
        assert_eq!(h, h2);
    }

    #[test]
    fn test_hash_multiple_domain_separation() {
        let h1 = hash_multiple(&[b"ab", b"cd"]);
        let h2 = hash_multiple(&[b"abc", b"d"]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_zero_hash() {
        assert_eq!(Blake3Hash::ZERO, Blake3Hash::from([0u8; 32]));
    }
}
