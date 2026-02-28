use crate::error::VmError;
use crate::gas::GasMeter;
use rustchain_core::{Account, Balance, BlockNumber, ChainId, EventLog, Timestamp};
use rustchain_crypto::{Address, Blake3Hash};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for reading state during VM execution.
pub trait StateReader: Send + Sync {
    fn get_account(&self, addr: &Address) -> Result<Option<Account>, VmError>;
    fn get_storage(&self, addr: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, VmError>;
    fn get_code(&self, code_hash: &Blake3Hash) -> Result<Option<Vec<u8>>, VmError>;
}

/// Execution context for a smart contract call.
pub struct ExecutionContext {
    pub caller: Address,
    pub origin: Address,
    pub contract_address: Address,
    pub value: Balance,
    pub block_number: BlockNumber,
    pub block_timestamp: Timestamp,
    pub chain_id: ChainId,
    pub gas_meter: GasMeter,
    pub call_depth: u32,
    pub max_call_depth: u32,
    pub logs: Vec<EventLog>,
    /// Journaled storage writes: (address, key) -> Option<value>. None = delete.
    pub storage_changes: HashMap<(Address, Vec<u8>), Option<Vec<u8>>>,
    /// Balance deltas for each address.
    pub balance_changes: HashMap<Address, i128>,
    pub state_reader: Arc<dyn StateReader>,
    /// Return data from the last cross-contract call.
    pub return_data: Vec<u8>,
}

impl ExecutionContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        caller: Address,
        origin: Address,
        contract_address: Address,
        value: Balance,
        block_number: BlockNumber,
        block_timestamp: Timestamp,
        chain_id: ChainId,
        gas_limit: u64,
        state_reader: Arc<dyn StateReader>,
    ) -> Self {
        Self {
            caller,
            origin,
            contract_address,
            value,
            block_number,
            block_timestamp,
            chain_id,
            gas_meter: GasMeter::new(gas_limit),
            call_depth: 0,
            max_call_depth: 256,
            logs: Vec::new(),
            storage_changes: HashMap::new(),
            balance_changes: HashMap::new(),
            state_reader,
            return_data: Vec::new(),
        }
    }

    /// Read storage, checking journaled changes first.
    pub fn storage_read(&self, addr: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, VmError> {
        let journal_key = (*addr, key.to_vec());
        if let Some(value) = self.storage_changes.get(&journal_key) {
            return Ok(value.clone());
        }
        self.state_reader.get_storage(addr, key)
    }

    /// Write to storage (journaled).
    pub fn storage_write(&mut self, addr: Address, key: Vec<u8>, value: Vec<u8>) {
        self.storage_changes.insert((addr, key), Some(value));
    }

    /// Delete from storage (journaled).
    pub fn storage_delete(&mut self, addr: Address, key: Vec<u8>) {
        self.storage_changes.insert((addr, key), None);
    }

    /// Get effective balance (base + deltas).
    pub fn get_balance(&self, addr: &Address) -> Result<Balance, VmError> {
        let base = self
            .state_reader
            .get_account(addr)?
            .map_or(0, |a| a.balance);
        let delta = self.balance_changes.get(addr).copied().unwrap_or(0);
        Ok((base as i128 + delta).max(0) as Balance)
    }

    /// Transfer value between addresses.
    pub fn transfer(&mut self, from: &Address, to: &Address, amount: Balance) -> Result<(), VmError> {
        let from_balance = self.get_balance(from)?;
        if from_balance < amount {
            return Err(VmError::HostFunction(format!(
                "insufficient balance: have {}, need {}",
                from_balance, amount
            )));
        }

        let from_delta = self.balance_changes.entry(*from).or_insert(0);
        *from_delta -= amount as i128;

        let to_delta = self.balance_changes.entry(*to).or_insert(0);
        *to_delta += amount as i128;

        Ok(())
    }

    /// Emit an event log.
    pub fn emit_event(&mut self, log: EventLog) {
        self.logs.push(log);
    }

    /// Check call depth.
    pub fn check_call_depth(&self) -> Result<(), VmError> {
        if self.call_depth >= self.max_call_depth {
            return Err(VmError::CallDepthExceeded {
                max: self.max_call_depth,
            });
        }
        Ok(())
    }
}
