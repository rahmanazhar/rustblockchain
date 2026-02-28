use crate::error::CryptoError;
use crate::keypair::KeyPair;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use zeroize::Zeroize;

/// Encrypted keystore format.
#[derive(Serialize, Deserialize)]
struct EncryptedKeystore {
    version: u32,
    crypto: CryptoParams,
}

#[derive(Serialize, Deserialize)]
struct CryptoParams {
    cipher: String,
    ciphertext: Vec<u8>,
    nonce: Vec<u8>,
    kdf: KdfParams,
}

#[derive(Serialize, Deserialize)]
struct KdfParams {
    algorithm: String,
    log_n: u8,
    r: u32,
    p: u32,
    salt: Vec<u8>,
}

/// Encrypted key storage using scrypt + AES-256-GCM.
pub struct Keystore;

impl Keystore {
    /// Encrypt a keypair with a password.
    pub fn encrypt(keypair: &KeyPair, password: &str) -> Result<Vec<u8>, CryptoError> {
        let mut salt = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut salt);

        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

        // Derive encryption key with scrypt
        let params = scrypt::Params::new(14, 8, 1, 32)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;
        let mut derived_key = [0u8; 32];
        scrypt::scrypt(password.as_bytes(), &salt, &params, &mut derived_key)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;

        // Encrypt with AES-256-GCM
        let cipher = Aes256Gcm::new_from_slice(&derived_key)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, keypair.secret_bytes().as_ref())
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;

        derived_key.zeroize();

        let keystore = EncryptedKeystore {
            version: 1,
            crypto: CryptoParams {
                cipher: "aes-256-gcm".to_string(),
                ciphertext,
                nonce: nonce_bytes.to_vec(),
                kdf: KdfParams {
                    algorithm: "scrypt".to_string(),
                    log_n: 14,
                    r: 8,
                    p: 1,
                    salt: salt.to_vec(),
                },
            },
        };

        serde_json::to_vec_pretty(&keystore)
            .map_err(|e| CryptoError::Serialization(e.to_string()))
    }

    /// Decrypt a keypair from encrypted data.
    pub fn decrypt(encrypted: &[u8], password: &str) -> Result<KeyPair, CryptoError> {
        let keystore: EncryptedKeystore = serde_json::from_slice(encrypted)
            .map_err(|e| CryptoError::Decryption(e.to_string()))?;

        if keystore.version != 1 {
            return Err(CryptoError::Decryption(format!(
                "unsupported keystore version: {}",
                keystore.version
            )));
        }

        let params = scrypt::Params::new(
            keystore.crypto.kdf.log_n,
            keystore.crypto.kdf.r,
            keystore.crypto.kdf.p,
            32,
        )
        .map_err(|e| CryptoError::Decryption(e.to_string()))?;

        let mut derived_key = [0u8; 32];
        scrypt::scrypt(
            password.as_bytes(),
            &keystore.crypto.kdf.salt,
            &params,
            &mut derived_key,
        )
        .map_err(|e| CryptoError::Decryption(e.to_string()))?;

        let cipher = Aes256Gcm::new_from_slice(&derived_key)
            .map_err(|e| CryptoError::Decryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&keystore.crypto.nonce);
        let mut secret_bytes = cipher
            .decrypt(nonce, keystore.crypto.ciphertext.as_ref())
            .map_err(|_| CryptoError::Decryption("wrong password or corrupted keystore".to_string()))?;

        derived_key.zeroize();

        if secret_bytes.len() != 32 {
            secret_bytes.zeroize();
            return Err(CryptoError::Decryption(
                "invalid decrypted key length".to_string(),
            ));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&secret_bytes);
        secret_bytes.zeroize();

        let keypair = KeyPair::from_bytes(&key_bytes)?;
        key_bytes.zeroize();
        Ok(keypair)
    }

    /// Save an encrypted keystore to a file.
    pub fn save_to_file(
        keypair: &KeyPair,
        password: &str,
        path: &Path,
    ) -> Result<(), CryptoError> {
        let encrypted = Self::encrypt(keypair, password)?;
        std::fs::write(path, &encrypted)
            .map_err(|e| CryptoError::Keystore(e.to_string()))
    }

    /// Load and decrypt a keystore from a file.
    pub fn load_from_file(path: &Path, password: &str) -> Result<KeyPair, CryptoError> {
        let data = std::fs::read(path)
            .map_err(|e| CryptoError::Keystore(e.to_string()))?;
        Self::decrypt(&data, password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let kp = KeyPair::generate();
        let encrypted = Keystore::encrypt(&kp, "test-password").unwrap();
        let kp2 = Keystore::decrypt(&encrypted, "test-password").unwrap();
        assert_eq!(kp.public_key(), kp2.public_key());
    }

    #[test]
    fn test_wrong_password() {
        let kp = KeyPair::generate();
        let encrypted = Keystore::encrypt(&kp, "correct-password").unwrap();
        assert!(Keystore::decrypt(&encrypted, "wrong-password").is_err());
    }

    #[test]
    fn test_file_roundtrip() {
        let kp = KeyPair::generate();
        let path = std::env::temp_dir().join("rustchain_test_keystore.json");
        Keystore::save_to_file(&kp, "file-password", &path).unwrap();
        let kp2 = Keystore::load_from_file(&path, "file-password").unwrap();
        assert_eq!(kp.public_key(), kp2.public_key());
        let _ = std::fs::remove_file(&path);
    }
}
