use crate::context::ExecutionContext;
use crate::error::VmError;
use crate::gas::GasCosts;
use dashmap::DashMap;
use rustchain_core::{EventLog, TxStatus};
use rustchain_crypto::{hash, Address, Blake3Hash};
use serde::{Deserialize, Serialize};
use wasmtime::*;

/// Configuration for the WASM VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub max_memory_pages: u32,
    pub max_call_depth: u32,
    pub max_code_size: usize,
    pub max_stack_size: usize,
    pub fuel_metering: bool,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_memory_pages: 256,   // 16 MB
            max_call_depth: 256,
            max_code_size: 256 * 1024, // 256 KB
            max_stack_size: 1024 * 1024, // 1 MB
            fuel_metering: true,
        }
    }
}

/// Result of a contract deployment.
pub struct DeployResult {
    pub contract_address: Address,
    pub code_hash: Blake3Hash,
    pub gas_used: u64,
    pub logs: Vec<EventLog>,
}

/// Result of a contract call.
pub struct CallResult {
    pub return_data: Vec<u8>,
    pub gas_used: u64,
    pub logs: Vec<EventLog>,
    pub status: TxStatus,
}

/// Host state passed to WASM functions.
pub struct HostState {
    pub context: ExecutionContext,
    pub gas_costs: GasCosts,
}

/// The main WASM execution engine.
pub struct WasmEngine {
    engine: Engine,
    module_cache: DashMap<Blake3Hash, Module>,
    config: VmConfig,
    gas_costs: GasCosts,
}

impl WasmEngine {
    pub fn new(config: &VmConfig) -> Result<Self, VmError> {
        let mut engine_config = Config::new();
        engine_config.wasm_bulk_memory(true);
        engine_config.wasm_multi_value(true);

        if config.fuel_metering {
            engine_config.consume_fuel(true);
        }

        let engine =
            Engine::new(&engine_config).map_err(|e| VmError::Compilation(e.to_string()))?;

        Ok(Self {
            engine,
            module_cache: DashMap::new(),
            config: config.clone(),
            gas_costs: GasCosts::default(),
        })
    }

    /// Validate WASM module bytecode.
    pub fn validate_module(&self, bytecode: &[u8]) -> Result<(), VmError> {
        if bytecode.len() > self.config.max_code_size {
            return Err(VmError::CodeTooLarge {
                max: self.config.max_code_size,
                got: bytecode.len(),
            });
        }

        Module::validate(&self.engine, bytecode)
            .map_err(|e| VmError::InvalidModule(e.to_string()))
    }

    /// Compile a WASM module (caches by code hash).
    pub fn compile_module(&self, bytecode: &[u8]) -> Result<(Module, Blake3Hash), VmError> {
        let code_hash = hash(bytecode);

        if let Some(cached) = self.module_cache.get(&code_hash) {
            return Ok((cached.clone(), code_hash));
        }

        self.validate_module(bytecode)?;

        let module = Module::new(&self.engine, bytecode)
            .map_err(|e| VmError::Compilation(e.to_string()))?;

        self.module_cache.insert(code_hash, module.clone());
        Ok((module, code_hash))
    }

