use crate::types::*;
use rustchain_crypto::{Address, Blake3Hash};
use serde::{Deserialize, Serialize};

/// Account state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub address: Address,
    pub balance: Balance,
    pub nonce: Nonce,
    /// Code hash for smart contract accounts. None for externally owned accounts.
    pub code_hash: Option<Blake3Hash>,
    /// Storage root hash (Merkle Patricia Trie root for contract storage).
    pub storage_root: Blake3Hash,
}

impl Account {
    /// Create a new externally owned account.
    pub fn new(address: Address, balance: Balance) -> Self {
        Self {
            address,
            balance,
            nonce: 0,
            code_hash: None,
            storage_root: Blake3Hash::ZERO,
        }
    }

    /// Create a new contract account.
    pub fn new_contract(address: Address, balance: Balance, code_hash: Blake3Hash) -> Self {
        Self {
            address,
            balance,
            nonce: 0,
            code_hash: Some(code_hash),
            storage_root: Blake3Hash::ZERO,
        }
    }

    /// Whether this is a contract account.
    pub fn is_contract(&self) -> bool {
        self.code_hash.is_some()
    }

    /// Whether the account has sufficient balance.
    pub fn has_sufficient_balance(&self, amount: Balance) -> bool {
        self.balance >= amount
    }

    /// Increment the nonce.
    pub fn increment_nonce(&mut self) {
        self.nonce += 1;
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            address: Address::ZERO,
            balance: 0,
            nonce: 0,
            code_hash: None,
            storage_root: Blake3Hash::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_crypto::KeyPair;

    #[test]
    fn test_new_account() {
        let kp = KeyPair::generate();
        let acct = Account::new(kp.address(), 1000);
        assert_eq!(acct.balance, 1000);
        assert_eq!(acct.nonce, 0);
        assert!(!acct.is_contract());
    }

    #[test]
    fn test_contract_account() {
        let kp = KeyPair::generate();
        let code_hash = rustchain_crypto::hash(b"contract code");
        let acct = Account::new_contract(kp.address(), 0, code_hash);
        assert!(acct.is_contract());
    }

    #[test]
    fn test_balance_check() {
        let acct = Account::new(Address::ZERO, 500);
        assert!(acct.has_sufficient_balance(500));
        assert!(!acct.has_sufficient_balance(501));
    }

    #[test]
    fn test_increment_nonce() {
        let mut acct = Account::new(Address::ZERO, 0);
        assert_eq!(acct.nonce, 0);
        acct.increment_nonce();
        assert_eq!(acct.nonce, 1);
    }
}
