//! RustChain Smart Contract SDK
//!
//! Provides safe Rust wrappers around the host function imports
//! for building WASM smart contracts.
//!
//! On `wasm32` targets, functions call real host imports provided by the VM.
//! On native targets (x86_64, aarch64, etc.), a thread-local mock environment
//! is used, allowing contracts to be unit-tested without a WASM runtime.

// ─── WASM target: real host imports ──────────────────────────

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

// ─── Native targets: thread-local mock environment ───────────

#[cfg(not(target_arch = "wasm32"))]
mod mock {
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Mock blockchain environment for native testing.
    pub struct MockEnv {
        pub storage: HashMap<Vec<u8>, Vec<u8>>,
        pub caller: [u8; 20],
        pub self_addr: [u8; 20],
        pub block_number: u64,
        pub block_timestamp: u64,
        pub chain_id: u64,
        pub self_balance: u64,
        pub events: Vec<Vec<u8>>,
        pub transfers: Vec<([u8; 20], u64)>,
    }

    impl Default for MockEnv {
        fn default() -> Self {
            Self {
                storage: HashMap::new(),
                caller: [0u8; 20],
                self_addr: [0u8; 20],
                block_number: 1,
                block_timestamp: 1_700_000_000,
                chain_id: 9999,
                self_balance: 0,
                events: Vec::new(),
                transfers: Vec::new(),
            }
        }
    }

    thread_local! {
        pub static ENV: RefCell<MockEnv> = RefCell::new(MockEnv::default());
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use mock::MockEnv;

/// Reset the mock environment to defaults. Call between tests.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_reset() {
    mock::ENV.with(|e| *e.borrow_mut() = MockEnv::default());
}

/// Configure the mock caller address.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_set_caller(addr: [u8; 20]) {
    mock::ENV.with(|e| e.borrow_mut().caller = addr);
}

/// Configure the mock contract address.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_set_self_address(addr: [u8; 20]) {
    mock::ENV.with(|e| e.borrow_mut().self_addr = addr);
}

/// Configure the mock block number.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_set_block_number(n: u64) {
    mock::ENV.with(|e| e.borrow_mut().block_number = n);
}

/// Configure the mock block timestamp.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_set_block_timestamp(ts: u64) {
    mock::ENV.with(|e| e.borrow_mut().block_timestamp = ts);
}

/// Configure the mock chain ID.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_set_chain_id(id: u64) {
    mock::ENV.with(|e| e.borrow_mut().chain_id = id);
}

/// Configure the mock contract balance.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_set_balance(bal: u64) {
    mock::ENV.with(|e| e.borrow_mut().self_balance = bal);
}

/// Get all events emitted during this mock session.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_get_events() -> Vec<Vec<u8>> {
    mock::ENV.with(|e| e.borrow().events.clone())
}

/// Get all transfers made during this mock session.
#[cfg(not(target_arch = "wasm32"))]
pub fn mock_get_transfers() -> Vec<([u8; 20], u64)> {
    mock::ENV.with(|e| e.borrow().transfers.clone())
}

// ─── Native API implementations ─────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub fn storage_read(key: &[u8]) -> Option<Vec<u8>> {
    mock::ENV.with(|e| e.borrow().storage.get(key).cloned())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn storage_write(key: &[u8], value: &[u8]) {
    mock::ENV.with(|e| {
        e.borrow_mut()
            .storage
            .insert(key.to_vec(), value.to_vec());
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn caller() -> [u8; 20] {
    mock::ENV.with(|e| e.borrow().caller)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn self_address() -> [u8; 20] {
    mock::ENV.with(|e| e.borrow().self_addr)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn block_number() -> u64 {
    mock::ENV.with(|e| e.borrow().block_number)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn block_timestamp() -> u64 {
    mock::ENV.with(|e| e.borrow().block_timestamp)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn chain_id() -> u64 {
    mock::ENV.with(|e| e.borrow().chain_id)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn self_balance() -> u64 {
    mock::ENV.with(|e| e.borrow().self_balance)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn transfer(to: &[u8; 20], amount: u64) -> bool {
    mock::ENV.with(|e| {
        let mut env = e.borrow_mut();
        if env.self_balance >= amount {
            env.self_balance -= amount;
            env.transfers.push((*to, amount));
            true
        } else {
            false
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn emit_event(data: &[u8]) {
    mock::ENV.with(|e| e.borrow_mut().events.push(data.to_vec()));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn abort(msg: &str) -> ! {
    panic!("contract abort: {}", msg);
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_storage_roundtrip() {
        mock_reset();
        assert_eq!(storage_read(b"key1"), None);
        storage_write(b"key1", b"hello");
        assert_eq!(storage_read(b"key1"), Some(b"hello".to_vec()));
        storage_write(b"key1", b"updated");
        assert_eq!(storage_read(b"key1"), Some(b"updated".to_vec()));
    }

    #[test]
    fn test_mock_caller() {
        mock_reset();
        assert_eq!(caller(), [0u8; 20]);
        let addr = [1u8; 20];
        mock_set_caller(addr);
        assert_eq!(caller(), addr);
    }

    #[test]
    fn test_mock_block_info() {
        mock_reset();
        mock_set_block_number(42);
        mock_set_block_timestamp(1_700_000_100);
        mock_set_chain_id(1337);
        assert_eq!(block_number(), 42);
        assert_eq!(block_timestamp(), 1_700_000_100);
        assert_eq!(chain_id(), 1337);
    }

    #[test]
    fn test_mock_transfer_success() {
        mock_reset();
        mock_set_balance(1000);
        let to = [2u8; 20];
        assert!(transfer(&to, 600));
        assert_eq!(self_balance(), 400);
        let transfers = mock_get_transfers();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0], (to, 600));
    }

    #[test]
    fn test_mock_transfer_insufficient() {
        mock_reset();
        mock_set_balance(100);
        let to = [3u8; 20];
        assert!(!transfer(&to, 200));
        assert_eq!(self_balance(), 100);
        assert!(mock_get_transfers().is_empty());
    }

    #[test]
    fn test_mock_events() {
        mock_reset();
        emit_event(b"Event1");
        emit_event(b"Event2");
        let events = mock_get_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], b"Event1");
        assert_eq!(events[1], b"Event2");
    }

    #[test]
    fn test_mock_self_address() {
        mock_reset();
        let addr = [0xAB; 20];
        mock_set_self_address(addr);
        assert_eq!(self_address(), addr);
    }

    #[test]
    #[should_panic(expected = "contract abort: test error")]
    fn test_mock_abort() {
        mock_reset();
        abort("test error");
    }
}
