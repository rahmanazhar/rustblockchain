//! RustChain Smart Contract SDK
//!
//! Provides safe Rust wrappers around the host function imports
//! for building WASM smart contracts.

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn host_storage_read(key_ptr: i32, key_len: i32, val_ptr: i32) -> i32;
    fn host_storage_write(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32);
    fn host_caller(out_ptr: i32);
    fn host_self_address(out_ptr: i32);
    fn host_block_number() -> i64;
    fn host_block_timestamp() -> i64;
    fn host_chain_id() -> i64;
    fn host_self_balance() -> i64;
    fn host_transfer(to_ptr: i32, amount: i64) -> i32;
    fn host_emit_event(topics_ptr: i32, topics_count: i32, data_ptr: i32, data_len: i32);
    fn host_abort(msg_ptr: i32, msg_len: i32);
}

/// Read a value from contract storage.
#[cfg(target_arch = "wasm32")]
pub fn storage_read(key: &[u8]) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; 4096];
    let len = unsafe {
        host_storage_read(
            key.as_ptr() as i32,
            key.len() as i32,
            buf.as_mut_ptr() as i32,
        )
    };
    if len > 0 {
        buf.truncate(len as usize);
        Some(buf)
    } else {
        None
    }
}

/// Write a value to contract storage.
#[cfg(target_arch = "wasm32")]
pub fn storage_write(key: &[u8], value: &[u8]) {
    unsafe {
        host_storage_write(
            key.as_ptr() as i32,
            key.len() as i32,
            value.as_ptr() as i32,
            value.len() as i32,
        );
    }
}

/// Get the caller's address.
#[cfg(target_arch = "wasm32")]
pub fn caller() -> [u8; 20] {
    let mut addr = [0u8; 20];
    unsafe {
        host_caller(addr.as_mut_ptr() as i32);
    }
    addr
}

/// Get the contract's own address.
#[cfg(target_arch = "wasm32")]
pub fn self_address() -> [u8; 20] {
    let mut addr = [0u8; 20];
    unsafe {
        host_self_address(addr.as_mut_ptr() as i32);
    }
    addr
}

/// Get the current block number.
#[cfg(target_arch = "wasm32")]
pub fn block_number() -> u64 {
    unsafe { host_block_number() as u64 }
}

/// Get the current block timestamp.
#[cfg(target_arch = "wasm32")]
pub fn block_timestamp() -> u64 {
    unsafe { host_block_timestamp() as u64 }
}

/// Get the chain ID.
#[cfg(target_arch = "wasm32")]
pub fn chain_id() -> u64 {
    unsafe { host_chain_id() as u64 }
}

/// Get the contract's own balance.
#[cfg(target_arch = "wasm32")]
pub fn self_balance() -> u64 {
    unsafe { host_self_balance() as u64 }
}

/// Transfer tokens to an address.
#[cfg(target_arch = "wasm32")]
pub fn transfer(to: &[u8; 20], amount: u64) -> bool {
    unsafe { host_transfer(to.as_ptr() as i32, amount as i64) == 0 }
}

/// Emit an event.
#[cfg(target_arch = "wasm32")]
pub fn emit_event(data: &[u8]) {
    unsafe {
        host_emit_event(0, 0, data.as_ptr() as i32, data.len() as i32);
    }
}

/// Abort contract execution with a message.
#[cfg(target_arch = "wasm32")]
pub fn abort(msg: &str) -> ! {
    unsafe {
        host_abort(msg.as_ptr() as i32, msg.len() as i32);
    }
    unreachable!()
}

// Stub implementations for non-wasm targets (testing)
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_read(_key: &[u8]) -> Option<Vec<u8>> { None }
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_write(_key: &[u8], _value: &[u8]) {}
#[cfg(not(target_arch = "wasm32"))]
pub fn caller() -> [u8; 20] { [0u8; 20] }
#[cfg(not(target_arch = "wasm32"))]
pub fn self_address() -> [u8; 20] { [0u8; 20] }
#[cfg(not(target_arch = "wasm32"))]
pub fn block_number() -> u64 { 0 }
#[cfg(not(target_arch = "wasm32"))]
pub fn block_timestamp() -> u64 { 0 }
#[cfg(not(target_arch = "wasm32"))]
pub fn chain_id() -> u64 { 0 }
#[cfg(not(target_arch = "wasm32"))]
pub fn self_balance() -> u64 { 0 }
#[cfg(not(target_arch = "wasm32"))]
pub fn transfer(_to: &[u8; 20], _amount: u64) -> bool { true }
#[cfg(not(target_arch = "wasm32"))]
pub fn emit_event(_data: &[u8]) {}
