use crate::error::CryptoError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 20-byte address derived from Blake3(public_key)[0..20].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Address(#[serde(with = "hex::serde")] [u8; 20]);

impl Address {
    pub const ZERO: Self = Self([0u8; 20]);

    pub fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }

    pub fn from_hex(s: &str) -> Result<Self, CryptoError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s).map_err(|e| CryptoError::InvalidHex(e.to_string()))?;
        if bytes.len() != 20 {
            return Err(CryptoError::InvalidKeyLength {
                expected: 20,
                actual: bytes.len(),
            });
        }
        let mut arr = [0u8; 20];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 20]
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({})", self.to_hex())
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Default for Address {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_hex_roundtrip() {
        let addr = Address::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
        let hex_str = addr.to_hex();
        let addr2 = Address::from_hex(&hex_str).unwrap();
        assert_eq!(addr, addr2);
    }

    #[test]
    fn test_zero_address() {
        assert!(Address::ZERO.is_zero());
        assert!(!Address::from_bytes([1; 20]).is_zero());
    }

    #[test]
    fn test_address_display() {
        let addr = Address::ZERO;
        assert!(addr.to_hex().starts_with("0x"));
        assert_eq!(addr.to_hex().len(), 42); // "0x" + 40 hex chars
    }
}
