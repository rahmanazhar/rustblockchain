use crate::error::CoreError;
use crate::types::*;
use rustchain_crypto::{hash, Address, Blake3Hash, KeyPair, PublicKey, Signature};
use serde::{Deserialize, Serialize};

/// The type of transaction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TxType {
    /// Native token transfer.
    Transfer,
    /// Deploy a WASM smart contract.
    ContractDeploy,
    /// Call a deployed smart contract.
    ContractCall,
    /// Stake tokens to become a validator.
    Stake,
    /// Unstake tokens.
    Unstake,
}

/// Unsigned transaction data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub chain_id: ChainId,
    pub nonce: Nonce,
    pub from: Address,
    pub to: Option<Address>,
    pub value: Balance,
    pub tx_type: TxType,
    pub gas_limit: Gas,
    pub gas_price: GasPrice,
    pub data: Vec<u8>,
    pub timestamp: Timestamp,
}

/// A transaction with its signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedTransaction {
    pub transaction: Transaction,
    pub public_key: PublicKey,
    pub signature: Signature,
    pub tx_hash: Blake3Hash,
}

impl Transaction {
    /// Compute the hash used for signing (all fields in canonical order).
    pub fn signing_hash(&self) -> Blake3Hash {
        let encoded = bincode::serialize(self).expect("transaction serialization cannot fail");
        hash(&encoded)
    }

    /// Basic validation without state access.
    pub fn validate_basic(&self) -> Result<(), CoreError> {
        if self.data.len() > MAX_TRANSACTION_DATA_SIZE {
            return Err(CoreError::DataTooLarge {
                max: MAX_TRANSACTION_DATA_SIZE,
                got: self.data.len(),
            });
        }

        if self.gas_limit == 0 {
            return Err(CoreError::InvalidTransaction(
                "gas limit must be > 0".to_string(),
            ));
        }

        if self.gas_price == 0 {
            return Err(CoreError::InvalidTransaction(
                "gas price must be > 0".to_string(),
            ));
        }

        match self.tx_type {
            TxType::Transfer => {
                if self.to.is_none() {
                    return Err(CoreError::InvalidTransaction(
                        "transfer must have a recipient".to_string(),
                    ));
                }
            }
            TxType::ContractDeploy => {
                if self.to.is_some() {
                    return Err(CoreError::InvalidTransaction(
                        "contract deploy must not have a recipient".to_string(),
                    ));
                }
                if self.data.is_empty() {
                    return Err(CoreError::InvalidTransaction(
                        "contract deploy must include bytecode".to_string(),
                    ));
                }
            }
            TxType::ContractCall => {
                if self.to.is_none() {
                    return Err(CoreError::InvalidTransaction(
                        "contract call must have a target address".to_string(),
                    ));
                }
            }
            TxType::Stake => {
                if self.value == 0 {
                    return Err(CoreError::InvalidTransaction(
                        "stake amount must be > 0".to_string(),
                    ));
                }
            }
            TxType::Unstake => {
                if self.value == 0 {
                    return Err(CoreError::InvalidTransaction(
                        "unstake amount must be > 0".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Total gas cost (gas_limit * gas_price).
    pub fn max_gas_cost(&self) -> Balance {
        self.gas_limit as Balance * self.gas_price as Balance
    }

    /// Total cost (value + max gas cost).
    pub fn total_cost(&self) -> Balance {
        self.value + self.max_gas_cost()
    }
}

impl SignedTransaction {
    /// Create a new signed transaction.
    pub fn new(tx: Transaction, keypair: &KeyPair) -> Self {
        let signing_hash = tx.signing_hash();
        let signature = keypair.sign(signing_hash.as_bytes());
        let public_key = keypair.public_key();

        // tx_hash includes the signature
        let mut hash_input = bincode::serialize(&tx).expect("serialization cannot fail");
        hash_input.extend_from_slice(signature.as_bytes());
        let tx_hash = hash(&hash_input);

        Self {
            transaction: tx,
            public_key,
            signature,
            tx_hash,
        }
    }

    /// Verify the transaction signature.
    pub fn verify(&self) -> Result<(), CoreError> {
        self.transaction.validate_basic()?;

        // Verify that public key matches the from address
        let derived_addr = self.public_key.to_address();
        if derived_addr != self.transaction.from {
            return Err(CoreError::InvalidTransaction(
                "sender address does not match public key".to_string(),
            ));
        }

        // Verify the signature
        let signing_hash = self.transaction.signing_hash();
        self.public_key
            .verify(signing_hash.as_bytes(), &self.signature)?;

        Ok(())
    }

    /// Get the transaction hash.
    pub fn hash(&self) -> Blake3Hash {
        self.tx_hash
    }

    /// Get the sender address.
    pub fn sender(&self) -> Address {
        self.transaction.from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_transfer(keypair: &KeyPair) -> Transaction {
        Transaction {
            chain_id: 1,
            nonce: 0,
            from: keypair.address(),
            to: Some(Address::ZERO),
            value: 1000,
            tx_type: TxType::Transfer,
            gas_limit: 21000,
            gas_price: 1,
            data: vec![],
            timestamp: 1000,
        }
    }

    #[test]
    fn test_sign_and_verify() {
        let kp = KeyPair::generate();
        let tx = make_transfer(&kp);
        let signed = SignedTransaction::new(tx, &kp);
        assert!(signed.verify().is_ok());
    }

    #[test]
    fn test_wrong_signer() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let tx = make_transfer(&kp1);
        // Sign with kp2 but from is kp1's address
        let signed = SignedTransaction::new(tx, &kp2);
        assert!(signed.verify().is_err());
    }

    #[test]
    fn test_transfer_requires_recipient() {
        let kp = KeyPair::generate();
        let mut tx = make_transfer(&kp);
        tx.to = None;
        assert!(tx.validate_basic().is_err());
    }

    #[test]
    fn test_deploy_requires_data() {
        let kp = KeyPair::generate();
        let tx = Transaction {
            chain_id: 1,
            nonce: 0,
            from: kp.address(),
            to: None,
            value: 0,
            tx_type: TxType::ContractDeploy,
            gas_limit: 100000,
            gas_price: 1,
            data: vec![],
            timestamp: 1000,
        };
        assert!(tx.validate_basic().is_err());
    }

    #[test]
    fn test_total_cost() {
        let kp = KeyPair::generate();
        let tx = make_transfer(&kp);
        assert_eq!(tx.total_cost(), 1000 + 21000);
    }

    #[test]
    fn test_tx_hash_deterministic() {
        let kp = KeyPair::generate();
        let tx1 = make_transfer(&kp);
        let tx2 = tx1.clone();
        let signed1 = SignedTransaction::new(tx1, &kp);
        let signed2 = SignedTransaction::new(tx2, &kp);
        // Ed25519 signing is deterministic
        assert_eq!(signed1.hash(), signed2.hash());
    }
}