    /// Deploy a new smart contract.
    pub fn deploy_contract(
        &self,
        code: &[u8],
        constructor_args: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<DeployResult, VmError> {
        let (module, code_hash) = self.compile_module(code)?;

        // Derive contract address from sender + nonce
        let sender_bytes = context.caller.as_bytes();
        let nonce_bytes = context.block_number.to_le_bytes();
        let addr_hash = rustchain_crypto::hash_multiple(&[sender_bytes, &nonce_bytes]);
        let mut addr_bytes = [0u8; 20];
        addr_bytes.copy_from_slice(&addr_hash.as_bytes()[..20]);
        let contract_address = Address::from_bytes(addr_bytes);

        context.contract_address = contract_address;

        let gas_before = context.gas_meter.consumed();

        // If there's a constructor (init function), call it
        if let Err(e) = self.execute_function(&module, "init", constructor_args, context) {
            // If constructor isn't found, that's okay (no init needed)
            if !matches!(e, VmError::FunctionNotFound(_)) {
                return Err(e);
            }
        }

        let gas_used = context.gas_meter.consumed() - gas_before;

        Ok(DeployResult {
            contract_address,
            code_hash,
            gas_used,
            logs: std::mem::take(&mut context.logs),
        })
    }

    /// Call a function on a deployed contract.
    pub fn call_contract(
        &self,
        code_hash: &Blake3Hash,
        function: &str,
        args: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<CallResult, VmError> {
        let module = self
            .module_cache
            .get(code_hash)
            .map(|m| m.clone())
            .ok_or_else(|| VmError::ContractNotFound(code_hash.to_hex()))?;

        let gas_before = context.gas_meter.consumed();

        match self.execute_function(&module, function, args, context) {
            Ok(return_data) => {
                let gas_used = context.gas_meter.consumed() - gas_before;
                Ok(CallResult {
                    return_data,
                    gas_used,
                    logs: std::mem::take(&mut context.logs),
                    status: TxStatus::Success,
                })
            }
            Err(VmError::OutOfGas { limit, used: _ }) => {
                Ok(CallResult {
                    return_data: vec![],
                    gas_used: limit,
                    logs: vec![],
                    status: TxStatus::OutOfGas,
                })
            }
            Err(e) => {
                let err_gas_used = context.gas_meter.consumed() - gas_before;
                Ok(CallResult {
                    return_data: vec![],
                    gas_used: err_gas_used,
                    logs: vec![],
                    status: TxStatus::Failure(e.to_string()),
                })
            }
        }
    }

    fn execute_function(
        &self,
        module: &Module,
        function_name: &str,
        args: &[u8],
        context: &mut ExecutionContext,
    ) -> Result<Vec<u8>, VmError> {
        let mut store = Store::new(
            &self.engine,
            HostState {
                context: std::mem::replace(
                    context,
                    ExecutionContext::new(
                        Address::ZERO,
                        Address::ZERO,
                        Address::ZERO,
                        0,
                        0,
                        0,
                        0,
                        0,
                        context.state_reader.clone(),
                    ),
                ),
                gas_costs: self.gas_costs.clone(),
            },
        );

        if self.config.fuel_metering {
            let fuel = store.data().context.gas_meter.remaining();
            store.set_fuel(fuel).map_err(|e| VmError::Execution(e.to_string()))?;
        }

        let mut linker = Linker::new(&self.engine);
        self.register_host_functions(&mut linker)?;

        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|e| VmError::Execution(format!("{e}")))?;

        // Allocate memory for args if needed
        let return_data = if !args.is_empty() {
            // Try to call allocate function for args
            if let Ok(alloc) = instance.get_typed_func::<i32, i32>(&mut store, "allocate") {
                let ptr = alloc
                    .call(&mut store, args.len() as i32)
                    .map_err(|e| VmError::Execution(e.to_string()))?;

                // Write args to WASM memory
                let memory = instance
                    .get_memory(&mut store, "memory")
                    .ok_or_else(|| VmError::Execution("no memory export".to_string()))?;

                memory
                    .write(&mut store, ptr as usize, args)
                    .map_err(|_| VmError::MemoryOutOfBounds {
                        offset: ptr as u32,
                        len: args.len() as u32,
                        memory_size: memory.size(&store) as u32 * 65536,
                    })?;

                // Call the function with pointer and length
                let func = instance
                    .get_typed_func::<(i32, i32), i32>(&mut store, function_name)
                    .map_err(|_| VmError::FunctionNotFound(function_name.to_string()))?;

                let result = func
                    .call(&mut store, (ptr, args.len() as i32))
                    .map_err(|e| VmError::Trap(e.to_string()))?;

                vec![result as u8]
            } else {
                // No allocator; call with no args
                let func = instance
                    .get_typed_func::<(), i32>(&mut store, function_name)
                    .map_err(|_| VmError::FunctionNotFound(function_name.to_string()))?;

                let result = func
                    .call(&mut store, ())
                    .map_err(|e| VmError::Trap(e.to_string()))?;

                vec![result as u8]
            }
        } else {
            let func = instance
                .get_typed_func::<(), i32>(&mut store, function_name)
                .map_err(|_| VmError::FunctionNotFound(function_name.to_string()))?;

            let result = func
                .call(&mut store, ())
                .map_err(|e| VmError::Trap(e.to_string()))?;

            vec![result as u8]
        };

        // Update fuel consumed
        if self.config.fuel_metering {
            let _remaining = store.get_fuel().unwrap_or(0);
            // Gas meter was moved, we track via fuel
        }

        // Move context back
        let host_state = store.into_data();
        *context = host_state.context;

        Ok(return_data)
    }

    fn register_host_functions(&self, linker: &mut Linker<HostState>) -> Result<(), VmError> {
        // Storage read
        linker
            .func_wrap(
                "env",
                "host_storage_read",
                |mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32, val_ptr: i32| -> i32 {
                    let gas_cost = caller.data().gas_costs.storage_read;
                    if caller.data_mut().context.gas_meter.consume(gas_cost).is_err() {
                        return -1;
                    }

                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return -1,
                    };

                    let mut key = vec![0u8; key_len as usize];
                    if memory.read(&caller, key_ptr as usize, &mut key).is_err() {
                        return -1;
                    }

                    let addr = caller.data().context.contract_address;
                    match caller.data().context.storage_read(&addr, &key) {
                        Ok(Some(value)) => {
                            if memory.write(&mut caller, val_ptr as usize, &value).is_err() {
                                return -1;
                            }
                            value.len() as i32
                        }
                        _ => 0,
                    }
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Storage write
        linker
            .func_wrap(
                "env",
                "host_storage_write",
                |mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32| {
                    let gas_cost = caller.data().gas_costs.storage_write;
                    let _ = caller.data_mut().context.gas_meter.consume(gas_cost);

                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return,
                    };

                    let mut key = vec![0u8; key_len as usize];
                    let mut value = vec![0u8; val_len as usize];

                    if memory.read(&caller, key_ptr as usize, &mut key).is_err() {
                        return;
                    }
                    if memory.read(&caller, val_ptr as usize, &mut value).is_err() {
                        return;
                    }

                    let addr = caller.data().context.contract_address;
                    caller.data_mut().context.storage_write(addr, key, value);
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Get caller address
        linker
            .func_wrap(
                "env",
                "host_caller",
                |mut caller: Caller<'_, HostState>, out_ptr: i32| {
                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return,
                    };
                    let addr = caller.data().context.caller;
                    let _ = memory.write(&mut caller, out_ptr as usize, addr.as_bytes());
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Get self address
        linker
            .func_wrap(
                "env",
                "host_self_address",
                |mut caller: Caller<'_, HostState>, out_ptr: i32| {
                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return,
                    };
                    let addr = caller.data().context.contract_address;
                    let _ = memory.write(&mut caller, out_ptr as usize, addr.as_bytes());
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Get block number
        linker
            .func_wrap(
                "env",
                "host_block_number",
                |caller: Caller<'_, HostState>| -> i64 {
                    caller.data().context.block_number as i64
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Get block timestamp
        linker
            .func_wrap(
                "env",
                "host_block_timestamp",
                |caller: Caller<'_, HostState>| -> i64 {
                    caller.data().context.block_timestamp as i64
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Get chain ID
        linker
            .func_wrap(
                "env",
                "host_chain_id",
                |caller: Caller<'_, HostState>| -> i64 {
                    caller.data().context.chain_id as i64
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Get self balance
        linker
            .func_wrap(
                "env",
                "host_self_balance",
                |caller: Caller<'_, HostState>| -> i64 {
                    let addr = caller.data().context.contract_address;
                    caller.data().context.get_balance(&addr).unwrap_or(0) as i64
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Transfer
        linker
            .func_wrap(
                "env",
                "host_transfer",
                |mut caller: Caller<'_, HostState>, to_ptr: i32, amount: i64| -> i32 {
                    let gas_cost = caller.data().gas_costs.transfer;
                    if caller.data_mut().context.gas_meter.consume(gas_cost).is_err() {
                        return -1;
                    }

                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return -1,
                    };

                    let mut to_bytes = [0u8; 20];
                    if memory.read(&caller, to_ptr as usize, &mut to_bytes).is_err() {
                        return -1;
                    }

                    let from = caller.data().context.contract_address;
                    let to = Address::from_bytes(to_bytes);

                    match caller.data_mut().context.transfer(&from, &to, amount as u128) {
                        Ok(_) => 0,
                        Err(_) => -1,
                    }
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Emit event
        linker
            .func_wrap(
                "env",
                "host_emit_event",
                |mut caller: Caller<'_, HostState>,
                 _topics_ptr: i32,
                 topics_count: i32,
                 data_ptr: i32,
                 data_len: i32| {
                    let base_cost = caller.data().gas_costs.event_base;
                    let topic_cost = caller.data().gas_costs.event_per_topic * topics_count as u64;
                    let data_cost = caller.data().gas_costs.event_per_byte * data_len as u64;

                    if caller
                        .data_mut()
                        .context
                        .gas_meter
                        .consume(base_cost + topic_cost + data_cost)
                        .is_err()
                    {
                        return;
                    }

                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return,
                    };

                    let mut data = vec![0u8; data_len as usize];
                    if memory.read(&caller, data_ptr as usize, &mut data).is_err() {
                        return;
                    }

                    let addr = caller.data().context.contract_address;
                    let log = EventLog {
                        address: addr,
                        topics: vec![],
                        data,
                    };
                    caller.data_mut().context.emit_event(log);
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        // Abort
        linker
            .func_wrap(
                "env",
                "host_abort",
                |_caller: Caller<'_, HostState>, _msg_ptr: i32, _msg_len: i32| {
                    // The trap will be caught by the caller
                },
            )
            .map_err(|e| VmError::Execution(e.to_string()))?;

        Ok(())
    }

    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    pub fn gas_costs(&self) -> &GasCosts {
        &self.gas_costs
    }
}
