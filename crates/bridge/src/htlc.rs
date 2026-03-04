//! Hash Time-Locked Contract (HTLC) implementation for atomic swaps.
//!
//! HTLCs enable trustless cross-chain token exchanges:
//! 1. Alice locks tokens on RustChain with a hash lock H(secret)
//! 2. Bob sees the lock and locks tokens on the other chain with the same H(secret)
//! 3. Alice claims Bob's tokens by revealing the secret
//! 4. Bob uses the revealed secret to claim Alice's tokens on RustChain
//!
//! If either party fails to act, the timelock expires and tokens are refunded.

use crate::error::BridgeError;
use crate::types::{ExternalChain, HtlcStatus};
use dashmap::DashMap;
use rustchain_crypto::Address;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// An individual HTLC swap record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtlcSwap {
    /// Unique swap ID (hex of hash_lock)
    pub id: String,
    /// SHA-256 hash lock — the hash of the secret preimage
    pub hash_lock: [u8; 32],
    /// The secret preimage (populated once claimed)
    pub preimage: Option<Vec<u8>>,
    /// Sender address on RustChain
    pub sender: Address,
    /// Recipient address on RustChain
    pub recipient: Address,
    /// Amount of RCT locked
    pub amount: u128,
    /// External chain involved in the swap
    pub external_chain: ExternalChain,
    /// Counterparty address on external chain
    pub external_address: String,
    /// External chain amount (informational, in external chain's smallest unit)
    pub external_amount: String,
    /// Timelock expiry (Unix timestamp in seconds)
    pub timelock: u64,
    /// Current status
    pub status: HtlcStatus,
    /// Creation timestamp
    pub created_at: u64,
    /// Claim/refund timestamp
    pub settled_at: Option<u64>,
}

/// Manages HTLC atomic swaps.
pub struct HtlcManager {
    /// Active and historical swaps keyed by swap ID
    swaps: Arc<DashMap<String, HtlcSwap>>,
    /// Swaps indexed by sender
    by_sender: Arc<DashMap<Address, Vec<String>>>,
    /// Swaps indexed by recipient
    by_recipient: Arc<DashMap<Address, Vec<String>>>,
    /// Default timelock duration in seconds
    default_timelock: u64,
}

impl HtlcManager {
    pub fn new(default_timelock: u64) -> Self {
        Self {
            swaps: Arc::new(DashMap::new()),
            by_sender: Arc::new(DashMap::new()),
            by_recipient: Arc::new(DashMap::new()),
            default_timelock,
        }
    }

