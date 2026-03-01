use rustchain_core::*;
use rustchain_consensus::FinalityVote;
use rustchain_crypto::Blake3Hash;
use serde::{Deserialize, Serialize};

/// Messages exchanged between peers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkMessage {
    // Gossiped messages (broadcast)
    NewBlock(Block),
    NewTransaction(SignedTransaction),
    FinalityVote(FinalityVote),

    // Request/response (direct peer)
    BlockRequest {
        start: BlockNumber,
        count: u32,
    },
    BlockResponse {
        blocks: Vec<Block>,
    },
    StatusRequest,
    StatusResponse(PeerStatus),
}

/// Status information about a peer.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeerStatus {
    pub chain_id: ChainId,
    pub best_block: BlockNumber,
    pub best_hash: Blake3Hash,
    pub genesis_hash: Blake3Hash,
    pub protocol_version: u32,
}
