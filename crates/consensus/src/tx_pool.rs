use crate::error::ConsensusError;
use dashmap::DashMap;
use rustchain_core::{Gas, GasPrice, Nonce, SignedTransaction};
use rustchain_crypto::{Address, Blake3Hash};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use tracing::debug;

struct PooledTransaction {
    tx: SignedTransaction,
    added_at: Instant,
    gas_price: GasPrice,
}

/// Thread-safe transaction mempool.
pub struct TransactionPool {
    pending: DashMap<Blake3Hash, PooledTransaction>,
    by_sender: DashMap<Address, BTreeSet<(Nonce, Blake3Hash)>>,
    max_size: usize,
    min_gas_price: GasPrice,
}

impl TransactionPool {
    pub fn new(max_size: usize, min_gas_price: GasPrice) -> Self {
        Self {
            pending: DashMap::new(),
            by_sender: DashMap::new(),
            max_size,
            min_gas_price,
        }
    }

    /// Insert a transaction into the pool after basic validation.
    pub fn insert(&self, tx: SignedTransaction) -> Result<(), ConsensusError> {
        // Verify signature
        tx.verify().map_err(|e| {
            ConsensusError::TransactionPool(format!("invalid transaction: {}", e))
        })?;

        // Check gas price
        if tx.transaction.gas_price < self.min_gas_price {
            return Err(ConsensusError::TransactionPool(format!(
                "gas price {} below minimum {}",
                tx.transaction.gas_price, self.min_gas_price
            )));
        }

        // Check pool capacity
        if self.pending.len() >= self.max_size {
            return Err(ConsensusError::TransactionPool(
                "transaction pool is full".to_string(),
            ));
        }

        let tx_hash = tx.hash();
        if self.pending.contains_key(&tx_hash) {
            return Err(ConsensusError::TransactionPool(
                "duplicate transaction".to_string(),
            ));
        }

        let sender = tx.sender();
        let nonce = tx.transaction.nonce;
        let gas_price = tx.transaction.gas_price;

        self.pending.insert(
            tx_hash,
            PooledTransaction {
                tx,
                added_at: Instant::now(),
                gas_price,
            },
        );

        self.by_sender
            .entry(sender)
            .or_default()
            .insert((nonce, tx_hash));

        debug!("Transaction {} added to pool", tx_hash);
        Ok(())
    }

    /// Remove a transaction from the pool.
    pub fn remove(&self, hash: &Blake3Hash) -> Option<SignedTransaction> {
        if let Some((_, pooled)) = self.pending.remove(hash) {
            let sender = pooled.tx.sender();
            if let Some(mut set) = self.by_sender.get_mut(&sender) {
                set.remove(&(pooled.tx.transaction.nonce, *hash));
                if set.is_empty() {
                    drop(set);
                    self.by_sender.remove(&sender);
                }
            }
            Some(pooled.tx)
        } else {
            None
        }
    }

    /// Get a transaction by hash.
    pub fn get(&self, hash: &Blake3Hash) -> Option<SignedTransaction> {
        self.pending.get(hash).map(|p| p.tx.clone())
    }

    /// Number of pending transactions.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Whether the pool contains a transaction.
    pub fn contains(&self, hash: &Blake3Hash) -> bool {
        self.pending.contains_key(hash)
    }

    /// Select the best transactions for a block.
    /// Returns transactions sorted by gas price (descending), respecting nonce ordering.
    pub fn select_transactions(&self, gas_limit: Gas, max_count: usize) -> Vec<SignedTransaction> {
        let mut candidates: Vec<(GasPrice, Nonce, Blake3Hash)> = self
            .pending
            .iter()
            .map(|entry| {
                let p = entry.value();
                (p.gas_price, p.tx.transaction.nonce, *entry.key())
            })
            .collect();

        // Sort by gas price descending, then nonce ascending
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut selected = Vec::new();
        let mut total_gas: Gas = 0;

        for (_gas_price, _nonce, hash) in candidates {
            if selected.len() >= max_count {
                break;
            }

            if let Some(entry) = self.pending.get(&hash) {
                let tx = &entry.tx;
                let tx_gas = tx.transaction.gas_limit;

                if total_gas + tx_gas > gas_limit {
                    continue;
                }

                total_gas += tx_gas;
                selected.push(tx.clone());
            }
        }

        selected
    }

    /// Remove transactions that have been committed in a block.
    pub fn prune_committed(&self, tx_hashes: &[Blake3Hash]) {
        for hash in tx_hashes {
            self.remove(hash);
        }
    }

    /// Remove transactions older than a given age.
    pub fn prune_expired(&self, max_age: Duration) {
        let now = Instant::now();
        let to_remove: Vec<Blake3Hash> = self
            .pending
            .iter()
            .filter(|entry| now.duration_since(entry.value().added_at) > max_age)
            .map(|entry| *entry.key())
            .collect();

        for hash in to_remove {
            self.remove(&hash);
        }
    }

    /// Get all pending transaction hashes.
    pub fn pending_hashes(&self) -> Vec<Blake3Hash> {
        self.pending.iter().map(|e| *e.key()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_core::{Transaction, TxType};
    use rustchain_crypto::KeyPair;

    fn make_signed_tx(kp: &KeyPair, nonce: u64, gas_price: u64) -> SignedTransaction {
        let tx = Transaction {
            chain_id: 1,
            nonce,
            from: kp.address(),
            to: Some(Address::ZERO),
            value: 100,
            tx_type: TxType::Transfer,
            gas_limit: 21000,
            gas_price,
            data: vec![],
            timestamp: 1000,
        };
        SignedTransaction::new(tx, kp)
    }

    #[test]
    fn test_insert_and_get() {
        let pool = TransactionPool::new(100, 1);
        let kp = KeyPair::generate();
        let tx = make_signed_tx(&kp, 0, 10);
        let hash = tx.hash();

        pool.insert(tx).unwrap();
        assert!(pool.contains(&hash));
        assert_eq!(pool.pending_count(), 1);
    }

    #[test]
    fn test_duplicate_rejection() {
        let pool = TransactionPool::new(100, 1);
        let kp = KeyPair::generate();
        let tx = make_signed_tx(&kp, 0, 10);

        pool.insert(tx.clone()).unwrap();
        assert!(pool.insert(tx).is_err());
    }

    #[test]
    fn test_gas_price_minimum() {
        let pool = TransactionPool::new(100, 5);
        let kp = KeyPair::generate();
        let tx = make_signed_tx(&kp, 0, 1);
        assert!(pool.insert(tx).is_err());
    }

    #[test]
    fn test_select_transactions() {
        let pool = TransactionPool::new(100, 1);
        let kp = KeyPair::generate();

        for i in 0..5 {
            let tx = make_signed_tx(&kp, i, 10 - i);
            pool.insert(tx).unwrap();
        }

        let selected = pool.select_transactions(100000, 3);
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_remove_transaction() {
        let pool = TransactionPool::new(100, 1);
        let kp = KeyPair::generate();
        let tx = make_signed_tx(&kp, 0, 10);
        let hash = tx.hash();

        pool.insert(tx).unwrap();
        assert!(pool.remove(&hash).is_some());
        assert!(!pool.contains(&hash));
        assert_eq!(pool.pending_count(), 0);
    }
}
