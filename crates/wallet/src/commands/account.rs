use anyhow::Result;
use rustchain_crypto::{KeyPair, Keystore, Mnemonic};
use std::path::Path;

/// Create a new account with a BIP39 mnemonic.
pub fn create_account(word_count: usize, keystore_dir: &Path, password: &str) -> Result<()> {
    std::fs::create_dir_all(keystore_dir)?;

    let mnemonic = Mnemonic::generate(word_count)?;
    let keypair = mnemonic.to_keypair(password)?;
    let address = keypair.address();

    let keyfile = keystore_dir.join(format!("{}.json", address.to_hex()));
    Keystore::save_to_file(&keypair, password, &keyfile)?;

    println!("Account created successfully!");
    println!();
    println!("  Address:  {}", address);
    println!("  Keyfile:  {}", keyfile.display());
    println!();
    println!("  Mnemonic (SAVE THIS - displayed only once):");
    println!("  {}", mnemonic.phrase());
    println!();

    Ok(())
}

/// Import an account from a mnemonic phrase.
pub fn import_from_mnemonic(
    phrase: &str,
    keystore_dir: &Path,
    password: &str,
) -> Result<()> {
    std::fs::create_dir_all(keystore_dir)?;

    let mnemonic = Mnemonic::from_phrase(phrase)?;
    let keypair = mnemonic.to_keypair(password)?;
    let address = keypair.address();

    let keyfile = keystore_dir.join(format!("{}.json", address.to_hex()));
    Keystore::save_to_file(&keypair, password, &keyfile)?;

    println!("Account imported successfully!");
    println!("  Address: {}", address);
    println!("  Keyfile: {}", keyfile.display());

    Ok(())
}

/// Import an account from a raw private key hex string.
pub fn import_from_private_key(
    private_key_hex: &str,
    keystore_dir: &Path,
    password: &str,
) -> Result<()> {
    std::fs::create_dir_all(keystore_dir)?;

    let hex_str = private_key_hex.strip_prefix("0x").unwrap_or(private_key_hex);
    let bytes = hex::decode(hex_str)?;
    if bytes.len() != 32 {
        anyhow::bail!("private key must be 32 bytes");
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&bytes);

    let keypair = KeyPair::from_bytes(&key_bytes)?;
    let address = keypair.address();

    let keyfile = keystore_dir.join(format!("{}.json", address.to_hex()));
    Keystore::save_to_file(&keypair, password, &keyfile)?;

    println!("Account imported successfully!");
    println!("  Address: {}", address);
    println!("  Keyfile: {}", keyfile.display());

    Ok(())
}

/// List all accounts in the keystore directory.
pub fn list_accounts(keystore_dir: &Path) -> Result<()> {
    if !keystore_dir.exists() {
        println!("No accounts found. Create one with: rustchain-wallet account create");
        return Ok(());
    }

    let mut count = 0;
    for entry in std::fs::read_dir(keystore_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                println!("  {}", name);
                count += 1;
            }
        }
    }

    if count == 0 {
        println!("No accounts found.");
    } else {
        println!("\n{} account(s) found", count);
    }

    Ok(())
}
