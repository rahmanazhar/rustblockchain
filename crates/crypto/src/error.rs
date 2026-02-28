use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid signature")]
    InvalidSignature,

    #[error("invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("invalid hex: {0}")]
    InvalidHex(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("mnemonic error: {0}")]
    Mnemonic(String),

    #[error("keystore error: {0}")]
    Keystore(String),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("decryption error: {0}")]
    Decryption(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}
