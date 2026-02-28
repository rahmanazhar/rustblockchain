use crate::client::NodeClient;
use anyhow::Result;
use rustchain_core::{Transaction, TxType, SignedTransaction};
use rustchain_crypto::{Address, Keystore};
use std::path::Path;

/// Send tokens to another address.
pub async fn send_transfer(
    node_url: &str,
    keystore_dir: &Path,
    from_hex: &str,
    to_hex: &str,
    amount: u128,
    password: &str,
) -> Result<()> {
    let client = NodeClient::new(node_url);

    // Load keypair from keystore
    let from_addr = Address::from_hex(from_hex)?;
    let keyfile = keystore_dir.join(format!("{}.json", from_addr.to_hex()));
    if !keyfile.exists() {
        anyhow::bail!("Keyfile not found for address {}", from_addr);
    }
    let keypair = Keystore::load_from_file(&keyfile, password)?;

    // Get nonce from the node
    let account_info: serde_json::Value = client.get_account(from_hex).await?;
    let nonce = account_info
        .get("data")
        .and_then(|d| d.get("nonce"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let to_addr = Address::from_hex(to_hex)?;

    // Build transaction
    let tx = Transaction {
        chain_id: 1, // Would be fetched from node in production
        nonce,
        from: from_addr,
        to: Some(to_addr),
        value: amount,
        tx_type: TxType::Transfer,
        gas_limit: 21000,
        gas_price: 1,
        data: vec![],
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let signed = SignedTransaction::new(tx, &keypair);

    // Submit via API
    let body = serde_json::json!({
        "from": from_hex,
        "to": to_hex,
        "value": amount.to_string(),
        "gas_limit": 21000,
        "gas_price": 1,
        "nonce": nonce,
        "tx_type": "Transfer",
        "data": "",
        "public_key": keypair.public_key().to_hex(),
        "signature": signed.signature.to_hex(),
    });

    let result: serde_json::Value = client.submit_transaction(&body).await?;

    println!("Transaction submitted!");
    println!("  Hash: {}", signed.hash());
    println!("  From: {}", from_addr);
    println!("  To:   {}", to_addr);
    println!("  Amount: {}", amount);

    if let Some(data) = result.get("data") {
        if let Some(tx_hash) = data.get("tx_hash") {
            println!("  Confirmed hash: {}", tx_hash);
        }
    }

    Ok(())
}
