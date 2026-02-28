use crate::client::NodeClient;
use anyhow::Result;

/// Query account balance.
pub async fn query_balance(node_url: &str, address: &str) -> Result<()> {
    let client = NodeClient::new(node_url);
    let result: serde_json::Value = client.get_balance(address).await?;

    if let Some(data) = result.get("data") {
        if let Some(balance) = data.get("balance") {
            println!("Balance for {}: {}", address, balance);
        } else {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    } else {
        println!("Account not found");
    }

    Ok(())
}

/// Query chain information.
pub async fn query_chain_info(node_url: &str) -> Result<()> {
    let client = NodeClient::new(node_url);
    let result: serde_json::Value = client.get_chain_info().await?;

    if let Some(data) = result.get("data") {
        println!("Chain Information:");
        if let Some(v) = data.get("chain_id") {
            println!("  Chain ID:           {}", v);
        }
        if let Some(v) = data.get("height") {
            println!("  Height:             {}", v);
        }
        if let Some(v) = data.get("best_block_hash") {
            println!("  Best block hash:    {}", v);
        }
        if let Some(v) = data.get("epoch") {
            println!("  Current epoch:      {}", v);
        }
        if let Some(v) = data.get("finalized_height") {
            println!("  Finalized height:   {}", v);
        }
        if let Some(v) = data.get("pending_transactions") {
            println!("  Pending txs:        {}", v);
        }
        if let Some(v) = data.get("active_validators") {
            println!("  Active validators:  {}", v);
        }
    } else {
        println!("Failed to get chain info");
    }

    Ok(())
}

/// Query a specific block.
pub async fn query_block(node_url: &str, block_id: &str) -> Result<()> {
    let client = NodeClient::new(node_url);
    let result: serde_json::Value = client.get_block(block_id).await?;

    if let Some(data) = result.get("data") {
        println!("{}", serde_json::to_string_pretty(&data)?);
    } else {
        println!("Block not found");
    }

    Ok(())
}

/// Query a transaction by hash.
pub async fn query_transaction(node_url: &str, tx_hash: &str) -> Result<()> {
    let client = NodeClient::new(node_url);
    let result: serde_json::Value = client.get(&format!("/tx/{}", tx_hash)).await?;

    if let Some(data) = result.get("data") {
        println!("{}", serde_json::to_string_pretty(&data)?);
    } else {
        println!("Transaction not found");
    }

    Ok(())
}
