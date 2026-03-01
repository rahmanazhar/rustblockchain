use crate::error::ConsensusError;
use rustchain_core::*;
use rustchain_crypto::{Address, Blake3Hash};
use rustchain_storage::ChainDatabase;
use rustchain_vm::{ExecutionContext, StateReader, VmError, WasmEngine};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// In-memory state reader backed by the database.
pub struct DbStateReader {
    db: Arc<ChainDatabase>,
}

impl DbStateReader {
    pub fn new(db: Arc<ChainDatabase>) -> Self {
        Self { db }
    }
}

impl StateReader for DbStateReader {
    fn get_account(&self, addr: &Address) -> Result<Option<Account>, VmError> {
        self.db
            .get_account(addr)
            .map_err(|e| VmError::State(e.to_string()))
    }

    fn get_storage(&self, addr: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, VmError> {
        self.db
            .get_contract_storage(addr, key)
            .map_err(|e| VmError::State(e.to_string()))
    }

    fn get_code(&self, code_hash: &Blake3Hash) -> Result<Option<Vec<u8>>, VmError> {
        self.db
            .get_contract_code(code_hash)
            .map_err(|e| VmError::State(e.to_string()))
    }
}

/// Manages the authoritative chain state.
pub struct ChainState {
    head: Block,
    height: BlockNumber,
    current_epoch: EpochInfo,
    storage: Arc<ChainDatabase>,
}

impl ChainState {
    /// Initialize from genesis.
    pub fn from_genesis(
        genesis: &GenesisConfig,
        storage: Arc<ChainDatabase>,
    ) -> Result<Self, ConsensusError> {
        genesis.validate()?;

        let genesis_block = genesis.to_genesis_block();
        let genesis_hash = genesis_block.hash();

        // Check if already initialized
        if let Some(existing_hash) = storage.get_genesis_hash()? {
            if existing_hash != genesis_hash {
                return Err(ConsensusError::StateTransition(
                    "genesis hash mismatch with existing database".to_string(),
                ));
            }

            // Already initialized, load current state
            let height = storage.get_chain_height()?.unwrap_or(0);
            let head = storage
                .get_block_by_number(height)?
                .ok_or_else(|| ConsensusError::StateTransition("head block not found".to_string()))?;

            let epoch_number = height / genesis.consensus_params.epoch_length;
            let epoch_start = epoch_number * genesis.consensus_params.epoch_length;
            let epoch_end = epoch_start + genesis.consensus_params.epoch_length - 1;

            let validator_infos = genesis.to_validator_infos();
            let vs = ValidatorSet::new(
                validator_infos,
                genesis.consensus_params.min_stake,
                genesis.consensus_params.max_validators,
            );

            let current_epoch = EpochInfo {
                epoch_number,
                start_block: epoch_start,
                end_block: epoch_end,
                validator_set: vs,
                finalized_block: None,
            };

            info!("Loaded existing chain at height {}", height);
            return Ok(Self {
                head,
                height,
                current_epoch,
                storage,
            });
        }

        // First-time initialization
        storage.put_block(&genesis_block)?;
        storage.set_chain_height(0)?;
        storage.set_genesis_hash(&genesis_hash)?;

        // Create initial accounts
        for acct in &genesis.initial_accounts {
            let account = Account::new(acct.address, acct.balance);
            storage.put_account(&account)?;
        }

        // Create initial validators
        let validator_infos = genesis.to_validator_infos();
        for vi in &validator_infos {
            storage.put_validator(vi)?;
        }

        let vs = ValidatorSet::new(
            validator_infos,
            genesis.consensus_params.min_stake,
            genesis.consensus_params.max_validators,
        );

        let current_epoch = EpochInfo {
            epoch_number: 0,
            start_block: 0,
            end_block: genesis.consensus_params.epoch_length - 1,
            validator_set: vs,
            finalized_block: None,
        };

        storage.put_epoch(&current_epoch)?;

        info!("Initialized chain from genesis: {}", genesis.chain_name);

        Ok(Self {
            head: genesis_block,
            height: 0,
            current_epoch,
            storage,
        })
    }

    pub fn head(&self) -> &Block {
        &self.head
    }

    pub fn height(&self) -> BlockNumber {
        self.height
    }

    pub fn current_epoch(&self) -> &EpochInfo {
        &self.current_epoch
    }

    pub fn storage(&self) -> &Arc<ChainDatabase> {
        &self.storage
    }

