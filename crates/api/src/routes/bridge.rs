use crate::dto::ApiResponse;
use crate::error::ApiError;
use crate::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use rustchain_bridge::{
    BridgeConfig, BridgeFeeSchedule, BridgeLiquidity, BridgeRelayer, BridgeState,
    BridgeTransfer, BridgeValidatorSet, ChainRegistry, ExternalChain, HtlcManager, HtlcSwap,
};
use rustchain_crypto::Address;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ─── Shared bridge state (lazy singleton) ────────────────────

use parking_lot::RwLock;

/// Bridge-specific state, added to handler context via Axum's State.
pub struct BridgeAppState {
    pub app: Arc<AppState>,
    pub htlc: Arc<HtlcManager>,
    pub bridge_state: Arc<BridgeState>,
    pub relayer: Arc<RwLock<BridgeRelayer>>,
    pub registry: Arc<ChainRegistry>,
    pub validators: Arc<BridgeValidatorSet>,
}

// ─── DTOs ────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SupportedChainsResponse {
    pub chains: Vec<ChainInfoResponse>,
}

#[derive(Serialize)]
pub struct ChainInfoResponse {
    pub name: String,
    pub symbol: String,
    pub chain_id: u64,
    pub is_evm: bool,
    pub tokens: Vec<TokenInfo>,
}

#[derive(Serialize)]
pub struct TokenInfo {
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub enabled: bool,
    pub min_amount: String,
    pub max_amount: String,
    pub fee_bps: u16,
}

#[derive(Deserialize)]
pub struct CreateHtlcRequest {
    pub sender: String,
    pub recipient: String,
    pub amount: String,
    pub hash_lock: String,
    pub external_chain: String,
    pub external_address: String,
    pub external_amount: String,
    pub timelock: Option<u64>,
}

#[derive(Deserialize)]
pub struct ClaimHtlcRequest {
    pub swap_id: String,
    pub preimage: String,
    pub claimer: String,
}

#[derive(Deserialize)]
pub struct RefundHtlcRequest {
    pub swap_id: String,
    pub refunder: String,
}

#[derive(Deserialize)]
pub struct InitiateTransferRequest {
    pub sender: String,
    pub dest_chain: String,
    pub recipient: String,
    pub token_symbol: String,
    pub amount: String,
}

#[derive(Deserialize)]
pub struct ConfirmTransferRequest {
    pub transfer_id: String,
    pub validator: String,
}

#[derive(Serialize)]
pub struct BridgeStatsResponse {
    pub total_transfers: usize,
    pub pending_transfers: usize,
    pub active_htlcs: usize,
    pub supported_chains: usize,
    pub bridge_validators: usize,
    pub liquidity: Vec<BridgeLiquidity>,
}

// ─── Handlers ────────────────────────────────────────────────

/// GET /bridge/chains — list supported chains and their tokens
async fn list_chains(
    State(state): State<Arc<BridgeAppState>>,
) -> Json<ApiResponse<SupportedChainsResponse>> {
    let chains: Vec<ChainInfoResponse> = ExternalChain::all()
        .iter()
        .map(|chain| {
            let tokens = state
                .registry
                .tokens_for_chain(chain)
                .into_iter()
                .map(|t| TokenInfo {
                    symbol: t.symbol,
                    name: t.name,
                    decimals: t.decimals,
                    enabled: t.enabled,
                    min_amount: t.min_amount.to_string(),
                    max_amount: t.max_amount.to_string(),
                    fee_bps: t.fee_bps,
                })
                .collect();

            ChainInfoResponse {
                name: chain.name().into(),
                symbol: chain.symbol().into(),
                chain_id: chain.chain_id(),
                is_evm: chain.is_evm(),
                tokens,
            }
        })
        .collect();

    Json(ApiResponse::ok(SupportedChainsResponse { chains }))
}

/// GET /bridge/stats — bridge statistics
async fn bridge_stats(
    State(state): State<Arc<BridgeAppState>>,
) -> Json<ApiResponse<BridgeStatsResponse>> {
    let stats = BridgeStatsResponse {
        total_transfers: state.bridge_state.total_transfers(),
        pending_transfers: state.bridge_state.pending_transfers(),
        active_htlcs: state.htlc.active_count(),
        supported_chains: ExternalChain::all().len(),
        bridge_validators: state.validators.active_count(),
        liquidity: state.bridge_state.get_liquidity(),
    };
    Json(ApiResponse::ok(stats))
}