    /// Compute SHA-256 hash of a preimage.
    pub fn hash_secret(preimage: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(preimage);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Create a new HTLC swap.
    ///
    /// The sender locks `amount` RCT tokens with a hash lock. The recipient
    /// can claim the tokens by revealing the preimage that hashes to `hash_lock`.
    /// If unclaimed before `timelock`, the sender can refund.
    #[allow(clippy::too_many_arguments)]
    pub fn create_swap(
        &self,
        sender: Address,
        recipient: Address,
        amount: u128,
        hash_lock: [u8; 32],
        external_chain: ExternalChain,
        external_address: String,
        external_amount: String,
        timelock: Option<u64>,
    ) -> Result<HtlcSwap, BridgeError> {
        let id = hex::encode(hash_lock);

        if self.swaps.contains_key(&id) {
            return Err(BridgeError::HtlcAlreadyExists(id));
        }

        if amount == 0 {
            return Err(BridgeError::InvalidAmount("Amount must be > 0".into()));
        }

        let now = chrono::Utc::now().timestamp() as u64;
        let timelock = timelock.unwrap_or(now + self.default_timelock);

        if timelock <= now {
            return Err(BridgeError::InvalidAmount(
                "Timelock must be in the future".into(),
            ));
        }

        let swap = HtlcSwap {
            id: id.clone(),
            hash_lock,
            preimage: None,
            sender,
            recipient,
            amount,
            external_chain,
            external_address,
            external_amount,
            timelock,
            status: HtlcStatus::Active,
            created_at: now,
            settled_at: None,
        };

        self.swaps.insert(id.clone(), swap.clone());
        self.by_sender
            .entry(sender)
            .or_default()
            .push(id.clone());
        self.by_recipient.entry(recipient).or_default().push(id);

        Ok(swap)
    }

    /// Claim an HTLC by revealing the preimage.
    ///
    /// The preimage must hash (SHA-256) to the stored hash_lock.
    /// Only callable by the recipient before the timelock expires.
    pub fn claim_swap(
        &self,
        swap_id: &str,
        preimage: Vec<u8>,
        claimer: &Address,
    ) -> Result<HtlcSwap, BridgeError> {
        let mut swap = self
            .swaps
            .get_mut(swap_id)
            .ok_or_else(|| BridgeError::HtlcNotFound(swap_id.to_string()))?;

        if swap.status == HtlcStatus::Claimed {
            return Err(BridgeError::HtlcAlreadyClaimed);
        }
        if swap.status == HtlcStatus::Refunded {
            return Err(BridgeError::HtlcAlreadyRefunded);
        }

        // Verify claimer is the recipient
        if claimer != &swap.recipient {
            return Err(BridgeError::Unauthorized(
                "Only the recipient can claim".into(),
            ));
        }

        // Check timelock
        let now = chrono::Utc::now().timestamp() as u64;
        if now >= swap.timelock {
            swap.status = HtlcStatus::Expired;
            return Err(BridgeError::HtlcExpired);
        }

        // Verify preimage
        let computed_hash = Self::hash_secret(&preimage);
        if computed_hash != swap.hash_lock {
            return Err(BridgeError::InvalidPreimage);
        }

        swap.preimage = Some(preimage);
        swap.status = HtlcStatus::Claimed;
        swap.settled_at = Some(now);

        Ok(swap.clone())
    }

    /// Refund an expired HTLC back to the sender.
    ///
    /// Only callable after the timelock has expired.
    pub fn refund_swap(
        &self,
        swap_id: &str,
        refunder: &Address,
    ) -> Result<HtlcSwap, BridgeError> {
        let mut swap = self
            .swaps
            .get_mut(swap_id)
            .ok_or_else(|| BridgeError::HtlcNotFound(swap_id.to_string()))?;

        if swap.status == HtlcStatus::Claimed {
            return Err(BridgeError::HtlcAlreadyClaimed);
        }
        if swap.status == HtlcStatus::Refunded {
            return Err(BridgeError::HtlcAlreadyRefunded);
        }

        // Verify refunder is the sender
        if refunder != &swap.sender {
            return Err(BridgeError::Unauthorized(
                "Only the sender can refund".into(),
            ));
        }

        // Check timelock has expired
        let now = chrono::Utc::now().timestamp() as u64;
        if now < swap.timelock {
            return Err(BridgeError::HtlcNotExpired);
        }

        swap.status = HtlcStatus::Refunded;
        swap.settled_at = Some(now);

        Ok(swap.clone())
    }

    /// Get a swap by ID.
    pub fn get_swap(&self, swap_id: &str) -> Option<HtlcSwap> {
        self.swaps.get(swap_id).map(|s| s.clone())
    }

    /// Get all swaps for a sender.
    pub fn get_swaps_by_sender(&self, sender: &Address) -> Vec<HtlcSwap> {
        self.by_sender
            .get(sender)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.swaps.get(id).map(|s| s.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all swaps for a recipient.
    pub fn get_swaps_by_recipient(&self, recipient: &Address) -> Vec<HtlcSwap> {
        self.by_recipient
            .get(recipient)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.swaps.get(id).map(|s| s.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List all swaps (most recent first).
    pub fn list_swaps(&self, limit: usize, offset: usize) -> Vec<HtlcSwap> {
        let mut swaps: Vec<HtlcSwap> = self.swaps.iter().map(|s| s.value().clone()).collect();
        swaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        swaps.into_iter().skip(offset).take(limit).collect()
    }

    /// Count active (unclaimed, unrefunded) swaps.
    pub fn active_count(&self) -> usize {
        self.swaps
            .iter()
            .filter(|s| s.status == HtlcStatus::Active)
            .count()
    }

    /// Expire all swaps past their timelock (batch maintenance).
    pub fn expire_stale_swaps(&self) -> usize {
        let now = chrono::Utc::now().timestamp() as u64;
        let mut expired = 0;
        for mut entry in self.swaps.iter_mut() {
            if entry.status == HtlcStatus::Active && now >= entry.timelock {
                entry.status = HtlcStatus::Expired;
                expired += 1;
            }
        }
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(byte: u8) -> Address {
        Address::from_bytes([byte; 20])
    }

    #[test]
    fn test_hash_secret() {
        let secret = b"my_secret_preimage";
        let hash1 = HtlcManager::hash_secret(secret);
        let hash2 = HtlcManager::hash_secret(secret);
        assert_eq!(hash1, hash2);

        let hash3 = HtlcManager::hash_secret(b"different");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_create_swap() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);
        let secret = b"test_secret";
        let hash_lock = HtlcManager::hash_secret(secret);

        let swap = mgr
            .create_swap(
                sender,
                recipient,
                1000,
                hash_lock,
                ExternalChain::Ethereum,
                "0xabc123".into(),
                "500000000000000000".into(), // 0.5 ETH in wei
                None,
            )
            .unwrap();

        assert_eq!(swap.status, HtlcStatus::Active);
        assert_eq!(swap.amount, 1000);
        assert_eq!(swap.sender, sender);
        assert_eq!(swap.recipient, recipient);
    }

    #[test]
    fn test_claim_swap() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);
        let secret = b"claim_test_secret";
        let hash_lock = HtlcManager::hash_secret(secret);

        let swap = mgr
            .create_swap(
                sender,
                recipient,
                500,
                hash_lock,
                ExternalChain::Bitcoin,
                "bc1q...".into(),
                "10000".into(),
                None,
            )
            .unwrap();

        // Claim with correct preimage
        let claimed = mgr
            .claim_swap(&swap.id, secret.to_vec(), &recipient)
            .unwrap();
        assert_eq!(claimed.status, HtlcStatus::Claimed);
        assert_eq!(claimed.preimage, Some(secret.to_vec()));
    }

    #[test]
    fn test_claim_wrong_preimage() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);
        let secret = b"real_secret";
        let hash_lock = HtlcManager::hash_secret(secret);

        let swap = mgr
            .create_swap(
                sender,
                recipient,
                100,
                hash_lock,
                ExternalChain::Solana,
                "So1...".into(),
                "1000000000".into(),
                None,
            )
            .unwrap();

        let result = mgr.claim_swap(&swap.id, b"wrong_secret".to_vec(), &recipient);
        assert!(matches!(result, Err(BridgeError::InvalidPreimage)));
    }

    #[test]
    fn test_claim_wrong_recipient() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);
        let impostor = test_address(3);
        let secret = b"auth_secret";
        let hash_lock = HtlcManager::hash_secret(secret);