    /// Flush VM balance_changes into the modified_accounts map.
    fn apply_balance_changes(
        &self,
        balance_changes: &HashMap<Address, i128>,
        modified_accounts: &mut HashMap<Address, Account>,
    ) {
        for (addr, delta) in balance_changes {
            let mut acct = modified_accounts
                .get(addr)
                .cloned()
                .or_else(|| self.storage.get_account(addr).ok().flatten())
                .unwrap_or_else(|| Account::new(*addr, 0));

            let new_balance = (acct.balance as i128 + delta).max(0) as Balance;
            acct.balance = new_balance;
            modified_accounts.insert(*addr, acct);
        }
    }

    /// Compute state root as a hash over all modified accounts (sorted by address).
    fn compute_state_root(modified_accounts: &HashMap<Address, Account>) -> Blake3Hash {
        if modified_accounts.is_empty() {
            return Blake3Hash::ZERO;
        }
        let mut sorted_addrs: Vec<&Address> = modified_accounts.keys().collect();
        sorted_addrs.sort();

        let mut hasher_input = Vec::new();
        for addr in sorted_addrs {
            let acct = &modified_accounts[addr];
            hasher_input.extend_from_slice(addr.as_bytes());
            hasher_input.extend_from_slice(&acct.balance.to_le_bytes());
            hasher_input.extend_from_slice(&acct.nonce.to_le_bytes());
        }
        rustchain_crypto::hash(&hasher_input)
    }

    /// Extract function name from contract call data.
    /// Format: first 4 bytes = function name length (u32 LE), followed by function name UTF-8 bytes,
    /// followed by the actual arguments.
    /// Falls back to treating the first bytes as a raw UTF-8 name if length decoding fails.
    fn extract_function_name(data: &[u8]) -> (String, &[u8]) {
        if data.len() >= 4 {
            let name_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            if name_len > 0 && name_len <= 256 && data.len() >= 4 + name_len {
                if let Ok(name) = std::str::from_utf8(&data[4..4 + name_len]) {
                    let args = &data[4 + name_len..];
                    return (name.to_string(), args);
                }
            }
        }
        // Fallback: treat entire data as args, use "call" as default function
        ("call".to_string(), data)
    }