/// GET /bridge/fees — fee schedule
async fn fee_schedule(
    State(state): State<Arc<BridgeAppState>>,
) -> Json<ApiResponse<Vec<BridgeFeeSchedule>>> {
    let fees = state.relayer.read().get_fee_schedule();
    Json(ApiResponse::ok(fees))
}

/// GET /bridge/liquidity — bridge liquidity per chain
async fn liquidity(
    State(state): State<Arc<BridgeAppState>>,
) -> Json<ApiResponse<Vec<BridgeLiquidity>>> {
    Json(ApiResponse::ok(state.bridge_state.get_liquidity()))
}

/// GET /bridge/transfers — list bridge transfers
async fn list_transfers(
    State(state): State<Arc<BridgeAppState>>,
) -> Json<ApiResponse<Vec<BridgeTransfer>>> {
    let transfers = state.bridge_state.list_transfers(100, 0);
    Json(ApiResponse::ok(transfers))
}

/// GET /bridge/transfers/:id — get transfer by ID
async fn get_transfer(
    State(state): State<Arc<BridgeAppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<BridgeTransfer>>, ApiError> {
    let transfer = state
        .bridge_state
        .get_transfer(&id)
        .ok_or_else(|| ApiError::NotFound(format!("Transfer {} not found", id)))?;
    Ok(Json(ApiResponse::ok(transfer)))
}

/// POST /bridge/transfer — initiate a cross-chain transfer
async fn initiate_transfer(
    State(state): State<Arc<BridgeAppState>>,
    Json(req): Json<InitiateTransferRequest>,
) -> Result<Json<ApiResponse<BridgeTransfer>>, ApiError> {
    let sender = Address::from_hex(&req.sender)
        .map_err(|e| ApiError::BadRequest(format!("Invalid sender: {}", e)))?;
    let dest_chain = ExternalChain::from_name(&req.dest_chain)
        .ok_or_else(|| ApiError::BadRequest(format!("Unsupported chain: {}", req.dest_chain)))?;
    let amount: u128 = req
        .amount
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid amount".into()))?;

    if amount == 0 {
        return Err(ApiError::BadRequest("Amount must be > 0".into()));
    }

    let transfer = state
        .relayer
        .read()
        .process_outbound_request(sender, dest_chain, req.recipient, req.token_symbol, amount)
        .map_err(|e| ApiError::BadRequest(format!("Bridge error: {}", e)))?;

    Ok(Json(ApiResponse::ok(transfer)))
}

/// POST /bridge/transfer/confirm — validator confirms a transfer
async fn confirm_transfer(
    State(state): State<Arc<BridgeAppState>>,
    Json(req): Json<ConfirmTransferRequest>,
) -> Result<Json<ApiResponse<BridgeTransfer>>, ApiError> {
    let validator = Address::from_hex(&req.validator)
        .map_err(|e| ApiError::BadRequest(format!("Invalid validator: {}", e)))?;

    let transfer = state
        .relayer
        .read()
        .submit_confirmation(&req.transfer_id, &validator)
        .map_err(|e| ApiError::BadRequest(format!("Confirmation error: {}", e)))?;

    Ok(Json(ApiResponse::ok(transfer)))
}

// ─── HTLC endpoints ─────────────────────────────────────────

/// GET /bridge/htlc — list HTLC swaps
async fn list_htlcs(
    State(state): State<Arc<BridgeAppState>>,
) -> Json<ApiResponse<Vec<HtlcSwap>>> {
    let swaps = state.htlc.list_swaps(100, 0);
    Json(ApiResponse::ok(swaps))
}

/// GET /bridge/htlc/:id — get HTLC by ID
async fn get_htlc(
    State(state): State<Arc<BridgeAppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<HtlcSwap>>, ApiError> {
    let swap = state
        .htlc
        .get_swap(&id)
        .ok_or_else(|| ApiError::NotFound(format!("HTLC {} not found", id)))?;
    Ok(Json(ApiResponse::ok(swap)))
}

/// POST /bridge/htlc/create — create a new HTLC atomic swap
async fn create_htlc(
    State(state): State<Arc<BridgeAppState>>,
    Json(req): Json<CreateHtlcRequest>,
) -> Result<Json<ApiResponse<HtlcSwap>>, ApiError> {
    let sender = Address::from_hex(&req.sender)
        .map_err(|e| ApiError::BadRequest(format!("Invalid sender: {}", e)))?;
    let recipient = Address::from_hex(&req.recipient)
        .map_err(|e| ApiError::BadRequest(format!("Invalid recipient: {}", e)))?;
    let amount: u128 = req
        .amount
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid amount".into()))?;

    let hash_hex = req.hash_lock.strip_prefix("0x").unwrap_or(&req.hash_lock);
    let hash_bytes = hex::decode(hash_hex)
        .map_err(|e| ApiError::BadRequest(format!("Invalid hash_lock hex: {}", e)))?;
    if hash_bytes.len() != 32 {
        return Err(ApiError::BadRequest(
            "hash_lock must be 32 bytes (64 hex chars)".into(),
        ));
    }
    let mut hash_lock = [0u8; 32];
    hash_lock.copy_from_slice(&hash_bytes);

    let external_chain = ExternalChain::from_name(&req.external_chain)
        .ok_or_else(|| ApiError::BadRequest(format!("Unsupported chain: {}", req.external_chain)))?;

    let swap = state
        .htlc
        .create_swap(
            sender,
            recipient,
            amount,
            hash_lock,
            external_chain,
            req.external_address,
            req.external_amount,
            req.timelock,
        )
        .map_err(|e| ApiError::BadRequest(format!("HTLC error: {}", e)))?;

    Ok(Json(ApiResponse::ok(swap)))
}

/// POST /bridge/htlc/claim — claim an HTLC with preimage
async fn claim_htlc_handler(
    State(state): State<Arc<BridgeAppState>>,
    Json(req): Json<ClaimHtlcRequest>,
) -> Result<Json<ApiResponse<HtlcSwap>>, ApiError> {
    let claimer = Address::from_hex(&req.claimer)
        .map_err(|e| ApiError::BadRequest(format!("Invalid claimer: {}", e)))?;

    let preimage_hex = req.preimage.strip_prefix("0x").unwrap_or(&req.preimage);
    let preimage = hex::decode(preimage_hex)
        .map_err(|e| ApiError::BadRequest(format!("Invalid preimage hex: {}", e)))?;

    let swap = state
        .htlc
        .claim_swap(&req.swap_id, preimage, &claimer)
        .map_err(|e| ApiError::BadRequest(format!("Claim error: {}", e)))?;

    Ok(Json(ApiResponse::ok(swap)))
}

/// POST /bridge/htlc/refund — refund an expired HTLC
async fn refund_htlc_handler(
    State(state): State<Arc<BridgeAppState>>,
    Json(req): Json<RefundHtlcRequest>,
) -> Result<Json<ApiResponse<HtlcSwap>>, ApiError> {
    let refunder = Address::from_hex(&req.refunder)
        .map_err(|e| ApiError::BadRequest(format!("Invalid refunder: {}", e)))?;

    let swap = state
        .htlc
        .refund_swap(&req.swap_id, &refunder)
        .map_err(|e| ApiError::BadRequest(format!("Refund error: {}", e)))?;

    Ok(Json(ApiResponse::ok(swap)))
}

// ─── Validators ─────────────────────────────────────────────

/// GET /bridge/validators — list bridge validators
async fn list_validators(
    State(state): State<Arc<BridgeAppState>>,
) -> Json<ApiResponse<Vec<rustchain_bridge::validator::BridgeValidator>>> {
    let validators = state.validators.list_validators();
    Json(ApiResponse::ok(validators))
}

// ─── Router ─────────────────────────────────────────────────

pub fn bridge_router(app_state: Arc<AppState>) -> Router {
    // Create bridge components
    let htlc = Arc::new(HtlcManager::new(3600));
    let bridge_state = Arc::new(BridgeState::new(BridgeConfig::default()));
    let validators = Arc::new(BridgeValidatorSet::new(2));
    let relayer = Arc::new(RwLock::new(BridgeRelayer::new(
        bridge_state.clone(),
        validators.clone(),
    )));
    let registry = Arc::new(ChainRegistry::new());

    let bridge_app_state = Arc::new(BridgeAppState {
        app: app_state,
        htlc,
        bridge_state,
        relayer,
        registry,
        validators,
    });

    Router::new()
        // Chain info
        .route("/chains", get(list_chains))
        .route("/stats", get(bridge_stats))
        .route("/fees", get(fee_schedule))
        .route("/liquidity", get(liquidity))
        // Transfers
        .route("/transfers", get(list_transfers))
        .route("/transfers/:id", get(get_transfer))
        .route("/transfer", post(initiate_transfer))
        .route("/transfer/confirm", post(confirm_transfer))
        // HTLC
        .route("/htlc", get(list_htlcs))
        .route("/htlc/:id", get(get_htlc))
        .route("/htlc/create", post(create_htlc))
        .route("/htlc/claim", post(claim_htlc_handler))
        .route("/htlc/refund", post(refund_htlc_handler))
        // Validators
        .route("/validators", get(list_validators))
        .with_state(bridge_app_state)
}
