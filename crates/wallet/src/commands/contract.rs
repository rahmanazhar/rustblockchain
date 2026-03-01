use crate::client::NodeClient;
use crate::commands::transfer::fetch_chain_id;
use anyhow::Result;
use rustchain_core::{SignedTransaction, Transaction, TxType};
use rustchain_crypto::{Address, Keystore};
use std::path::Path;

/// Deploy a WASM smart contract.
pub async fn deploy_contract(
    node_url: &str,
    keystore_dir: &Path,
    from_hex: &str,
    wasm_path: &Path,
    password: &str,
    gas_limit: u64,
) -> Result<()> {
    let client = NodeClient::new(node_url);
    let chain_id = fetch_chain_id(&client).await?;

    let from_addr = Address::from_hex(from_hex)?;
    let keyfile = keystore_dir.join(format!("{}.json", from_addr.to_hex()));
    if !keyfile.exists() {
        anyhow::bail!("Keyfile not found for address {}", from_addr);
    }
    let keypair = Keystore::load_from_file(&keyfile, password)?;

    let wasm_bytecode = std::fs::read(wasm_path)?;
    println!("Contract bytecode size: {} bytes", wasm_bytecode.len());

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
        value: 0,
        tx_type: TxType::ContractDeploy,
        gas_limit,
        gas_price: 1,
        data: wasm_bytecode,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let signed = SignedTransaction::new(tx, &keypair);

    let body = serde_json::json!({
        "chain_id": chain_id,
        "from": from_hex,
        "value": "0",
        "gas_limit": gas_limit,
        "gas_price": 1,
        "nonce": nonce,
        "tx_type": "ContractDeploy",
        "data": hex::encode(&signed.transaction.data),
        "timestamp": signed.transaction.timestamp,
        "public_key": keypair.public_key().to_hex(),
        "signature": signed.signature.to_hex(),
    });

    let result: serde_json::Value = client.submit_transaction(&body).await?;

    println!("Contract deployment submitted!");
    println!("  Hash: {}", signed.hash());
    println!("  From: {}", from_addr);

    if let Some(data) = result.get("data") {
        if let Some(tx_hash) = data.get("tx_hash") {
            println!("  Confirmed hash: {}", tx_hash);
        }
    }

    Ok(())
}

/// Call a function on a deployed contract.
#[allow(clippy::too_many_arguments)]
pub async fn call_contract(
    node_url: &str,
    keystore_dir: &Path,
    from_hex: &str,
    contract_hex: &str,
    function: &str,
    args_hex: &str,
    value: u128,
    password: &str,
    gas_limit: u64,
) -> Result<()> {
    let client = NodeClient::new(node_url);
    let chain_id = fetch_chain_id(&client).await?;

    let from_addr = Address::from_hex(from_hex)?;
    let contract_addr = Address::from_hex(contract_hex)?;

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

    // Encode call data: [func_name_len: 4 bytes LE][func_name: N bytes][args]
    let func_bytes = function.as_bytes();
    let args = hex::decode(args_hex.strip_prefix("0x").unwrap_or(args_hex))?;
    let mut data = Vec::new();
    data.extend_from_slice(&(func_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(func_bytes);
    data.extend_from_slice(&args);

    let tx = Transaction {
        chain_id,
        nonce,
        from: from_addr,
        to: Some(contract_addr),
        value,
        tx_type: TxType::ContractCall,
        gas_limit,
        gas_price: 1,
        data,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let signed = SignedTransaction::new(tx, &keypair);

    let body = serde_json::json!({
        "chain_id": chain_id,
        "from": from_hex,
        "to": contract_hex,
        "value": value.to_string(),
        "gas_limit": gas_limit,
        "gas_price": 1,
        "nonce": nonce,
        "tx_type": "ContractCall",
        "data": hex::encode(&signed.transaction.data),
        "timestamp": signed.transaction.timestamp,
        "public_key": keypair.public_key().to_hex(),
        "signature": signed.signature.to_hex(),
    });

    let result: serde_json::Value = client.submit_transaction(&body).await?;

    println!("Contract call submitted!");
    println!("  Hash: {}", signed.hash());
    println!("  From: {}", from_addr);
    println!("  Contract: {}", contract_addr);
    println!("  Function: {}", function);

    if let Some(data) = result.get("data") {
        if let Some(tx_hash) = data.get("tx_hash") {
            println!("  Confirmed hash: {}", tx_hash);
        }
    }

    Ok(())
}
