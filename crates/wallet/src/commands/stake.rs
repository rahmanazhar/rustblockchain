use crate::client::NodeClient;
use crate::commands::transfer::fetch_chain_id;
use anyhow::Result;
use rustchain_core::{SignedTransaction, Transaction, TxType};
use rustchain_crypto::{Address, Keystore};
use std::path::Path;

/// Stake tokens to become or increase stake as a validator.
pub async fn send_stake(
    node_url: &str,
    keystore_dir: &Path,
    from_hex: &str,
    amount: u128,
    password: &str,
) -> Result<()> {
    let client = NodeClient::new(node_url);
    let chain_id = fetch_chain_id(&client).await?;

    let from_addr = Address::from_hex(from_hex)?;
    let keyfile = keystore_dir.join(format!("{}.json", from_addr.to_hex()));
    if !keyfile.exists() {
        anyhow::bail!("Keyfile not found for address {}", from_addr);
    }
    let keypair = Keystore::load_from_file(&keyfile, password)?;

    let account_info: serde_json::Value = client.get_account(from_hex).await?;
    let nonce = account_info
        .get("data")
        .and_then(|d| d.get("nonce"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let tx = Transaction {
        chain_id,
        nonce,
        from: from_addr,
        to: None,
        value: amount,
        tx_type: TxType::Stake,
        gas_limit: 21000,
        gas_price: 1,
        data: vec![],
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let signed = SignedTransaction::new(tx, &keypair);

    let body = serde_json::json!({
        "chain_id": chain_id,
        "from": from_hex,
        "value": amount.to_string(),
        "gas_limit": 21000,
        "gas_price": 1,
        "nonce": nonce,
        "tx_type": "Stake",
        "data": "",
        "timestamp": signed.transaction.timestamp,
        "public_key": keypair.public_key().to_hex(),
        "signature": signed.signature.to_hex(),
    });

    let result: serde_json::Value = client.submit_transaction(&body).await?;

    println!("Stake transaction submitted!");
    println!("  Hash: {}", signed.hash());
    println!("  Validator: {}", from_addr);
    println!("  Stake amount: {}", amount);

    if let Some(data) = result.get("data") {
        if let Some(tx_hash) = data.get("tx_hash") {
            println!("  Confirmed hash: {}", tx_hash);
        }
    }

    Ok(())
}

/// Unstake tokens from the validator set.
pub async fn send_unstake(
    node_url: &str,
    keystore_dir: &Path,
    from_hex: &str,
    amount: u128,
    password: &str,
) -> Result<()> {
    let client = NodeClient::new(node_url);
    let chain_id = fetch_chain_id(&client).await?;

    let from_addr = Address::from_hex(from_hex)?;
    let keyfile = keystore_dir.join(format!("{}.json", from_addr.to_hex()));
    if !keyfile.exists() {
        anyhow::bail!("Keyfile not found for address {}", from_addr);
    }
    let keypair = Keystore::load_from_file(&keyfile, password)?;

    let account_info: serde_json::Value = client.get_account(from_hex).await?;
    let nonce = account_info
        .get("data")
        .and_then(|d| d.get("nonce"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let tx = Transaction {
        chain_id,
        nonce,
        from: from_addr,
        to: None,
        value: amount,
        tx_type: TxType::Unstake,
        gas_limit: 21000,
        gas_price: 1,
        data: vec![],
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let signed = SignedTransaction::new(tx, &keypair);

    let body = serde_json::json!({
        "chain_id": chain_id,
        "from": from_hex,
        "value": amount.to_string(),
        "gas_limit": 21000,
        "gas_price": 1,
        "nonce": nonce,
        "tx_type": "Unstake",
        "data": "",
        "timestamp": signed.transaction.timestamp,
        "public_key": keypair.public_key().to_hex(),
        "signature": signed.signature.to_hex(),
    });

    let result: serde_json::Value = client.submit_transaction(&body).await?;

    println!("Unstake transaction submitted!");
    println!("  Hash: {}", signed.hash());
    println!("  Validator: {}", from_addr);
    println!("  Unstake amount: {}", amount);

    if let Some(data) = result.get("data") {
        if let Some(tx_hash) = data.get("tx_hash") {
            println!("  Confirmed hash: {}", tx_hash);
        }
    }

    Ok(())
}
