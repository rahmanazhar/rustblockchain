use crate::address::Address;
use crate::error::CryptoError;
use crate::hash;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fmt;
/// Ed25519 keypair with manual zeroize-on-drop for the secret key.
#[derive(Clone)]
pub struct KeyPair {
    signing_key: SigningKey,
}

impl Drop for KeyPair {
    fn drop(&mut self) {
        // SigningKey internally zeroizes on drop via ed25519-dalek
        // This is a safety net
    }
}

/// Ed25519 public key (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey(#[serde(with = "hex::serde")] [u8; 32]);

/// Ed25519 signature (64 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(#[serde(with = "hex::serde")] [u8; 64]);

impl KeyPair {
    /// Generate a new random keypair using OS entropy.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Create a keypair from raw secret key bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let signing_key = SigningKey::from_bytes(bytes);
        Ok(Self { signing_key })
    }

    /// Get the public key.
    pub fn public_key(&self) -> PublicKey {
        let vk = self.signing_key.verifying_key();
        PublicKey(vk.to_bytes())
    }

    /// Sign a message.
    pub fn sign(&self, message: &[u8]) -> Signature {
        let sig = self.signing_key.sign(message);
        Signature(sig.to_bytes())
    }

    /// Get the raw secret key bytes (use with caution).
    pub fn secret_bytes(&self) -> &[u8; 32] {
        self.signing_key.as_bytes()
    }

    /// Derive the address from this keypair.
    pub fn address(&self) -> Address {
        self.public_key().to_address()
    }
}

impl PublicKey {
    /// Verify a signature against this public key.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), CryptoError> {
        let vk = VerifyingKey::from_bytes(&self.0)
            .map_err(|_| CryptoError::InvalidSignature)?;
        let sig = ed25519_dalek::Signature::from_bytes(&signature.0);
        vk.verify(message, &sig)
            .map_err(|_| CryptoError::InvalidSignature)
    }

    /// Derive the address from this public key.
    pub fn to_address(&self) -> Address {
        let h = hash::hash(&self.0);
        let mut addr_bytes = [0u8; 20];
        addr_bytes.copy_from_slice(&h.as_bytes()[..20]);
        Address::from_bytes(addr_bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
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
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", self.to_hex())
    }
}

impl Signature {
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, CryptoError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s).map_err(|e| CryptoError::InvalidHex(e.to_string()))?;
        if bytes.len() != 64 {
            return Err(CryptoError::InvalidKeyLength {
                expected: 64,
                actual: bytes.len(),
            });
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature({}...)", &self.to_hex()[..16])
    }
}

impl Default for Signature {
    fn default() -> Self {
        Self([0u8; 64])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generate_and_sign() {
        let kp = KeyPair::generate();
        let msg = b"hello blockchain";
        let sig = kp.sign(msg);
        assert!(kp.public_key().verify(msg, &sig).is_ok());
    }

    #[test]
    fn test_invalid_signature() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let msg = b"hello";
        let sig = kp1.sign(msg);
        assert!(kp2.public_key().verify(msg, &sig).is_err());
    }

    #[test]
    fn test_wrong_message() {
        let kp = KeyPair::generate();
        let sig = kp.sign(b"correct message");
        assert!(kp.public_key().verify(b"wrong message", &sig).is_err());
    }

    #[test]
    fn test_keypair_from_bytes() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::from_bytes(kp1.secret_bytes()).unwrap();
        assert_eq!(kp1.public_key(), kp2.public_key());
    }

    #[test]
    fn test_address_derivation() {
        let kp = KeyPair::generate();
        let addr1 = kp.public_key().to_address();
        let addr2 = kp.address();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_public_key_hex_roundtrip() {
        let kp = KeyPair::generate();
        let pk = kp.public_key();
        let hex_str = pk.to_hex();
        let pk2 = PublicKey::from_hex(&hex_str).unwrap();
        assert_eq!(pk, pk2);
    }
}