        let swap = mgr
            .create_swap(
                sender,
                recipient,
                100,
                hash_lock,
                ExternalChain::Ethereum,
                "0x...".into(),
                "1".into(),
                None,
            )
            .unwrap();

        let result = mgr.claim_swap(&swap.id, secret.to_vec(), &impostor);
        assert!(matches!(result, Err(BridgeError::Unauthorized(_))));
    }

    #[test]
    fn test_refund_before_expiry_fails() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);
        let hash_lock = HtlcManager::hash_secret(b"s");

        let swap = mgr
            .create_swap(
                sender,
                recipient,
                100,
                hash_lock,
                ExternalChain::Polygon,
                "0x...".into(),
                "1".into(),
                None,
            )
            .unwrap();

        let result = mgr.refund_swap(&swap.id, &sender);
        assert!(matches!(result, Err(BridgeError::HtlcNotExpired)));
    }

    #[test]
    fn test_refund_expired_swap() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);
        let hash_lock = HtlcManager::hash_secret(b"expire_test");

        // Create with a timelock in the past
        let past = chrono::Utc::now().timestamp() as u64 - 100;
        // We need to manually insert since create validates timelock > now
        let id = hex::encode(hash_lock);
        let swap = HtlcSwap {
            id: id.clone(),
            hash_lock,
            preimage: None,
            sender,
            recipient,
            amount: 100,
            external_chain: ExternalChain::BnbChain,
            external_address: "0x...".into(),
            external_amount: "1".into(),
            timelock: past,
            status: HtlcStatus::Active,
            created_at: past - 3600,
            settled_at: None,
        };
        mgr.swaps.insert(id.clone(), swap);

        let refunded = mgr.refund_swap(&id, &sender).unwrap();
        assert_eq!(refunded.status, HtlcStatus::Refunded);
    }

    #[test]
    fn test_duplicate_swap_rejected() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);
        let hash_lock = HtlcManager::hash_secret(b"dup");

        mgr.create_swap(
            sender,
            recipient,
            100,
            hash_lock,
            ExternalChain::Ethereum,
            "0x...".into(),
            "1".into(),
            None,
        )
        .unwrap();

        let result = mgr.create_swap(
            sender,
            recipient,
            200,
            hash_lock,
            ExternalChain::Ethereum,
            "0x...".into(),
            "2".into(),
            None,
        );
        assert!(matches!(result, Err(BridgeError::HtlcAlreadyExists(_))));
    }

    #[test]
    fn test_list_and_count() {
        let mgr = HtlcManager::new(3600);
        let sender = test_address(1);
        let recipient = test_address(2);

        for i in 0..5u8 {
            let secret = vec![i; 16];
            let hash_lock = HtlcManager::hash_secret(&secret);
            mgr.create_swap(
                sender,
                recipient,
                (i as u128 + 1) * 100,
                hash_lock,
                ExternalChain::Ethereum,
                format!("0x{:02x}", i),
                "1".into(),
                None,
            )
            .unwrap();
        }

        assert_eq!(mgr.active_count(), 5);
        let all = mgr.list_swaps(10, 0);
        assert_eq!(all.len(), 5);

        let page = mgr.list_swaps(2, 2);
        assert_eq!(page.len(), 2);

        let sender_swaps = mgr.get_swaps_by_sender(&sender);
        assert_eq!(sender_swaps.len(), 5);
    }
}
