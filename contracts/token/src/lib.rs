//! ERC20-like token contract for RustChain.
//!
//! Storage layout:
//!   "total_supply" -> u64
//!   "balance:{addr}" -> u64
//!   "allowance:{owner}:{spender}" -> u64
//!   "name" -> String
//!   "symbol" -> String
//!   "decimals" -> u8
//!   "owner" -> [u8; 20]

use rustchain_sdk as sdk;

fn balance_key(addr: &[u8; 20]) -> Vec<u8> {
    let mut key = b"balance:".to_vec();
    key.extend_from_slice(addr);
    key
}

fn _allowance_key(owner: &[u8; 20], spender: &[u8; 20]) -> Vec<u8> {
    let mut key = b"allowance:".to_vec();
    key.extend_from_slice(owner);
    key.push(b':');
    key.extend_from_slice(spender);
    key
}

fn read_u64(key: &[u8]) -> u64 {
    sdk::storage_read(key)
        .and_then(|v| {
            if v.len() == 8 {
                Some(u64::from_le_bytes(v.try_into().unwrap()))
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn write_u64(key: &[u8], value: u64) {
    sdk::storage_write(key, &value.to_le_bytes());
}

/// Initialize the token contract.
#[no_mangle]
pub extern "C" fn init() -> i32 {
    let caller = sdk::caller();

    sdk::storage_write(b"name", b"RustChain Token");
    sdk::storage_write(b"symbol", b"RCT");
    sdk::storage_write(b"decimals", &[18u8]);
    sdk::storage_write(b"owner", &caller);

    let initial_supply: u64 = 1_000_000_000; // 1 billion tokens
    write_u64(b"total_supply", initial_supply);
    write_u64(&balance_key(&caller), initial_supply);

    sdk::emit_event(b"TokenInitialized");
    0
}

/// Get the balance of an address.
#[no_mangle]
pub extern "C" fn balance_of(_addr_ptr: i32, _addr_len: i32) -> i32 {
    // In a real implementation, we'd read the address from memory
    // For now, return the caller's balance
    let caller = sdk::caller();
    read_u64(&balance_key(&caller)) as i32
}

/// Transfer tokens to another address.
#[no_mangle]
pub extern "C" fn transfer_tokens(_args_ptr: i32, _args_len: i32) -> i32 {
    let caller = sdk::caller();
    let _caller_balance = read_u64(&balance_key(&caller));

    // Simplified: would normally decode args for (to, amount)
    // This is a demonstration of the contract structure
    0
}

/// Get total supply.
#[no_mangle]
pub extern "C" fn total_supply() -> i32 {
    read_u64(b"total_supply") as i32
}
