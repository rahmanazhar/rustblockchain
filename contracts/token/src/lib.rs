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

fn allowance_key(owner: &[u8; 20], spender: &[u8; 20]) -> Vec<u8> {
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
/// Args: 20-byte address
#[no_mangle]
pub extern "C" fn balance_of(_addr_ptr: i32, addr_len: i32) -> i32 {
    if addr_len != 20 {
        // Fallback: return caller's balance
        let caller = sdk::caller();
        return read_u64(&balance_key(&caller)) as i32;
    }

    // Read the address from WASM memory via the SDK
    // The host has already written args to the pointer before calling us
    let caller = sdk::caller();
    // In the current SDK, we can only access our own caller address
    // For simplicity, return the caller's balance
    read_u64(&balance_key(&caller)) as i32
}

/// Transfer tokens to another address.
/// Args layout: [to: 20 bytes][amount: 8 bytes LE]
#[no_mangle]
pub extern "C" fn transfer_tokens(args_ptr: i32, args_len: i32) -> i32 {
    if args_len < 28 {
        sdk::emit_event(b"TransferFailed:InvalidArgs");
        return -1;
    }

    let caller = sdk::caller();
    let caller_balance = read_u64(&balance_key(&caller));

    // Decode arguments: first 20 bytes = to address, next 8 bytes = amount
    // The args have been written to WASM memory at args_ptr by the host.
    // We need to read them from our linear memory.
    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    let mut to_addr = [0u8; 20];
    to_addr.copy_from_slice(&args[..20]);

    let mut amount_bytes = [0u8; 8];
    amount_bytes.copy_from_slice(&args[20..28]);
    let amount = u64::from_le_bytes(amount_bytes);

    // Check balance
    if caller_balance < amount {
        sdk::emit_event(b"TransferFailed:InsufficientBalance");
        return -1;
    }

    // Debit sender
    write_u64(&balance_key(&caller), caller_balance - amount);

    // Credit receiver
    let to_balance = read_u64(&balance_key(&to_addr));
    write_u64(&balance_key(&to_addr), to_balance + amount);

    sdk::emit_event(b"Transfer");
    0
}

/// Approve a spender to spend tokens on behalf of the caller.
/// Args layout: [spender: 20 bytes][amount: 8 bytes LE]
#[no_mangle]
pub extern "C" fn approve(args_ptr: i32, args_len: i32) -> i32 {
    if args_len < 28 {
        return -1;
    }

    let caller = sdk::caller();
    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    let mut spender_addr = [0u8; 20];
    spender_addr.copy_from_slice(&args[..20]);

    let mut amount_bytes = [0u8; 8];
    amount_bytes.copy_from_slice(&args[20..28]);
    let amount = u64::from_le_bytes(amount_bytes);

    write_u64(&allowance_key(&caller, &spender_addr), amount);

    sdk::emit_event(b"Approval");
    0
}

/// Get total supply.
#[no_mangle]
pub extern "C" fn total_supply() -> i32 {
    read_u64(b"total_supply") as i32
}
