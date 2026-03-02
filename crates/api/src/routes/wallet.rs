use crate::dto::ApiResponse;
use crate::error::ApiError;
use crate::AppState;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use rustchain_core::{SignedTransaction, Transaction, TxType};
use rustchain_crypto::{Address, KeyPair, Keystore, Mnemonic};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ─── DTOs ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateAccountRequest {
    pub password: String,
    pub word_count: Option<usize>,
}

#[derive(Serialize)]
pub struct CreateAccountResponse {
    pub address: String,
    pub mnemonic: String,
    pub public_key: String,
    pub keystore: serde_json::Value,
}

#[derive(Deserialize)]
pub struct ImportMnemonicRequest {
    pub mnemonic: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ImportPrivateKeyRequest {
    pub private_key: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct ImportAccountResponse {
    pub address: String,
    pub public_key: String,
    pub keystore: serde_json::Value,
}

#[derive(Deserialize)]
pub struct WalletTransferRequest {
    pub keystore: serde_json::Value,
    pub password: String,
    pub to: String,
    pub value: String,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u64>,
}

#[derive(Deserialize)]
pub struct WalletStakeRequest {
    pub keystore: serde_json::Value,
    pub password: String,
    pub value: String,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u64>,
}

#[derive(Deserialize)]
pub struct WalletUnstakeRequest {
    pub keystore: serde_json::Value,
    pub password: String,
    pub value: String,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u64>,
}

#[derive(Deserialize)]
pub struct WalletDeployRequest {
    pub keystore: serde_json::Value,
    pub password: String,
    pub bytecode: String,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u64>,
}

#[derive(Deserialize)]
pub struct WalletCallRequest {
    pub keystore: serde_json::Value,
    pub password: String,
    pub contract: String,
    pub function: String,
    pub args: Option<String>,
    pub value: Option<String>,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u64>,
}

#[derive(Serialize)]
pub struct WalletTxResponse {
    pub tx_hash: String,
    pub from: String,
    pub nonce: u64,
}

// ─── Helpers ─────────────────────────────────────────────────

fn decrypt_keystore(keystore_json: &serde_json::Value, password: &str) -> Result<KeyPair, ApiError> {
    let keystore_bytes = serde_json::to_vec(keystore_json)
        .map_err(|e| ApiError::BadRequest(format!("Invalid keystore JSON: {}", e)))?;
    Keystore::decrypt(&keystore_bytes, password)
        .map_err(|e| ApiError::BadRequest(format!("Keystore decryption failed (wrong password?): {}", e)))
}

fn encrypt_keypair(keypair: &KeyPair, password: &str) -> Result<serde_json::Value, ApiError> {
    let encrypted = Keystore::encrypt(keypair, password)
        .map_err(|e| ApiError::Internal(format!("Keystore encryption failed: {}", e)))?;
    serde_json::from_slice(&encrypted)
        .map_err(|e| ApiError::Internal(format!("Keystore JSON parse failed: {}", e)))
}

fn get_nonce(state: &AppState, address: &Address) -> Result<u64, ApiError> {
    match state.storage.get_account(address)? {
        Some(account) => Ok(account.nonce),
        None => Ok(0),
    }
}

fn get_chain_id(state: &AppState) -> u64 {
    state.consensus.chain_info().chain_id
}

fn sign_and_submit(
    state: &AppState,
    keypair: &KeyPair,
    tx: Transaction,
) -> Result<WalletTxResponse, ApiError> {
    let signed = SignedTransaction::new(tx, keypair);
    signed
        .verify()
        .map_err(|e| ApiError::BadRequest(format!("Transaction verification failed: {}", e)))?;
    let tx_hash = signed.tx_hash.to_string();
    let from = signed.transaction.from.to_hex();
    let nonce = signed.transaction.nonce;
    state.consensus.on_transaction_received(signed)?;
    Ok(WalletTxResponse {
        tx_hash,
        from,
        nonce,
    })
}

// ─── Handlers ────────────────────────────────────────────────

/// POST /wallet/create
async fn create_account(
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<ApiResponse<CreateAccountResponse>>, ApiError> {
    let word_count = req.word_count.unwrap_or(12);
    if word_count != 12 && word_count != 24 {
        return Err(ApiError::BadRequest(
            "word_count must be 12 or 24".to_string(),
        ));
    }
    if req.password.is_empty() {
        return Err(ApiError::BadRequest("password is required".to_string()));
    }

    let mnemonic = Mnemonic::generate(word_count)
        .map_err(|e| ApiError::Internal(format!("Mnemonic generation failed: {}", e)))?;
    let keypair = mnemonic
        .to_keypair(&req.password)
        .map_err(|e| ApiError::Internal(format!("Key derivation failed: {}", e)))?;
    let keystore = encrypt_keypair(&keypair, &req.password)?;

    Ok(Json(ApiResponse::ok(CreateAccountResponse {
        address: keypair.address().to_hex(),
        mnemonic: mnemonic.phrase(),
        public_key: keypair.public_key().to_hex(),
        keystore,
    })))
}

/// POST /wallet/import/mnemonic
async fn import_mnemonic(
    Json(req): Json<ImportMnemonicRequest>,
) -> Result<Json<ApiResponse<ImportAccountResponse>>, ApiError> {
    if req.password.is_empty() {
        return Err(ApiError::BadRequest("password is required".to_string()));
    }

    let mnemonic = Mnemonic::from_phrase(&req.mnemonic)
        .map_err(|e| ApiError::BadRequest(format!("Invalid mnemonic: {}", e)))?;
    let keypair = mnemonic
        .to_keypair(&req.password)
        .map_err(|e| ApiError::Internal(format!("Key derivation failed: {}", e)))?;
    let keystore = encrypt_keypair(&keypair, &req.password)?;

    Ok(Json(ApiResponse::ok(ImportAccountResponse {
        address: keypair.address().to_hex(),
        public_key: keypair.public_key().to_hex(),
        keystore,
    })))
}

/// POST /wallet/import/private-key
async fn import_private_key(
    Json(req): Json<ImportPrivateKeyRequest>,
) -> Result<Json<ApiResponse<ImportAccountResponse>>, ApiError> {
    if req.password.is_empty() {
        return Err(ApiError::BadRequest("password is required".to_string()));
    }

    let key_hex = req.private_key.strip_prefix("0x").unwrap_or(&req.private_key);
    let key_bytes = hex::decode(key_hex)
        .map_err(|e| ApiError::BadRequest(format!("Invalid hex private key: {}", e)))?;
    if key_bytes.len() != 32 {
        return Err(ApiError::BadRequest(
            "Private key must be 32 bytes (64 hex chars)".to_string(),
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&key_bytes);
    let keypair = KeyPair::from_bytes(&arr)
        .map_err(|e| ApiError::BadRequest(format!("Invalid private key: {}", e)))?;
    let keystore = encrypt_keypair(&keypair, &req.password)?;

    Ok(Json(ApiResponse::ok(ImportAccountResponse {
        address: keypair.address().to_hex(),
        public_key: keypair.public_key().to_hex(),
        keystore,
    })))
}

/// POST /wallet/transfer
async fn send_transfer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WalletTransferRequest>,
) -> Result<Json<ApiResponse<WalletTxResponse>>, ApiError> {
    let keypair = decrypt_keystore(&req.keystore, &req.password)?;
    let from = keypair.address();
    let to = Address::from_hex(&req.to)
        .map_err(|e| ApiError::BadRequest(format!("Invalid to address: {}", e)))?;
    let value: u128 = req
        .value
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid value".to_string()))?;
    let chain_id = get_chain_id(&state);
    let nonce = get_nonce(&state, &from)?;

    let tx = Transaction {
        chain_id,
        nonce,
        from,
        to: Some(to),
        value,
        tx_type: TxType::Transfer,
        gas_limit: req.gas_limit.unwrap_or(21000),
        gas_price: req.gas_price.unwrap_or(1),
        data: vec![],
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let resp = sign_and_submit(&state, &keypair, tx)?;
    Ok(Json(ApiResponse::ok(resp)))
}

/// POST /wallet/stake
async fn send_stake(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WalletStakeRequest>,
) -> Result<Json<ApiResponse<WalletTxResponse>>, ApiError> {
    let keypair = decrypt_keystore(&req.keystore, &req.password)?;
    let from = keypair.address();
    let value: u128 = req
        .value
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid value".to_string()))?;
    let chain_id = get_chain_id(&state);
    let nonce = get_nonce(&state, &from)?;

    let tx = Transaction {
        chain_id,
        nonce,
        from,
        to: None,
        value,
        tx_type: TxType::Stake,
        gas_limit: req.gas_limit.unwrap_or(50000),
        gas_price: req.gas_price.unwrap_or(1),
        data: vec![],
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let resp = sign_and_submit(&state, &keypair, tx)?;
    Ok(Json(ApiResponse::ok(resp)))
}

/// POST /wallet/unstake
async fn send_unstake(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WalletUnstakeRequest>,
) -> Result<Json<ApiResponse<WalletTxResponse>>, ApiError> {
    let keypair = decrypt_keystore(&req.keystore, &req.password)?;
    let from = keypair.address();
    let value: u128 = req
        .value
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid value".to_string()))?;
    let chain_id = get_chain_id(&state);
    let nonce = get_nonce(&state, &from)?;

    let tx = Transaction {
        chain_id,
        nonce,
        from,
        to: None,
        value,
        tx_type: TxType::Unstake,
        gas_limit: req.gas_limit.unwrap_or(50000),
        gas_price: req.gas_price.unwrap_or(1),
        data: vec![],
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let resp = sign_and_submit(&state, &keypair, tx)?;
    Ok(Json(ApiResponse::ok(resp)))
}

/// POST /wallet/deploy
async fn deploy_contract(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WalletDeployRequest>,
) -> Result<Json<ApiResponse<WalletTxResponse>>, ApiError> {
    let keypair = decrypt_keystore(&req.keystore, &req.password)?;
    let from = keypair.address();
    let bytecode_hex = req.bytecode.strip_prefix("0x").unwrap_or(&req.bytecode);
    let bytecode = hex::decode(bytecode_hex)
        .map_err(|e| ApiError::BadRequest(format!("Invalid hex bytecode: {}", e)))?;
    if bytecode.is_empty() {
        return Err(ApiError::BadRequest(
            "Bytecode cannot be empty".to_string(),
        ));
    }
    let chain_id = get_chain_id(&state);
    let nonce = get_nonce(&state, &from)?;

    let tx = Transaction {
        chain_id,
        nonce,
        from,
        to: None,
        value: 0,
        tx_type: TxType::ContractDeploy,
        gas_limit: req.gas_limit.unwrap_or(1_000_000),
        gas_price: req.gas_price.unwrap_or(1),
        data: bytecode,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let resp = sign_and_submit(&state, &keypair, tx)?;
    Ok(Json(ApiResponse::ok(resp)))
}

/// POST /wallet/call
#[allow(clippy::too_many_arguments)]
async fn call_contract(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WalletCallRequest>,
) -> Result<Json<ApiResponse<WalletTxResponse>>, ApiError> {
    let keypair = decrypt_keystore(&req.keystore, &req.password)?;
    let from = keypair.address();
    let contract = Address::from_hex(&req.contract)
        .map_err(|e| ApiError::BadRequest(format!("Invalid contract address: {}", e)))?;
    let value: u128 = req
        .value
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid value".to_string()))?;

    // Encode call data: [func_name_len: 4 bytes LE][func_name][args]
    let func_bytes = req.function.as_bytes();
    let args_hex = req.args.as_deref().unwrap_or("");
    let args_hex = args_hex.strip_prefix("0x").unwrap_or(args_hex);
    let args = if args_hex.is_empty() {
        vec![]
    } else {
        hex::decode(args_hex)
            .map_err(|e| ApiError::BadRequest(format!("Invalid hex args: {}", e)))?
    };

    let mut data = Vec::new();
    data.extend_from_slice(&(func_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(func_bytes);
    data.extend_from_slice(&args);

    let chain_id = get_chain_id(&state);
    let nonce = get_nonce(&state, &from)?;

    let tx = Transaction {
        chain_id,
        nonce,
        from,
        to: Some(contract),
        value,
        tx_type: TxType::ContractCall,
        gas_limit: req.gas_limit.unwrap_or(500_000),
        gas_price: req.gas_price.unwrap_or(1),
        data,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let resp = sign_and_submit(&state, &keypair, tx)?;
    Ok(Json(ApiResponse::ok(resp)))
}

// ─── Router ──────────────────────────────────────────────────

pub fn wallet_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/create", post(create_account))
        .route("/import/mnemonic", post(import_mnemonic))
        .route("/import/private-key", post(import_private_key))
        .route("/transfer", post(send_transfer))
        .route("/stake", post(send_stake))
        .route("/unstake", post(send_unstake))
        .route("/deploy", post(deploy_contract))
        .route("/call", post(call_contract))
}