    /// Apply a validated block to the state, executing all transactions.
    pub fn apply_block(
        &mut self,
        block: &Block,
        vm: &WasmEngine,
    ) -> Result<Vec<TransactionReceipt>, ConsensusError> {
        // Validate parent hash
        let expected_parent = self.head.hash();
        if block.header.parent_hash != expected_parent {
            return Err(ConsensusError::InvalidBlock(format!(
                "parent hash mismatch: expected {}, got {}",
                expected_parent, block.header.parent_hash
            )));
        }

        // Validate block number
        if block.header.number != self.height + 1 {
            return Err(ConsensusError::InvalidBlock(format!(
                "block number mismatch: expected {}, got {}",
                self.height + 1,
                block.header.number
            )));
        }

        let state_reader = Arc::new(DbStateReader::new(self.storage.clone()));
        let mut receipts = Vec::new();
        let mut total_gas_used: Gas = 0;
        let mut modified_accounts: HashMap<Address, Account> = HashMap::new();

        // Execute each transaction
        for (i, signed_tx) in block.transactions.iter().enumerate() {
            let tx = &signed_tx.transaction;
            let sender = signed_tx.sender();

            // Get or create sender account
            let mut sender_account = modified_accounts
                .get(&sender)
                .cloned()
                .or_else(|| self.storage.get_account(&sender).ok().flatten())
                .unwrap_or_else(|| Account::new(sender, 0));

            // Verify nonce
            if tx.nonce != sender_account.nonce {
                let receipt = TransactionReceipt {
                    tx_hash: signed_tx.hash(),
                    block_number: block.header.number,
                    block_hash: block.hash(),
                    index: i as u32,
                    status: TxStatus::Failure(format!(
                        "nonce mismatch: expected {}, got {}",
                        sender_account.nonce, tx.nonce
                    )),
                    gas_used: 0,
                    logs: vec![],
                    contract_address: None,
                    return_data: vec![],
                };
                receipts.push(receipt);
                continue;
            }

            // Check balance for gas + value
            let total_cost = tx.total_cost();
            if !sender_account.has_sufficient_balance(total_cost) {
                let receipt = TransactionReceipt {
                    tx_hash: signed_tx.hash(),
                    block_number: block.header.number,
                    block_hash: block.hash(),
                    index: i as u32,
                    status: TxStatus::Failure(format!(
                        "insufficient balance: have {}, need {}",
                        sender_account.balance, total_cost
                    )),
                    gas_used: 0,
                    logs: vec![],
                    contract_address: None,
                    return_data: vec![],
                };
                receipts.push(receipt);
                continue;
            }

            // Deduct gas cost upfront
            sender_account.balance -= tx.max_gas_cost();
            sender_account.increment_nonce();

            let (receipt, gas_used) = match tx.tx_type {
                TxType::Transfer => {
                    let to_addr = tx.to.unwrap_or(Address::ZERO);
                    let mut to_account = modified_accounts
                        .get(&to_addr)
                        .cloned()
                        .or_else(|| self.storage.get_account(&to_addr).ok().flatten())
                        .unwrap_or_else(|| Account::new(to_addr, 0));

                    sender_account.balance -= tx.value;
                    to_account.balance += tx.value;
                    modified_accounts.insert(to_addr, to_account);

                    let base_gas: Gas = 21000;
                    let gas_refund = tx.gas_limit.saturating_sub(base_gas) * tx.gas_price;
                    sender_account.balance += gas_refund as Balance;

                    let receipt = TransactionReceipt {
                        tx_hash: signed_tx.hash(),
                        block_number: block.header.number,
                        block_hash: block.hash(),
                        index: i as u32,
                        status: TxStatus::Success,
                        gas_used: base_gas,
                        logs: vec![],
                        contract_address: None,
                        return_data: vec![],
                    };
                    (receipt, base_gas)
                }
                TxType::ContractDeploy => {
                    let mut context = ExecutionContext::new(
                        sender,
                        sender,
                        Address::ZERO,
                        tx.value,
                        block.header.number,
                        block.header.timestamp,
                        block.header.chain_id,
                        tx.gas_limit,
                        state_reader.clone(),
                    );

                    match vm.deploy_contract(&tx.data, &[], &mut context) {
                        Ok(result) => {
                            let code_hash = result.code_hash;
                            let contract_addr = result.contract_address;

                            // Store contract code
                            let _ = self.storage.put_contract_code(&code_hash, &tx.data);

                            // Create contract account
                            let contract_account =
                                Account::new_contract(contract_addr, tx.value, code_hash);
                            modified_accounts.insert(contract_addr, contract_account);

                            // Apply storage changes
                            for ((addr, key), value) in &context.storage_changes {
                                match value {
                                    Some(v) => { let _ = self.storage.put_contract_storage(addr, key, v); },
                                    None => { let _ = self.storage.delete_contract_storage(addr, key); },
                                }
                            }

                            // Flush VM balance changes to modified accounts
                            self.apply_balance_changes(&context.balance_changes, &mut modified_accounts);

                            sender_account.balance -= tx.value;
                            let gas_refund = tx.gas_limit.saturating_sub(result.gas_used) * tx.gas_price;
                            sender_account.balance += gas_refund as Balance;

                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Success,
                                gas_used: result.gas_used,
                                logs: result.logs,
                                contract_address: Some(contract_addr),
                                return_data: vec![],
                            };
                            (receipt, result.gas_used)
                        }
                        Err(e) => {
                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Failure(e.to_string()),
                                gas_used: tx.gas_limit,
                                logs: vec![],
                                contract_address: None,
                                return_data: vec![],
                            };
                            (receipt, tx.gas_limit)
                        }
                    }
                }
                TxType::ContractCall => {
                    let to_addr = tx.to.unwrap_or(Address::ZERO);
                    let to_account = modified_accounts
                        .get(&to_addr)
                        .cloned()
                        .or_else(|| self.storage.get_account(&to_addr).ok().flatten());

                    if let Some(acct) = to_account {
                        if let Some(code_hash) = acct.code_hash {
                            let mut context = ExecutionContext::new(
                                sender,
                                sender,
                                to_addr,
                                tx.value,
                                block.header.number,
                                block.header.timestamp,
                                block.header.chain_id,
                                tx.gas_limit,
                                state_reader.clone(),
                            );

                            // Extract function name using length-prefixed encoding
                            let (func_name, call_args) = Self::extract_function_name(&tx.data);

                            match vm.call_contract(&code_hash, &func_name, call_args, &mut context) {
                                Ok(result) => {
                                    // Apply storage changes on success
                                    if matches!(result.status, TxStatus::Success) {
                                        for ((addr, key), value) in &context.storage_changes {
                                            match value {
                                                Some(v) => { let _ = self.storage.put_contract_storage(addr, key, v); },
                                                None => { let _ = self.storage.delete_contract_storage(addr, key); },
                                            }
                                        }

                                        // Flush VM balance changes to modified accounts
                                        self.apply_balance_changes(&context.balance_changes, &mut modified_accounts);
                                    }

                                    let gas_refund = tx.gas_limit.saturating_sub(result.gas_used) * tx.gas_price;
                                    sender_account.balance += gas_refund as Balance;

                                    let receipt = TransactionReceipt {
                                        tx_hash: signed_tx.hash(),
                                        block_number: block.header.number,
                                        block_hash: block.hash(),
                                        index: i as u32,
                                        status: result.status,
                                        gas_used: result.gas_used,
                                        logs: result.logs,
                                        contract_address: None,
                                        return_data: result.return_data,
                                    };
                                    (receipt, result.gas_used)
                                }
                                Err(e) => {
                                    let receipt = TransactionReceipt {
                                        tx_hash: signed_tx.hash(),
                                        block_number: block.header.number,
                                        block_hash: block.hash(),
                                        index: i as u32,
                                        status: TxStatus::Failure(e.to_string()),
                                        gas_used: tx.gas_limit,
                                        logs: vec![],
                                        contract_address: None,
                                        return_data: vec![],
                                    };
                                    (receipt, tx.gas_limit)
                                }
                            }
                        } else {
                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Failure("target is not a contract".to_string()),
                                gas_used: 21000,
                                logs: vec![],
                                contract_address: None,
                                return_data: vec![],
                            };
                            (receipt, 21000)
                        }
                    } else {
                        let receipt = TransactionReceipt {
                            tx_hash: signed_tx.hash(),
                            block_number: block.header.number,
                            block_hash: block.hash(),
                            index: i as u32,
                            status: TxStatus::Failure("contract not found".to_string()),
                            gas_used: 21000,
                            logs: vec![],
                            contract_address: None,
                            return_data: vec![],
                        };
                        (receipt, 21000)
                    }
                }
                TxType::Stake => {
                    let stake_amount = tx.value;
                    sender_account.balance -= stake_amount;

                    // Update or create validator in the current epoch's validator set
                    let pubkey = signed_tx.public_key;
                    if let Some(vi) = self.current_epoch.validator_set.get_validator_mut(&sender) {
                        vi.stake += stake_amount;
                        vi.is_active = true;
                        self.storage.put_validator(vi).ok();
                        info!("Validator {} increased stake to {}", sender, vi.stake);
                    } else {
                        let vi = ValidatorInfo::new(sender, pubkey, stake_amount);
                        self.storage.put_validator(&vi).ok();
                        self.current_epoch.validator_set.validators.push(vi);
                        info!("New validator {} registered with stake {}", sender, stake_amount);
                    }
                    self.current_epoch.validator_set.recalculate_total_stake();

                    let base_gas: Gas = 21000;
                    let gas_refund = tx.gas_limit.saturating_sub(base_gas) * tx.gas_price;
                    sender_account.balance += gas_refund as Balance;

                    let receipt = TransactionReceipt {
                        tx_hash: signed_tx.hash(),
                        block_number: block.header.number,
                        block_hash: block.hash(),
                        index: i as u32,
                        status: TxStatus::Success,
                        gas_used: base_gas,
                        logs: vec![],
                        contract_address: None,
                        return_data: vec![],
                    };
                    (receipt, base_gas)
                }
                TxType::Unstake => {
                    let unstake_amount = tx.value;
                    let base_gas: Gas = 21000;
                    let min_stake = self.current_epoch.validator_set.min_stake;

                    if let Some(vi) = self.current_epoch.validator_set.get_validator_mut(&sender) {
                        if vi.stake < unstake_amount {
                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Failure(format!(
                                    "insufficient stake: have {}, want to unstake {}",
                                    vi.stake, unstake_amount
                                )),
                                gas_used: base_gas,
                                logs: vec![],
                                contract_address: None,
                                return_data: vec![],
                            };
                            let gas_refund = tx.gas_limit.saturating_sub(base_gas) * tx.gas_price;
                            sender_account.balance += gas_refund as Balance;
                            (receipt, base_gas)
                        } else {
                            vi.stake -= unstake_amount;
                            // Deactivate if stake falls below minimum
                            if vi.stake < min_stake {
                                vi.is_active = false;
                                warn!("Validator {} deactivated: stake below minimum", sender);
                            }
                            self.storage.put_validator(vi).ok();
                            self.current_epoch.validator_set.recalculate_total_stake();

                            // Return unstaked tokens to sender
                            sender_account.balance += unstake_amount;

                            let gas_refund = tx.gas_limit.saturating_sub(base_gas) * tx.gas_price;
                            sender_account.balance += gas_refund as Balance;

                            info!("Validator {} unstaked {}", sender, unstake_amount);

                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Success,
                                gas_used: base_gas,
                                logs: vec![],
                                contract_address: None,
                                return_data: vec![],
                            };
                            (receipt, base_gas)
                        }
                    } else {
                        let gas_refund = tx.gas_limit.saturating_sub(base_gas) * tx.gas_price;
                        sender_account.balance += gas_refund as Balance;

                        let receipt = TransactionReceipt {
                            tx_hash: signed_tx.hash(),
                            block_number: block.header.number,
                            block_hash: block.hash(),
                            index: i as u32,
                            status: TxStatus::Failure("not a validator".to_string()),
                            gas_used: base_gas,
                            logs: vec![],
                            contract_address: None,
                            return_data: vec![],
                        };
                        (receipt, base_gas)
                    }
                }
            };

            total_gas_used += gas_used;
            modified_accounts.insert(sender, sender_account);
            receipts.push(receipt);
        }

        // Compute state root over modified accounts
        let state_root = Self::compute_state_root(&modified_accounts);

        // Commit all changes atomically
        let mut batch = rustchain_storage::BlockCommitBatch::new(&self.storage);
        batch.put_block(block)?;
        batch.put_transactions(&block.transactions, block.header.number)?;
        batch.put_receipts(&receipts)?;
        batch.update_accounts(&modified_accounts.values().cloned().collect::<Vec<_>>())?;
        batch.set_chain_height(block.header.number)?;
        batch.set_best_block_hash(&block.hash())?;
        batch.commit()?;

        self.head = block.clone();
        self.height = block.header.number;

        debug!(
            "Applied block {} with {} txs, gas used: {}, state_root: {}",
            block.header.number,
            block.transactions.len(),
            total_gas_used,
            state_root,
        );

        Ok(receipts)
    }

    /// Check and advance epoch if at epoch boundary.
    /// Recalculates the validator set from current staking state.
    pub fn maybe_advance_epoch(
        &mut self,
        consensus_params: &ConsensusParams,
    ) -> Result<Option<EpochInfo>, ConsensusError> {
        if !self.current_epoch.is_last_block(self.height) {
            return Ok(None);
        }

        let new_epoch_number = self.current_epoch.epoch_number + 1;
        let start_block = self.height + 1;
        let end_block = start_block + consensus_params.epoch_length - 1;

        // Rebuild validator set from persisted validators, filtering inactive/jailed
        let current_time = chrono::Utc::now().timestamp_millis() as u64;
        let mut new_validators = self.current_epoch.validator_set.validators.clone();

        // Unjail validators whose jail period has expired
        for vi in &mut new_validators {
            if let Some(until) = vi.jailed_until {
                if current_time >= until {
                    vi.jailed_until = None;
                    if vi.stake >= consensus_params.min_stake {
                        vi.is_active = true;
                    }
                }
            }
        }

        // Only keep validators above min stake
        new_validators.retain(|v| v.stake >= consensus_params.min_stake);

        // Sort by stake descending, truncate to max_validators
        new_validators.sort_by(|a, b| b.stake.cmp(&a.stake));
        new_validators.truncate(consensus_params.max_validators as usize);

        let new_vs = ValidatorSet::new(
            new_validators,
            consensus_params.min_stake,
            consensus_params.max_validators,
        );

        let new_epoch = EpochInfo {
            epoch_number: new_epoch_number,
            start_block,
            end_block,
            validator_set: new_vs,
            finalized_block: None,
        };

        self.storage.put_epoch(&new_epoch)?;
        self.current_epoch = new_epoch.clone();

        info!(
            "Advanced to epoch {} (blocks {}-{}) with {} active validators",
            new_epoch_number,
            start_block,
            end_block,
            self.current_epoch.validator_set.active_validators().len(),
        );

        Ok(Some(new_epoch))
    }
}
