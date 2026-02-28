use crate::error::CryptoError;
use crate::keypair::KeyPair;
use bip39::Mnemonic as Bip39Mnemonic;
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroize;

/// BIP39 mnemonic wrapper for key generation.
pub struct Mnemonic {
    inner: Bip39Mnemonic,
}

impl Mnemonic {
    /// Generate a new random mnemonic (12 or 24 words).
    pub fn generate(word_count: usize) -> Result<Self, CryptoError> {
        let entropy_bytes = match word_count {
            12 => 16, // 128 bits
            24 => 32, // 256 bits
            _ => {
                return Err(CryptoError::Mnemonic(
                    "word count must be 12 or 24".to_string(),
                ))
            }
        };

        let mut entropy = vec![0u8; entropy_bytes];
        rand::rngs::OsRng.fill_bytes(&mut entropy);

        let inner = Bip39Mnemonic::from_entropy(&entropy)
            .map_err(|e| CryptoError::Mnemonic(e.to_string()))?;

        entropy.zeroize();
        Ok(Self { inner })
    }

    /// Parse a mnemonic from a phrase string.
    pub fn from_phrase(phrase: &str) -> Result<Self, CryptoError> {
        let inner = Bip39Mnemonic::parse(phrase)
            .map_err(|e| CryptoError::Mnemonic(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Get the mnemonic phrase.
    pub fn phrase(&self) -> String {
        self.inner.to_string()
    }

    /// Derive a seed from this mnemonic with an optional password.
    pub fn to_seed(&self, password: &str) -> [u8; 64] {
        self.inner.to_seed(password)
    }

    /// Derive an Ed25519 keypair from this mnemonic.
    pub fn to_keypair(&self, password: &str) -> Result<KeyPair, CryptoError> {
        let seed = self.to_seed(password);

        // Use HKDF to derive a 32-byte Ed25519 secret key from the 64-byte BIP39 seed
        let hk = Hkdf::<Sha256>::new(Some(b"rustchain-ed25519"), &seed);
        let mut secret_key = [0u8; 32];
        hk.expand(b"ed25519 key derivation", &mut secret_key)
            .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

        let keypair = KeyPair::from_bytes(&secret_key)?;
        secret_key.zeroize();
        Ok(keypair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_12_words() {
        let m = Mnemonic::generate(12).unwrap();
        let phrase = m.phrase();
        let words: Vec<&str> = phrase.split_whitespace().collect();
        assert_eq!(words.len(), 12);
    }

    #[test]
    fn test_generate_24_words() {
        let m = Mnemonic::generate(24).unwrap();
        let phrase = m.phrase();
        let words: Vec<&str> = phrase.split_whitespace().collect();
        assert_eq!(words.len(), 24);
    }

    #[test]
    fn test_invalid_word_count() {
        assert!(Mnemonic::generate(15).is_err());
    }

    #[test]
    fn test_from_phrase_roundtrip() {
        let m1 = Mnemonic::generate(12).unwrap();
        let phrase = m1.phrase();
        let m2 = Mnemonic::from_phrase(&phrase).unwrap();
        assert_eq!(m1.phrase(), m2.phrase());
    }

    #[test]
    fn test_keypair_derivation_deterministic() {
        let m = Mnemonic::generate(24).unwrap();
        let kp1 = m.to_keypair("password").unwrap();
        let kp2 = m.to_keypair("password").unwrap();
        assert_eq!(kp1.public_key(), kp2.public_key());
    }

    #[test]
    fn test_different_passwords_different_keys() {
        let m = Mnemonic::generate(24).unwrap();
        let kp1 = m.to_keypair("password1").unwrap();
        let kp2 = m.to_keypair("password2").unwrap();
        assert_ne!(kp1.public_key(), kp2.public_key());
    }
}
