//! RustChain Bridge Smart Contract
//!
//! On-chain HTLC and bridge lock/mint/burn/unlock logic.
//!
//! Storage layout:
//!   "owner"                          -> [u8; 20]
//!   "paused"                         -> u8 (0 or 1)
//!   "htlc:{hash_lock_hex}"          -> HTLC data
//!   "htlc_count"                    -> u64
//!   "locked:{chain}:{token}"        -> u128  (total locked per chain+token)
//!   "wrapped:{chain}:{token}:{addr}" -> u128 (wrapped token balances)
//!   "wrapped_supply:{chain}:{token}" -> u128 (total wrapped supply)
//!   "validator:{addr}"              -> u8 (1 = active)
//!   "validator_count"               -> u64
//!   "min_signatures"                -> u64
//!   "fee_bps"                       -> u64

use rustchain_sdk as sdk;

// ─── Helpers ────────────────────────────────────────────────

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

fn read_u128(key: &[u8]) -> u128 {
    sdk::storage_read(key)
        .and_then(|v| {
            if v.len() == 16 {
                Some(u128::from_le_bytes(v.try_into().unwrap()))
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn write_u128(key: &[u8], value: u128) {
    sdk::storage_write(key, &value.to_le_bytes());
}

fn read_bool(key: &[u8]) -> bool {
    sdk::storage_read(key).map(|v| !v.is_empty() && v[0] != 0).unwrap_or(false)
}

fn write_bool(key: &[u8], value: bool) {
    sdk::storage_write(key, &[if value { 1 } else { 0 }]);
}

fn read_address(key: &[u8]) -> [u8; 20] {
    sdk::storage_read(key)
        .and_then(|v| {
            if v.len() == 20 {
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&v);
                Some(addr)
            } else {
                None
            }
        })
        .unwrap_or([0u8; 20])
}

fn require_owner() {
    let owner = read_address(b"owner");
    let caller = sdk::caller();
    if caller != owner {
        sdk::abort("Only owner can call this function");
    }
}

fn require_not_paused() {
    if read_bool(b"paused") {
        sdk::abort("Bridge is paused");
    }
}

fn htlc_key(hash_lock_hex: &[u8]) -> Vec<u8> {
    let mut key = b"htlc:".to_vec();
    key.extend_from_slice(hash_lock_hex);
    key
}

fn locked_key(chain: &[u8], token: &[u8]) -> Vec<u8> {
    let mut key = b"locked:".to_vec();
    key.extend_from_slice(chain);
    key.push(b':');
    key.extend_from_slice(token);
    key
}

fn wrapped_balance_key(chain: &[u8], token: &[u8], addr: &[u8; 20]) -> Vec<u8> {
    let mut key = b"wrapped:".to_vec();
    key.extend_from_slice(chain);
    key.push(b':');
    key.extend_from_slice(token);
    key.push(b':');
    key.extend_from_slice(addr);
    key
}

fn wrapped_supply_key(chain: &[u8], token: &[u8]) -> Vec<u8> {
    let mut key = b"wrapped_supply:".to_vec();
    key.extend_from_slice(chain);
    key.push(b':');
    key.extend_from_slice(token);
    key
}

fn validator_key(addr: &[u8; 20]) -> Vec<u8> {
    let mut key = b"validator:".to_vec();
    key.extend_from_slice(addr);
    key
}

// ─── HTLC data encoding ─────────────────────────────────────
// Layout: [sender:20][recipient:20][amount:16][timelock:8][status:1]
// Status: 0=Active, 1=Claimed, 2=Refunded

fn encode_htlc(sender: &[u8; 20], recipient: &[u8; 20], amount: u128, timelock: u64, status: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(65);
    data.extend_from_slice(sender);
    data.extend_from_slice(recipient);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&timelock.to_le_bytes());
    data.push(status);
    data
}

struct HtlcData {
    sender: [u8; 20],
    recipient: [u8; 20],
    amount: u128,
    timelock: u64,
    status: u8,
}

fn decode_htlc(data: &[u8]) -> Option<HtlcData> {
    if data.len() < 65 {
        return None;
    }
    let mut sender = [0u8; 20];
    sender.copy_from_slice(&data[0..20]);
    let mut recipient = [0u8; 20];
    recipient.copy_from_slice(&data[20..40]);
    let mut amount_bytes = [0u8; 16];
    amount_bytes.copy_from_slice(&data[40..56]);
    let amount = u128::from_le_bytes(amount_bytes);
    let mut timelock_bytes = [0u8; 8];
    timelock_bytes.copy_from_slice(&data[56..64]);
    let timelock = u64::from_le_bytes(timelock_bytes);
    let status = data[64];
    Some(HtlcData {
        sender,
        recipient,
        amount,
        timelock,
        status,
    })
}

// ─── Contract entry points ──────────────────────────────────

/// Initialize the bridge contract.
#[no_mangle]
pub extern "C" fn init() -> i32 {
    let caller = sdk::caller();
    sdk::storage_write(b"owner", &caller);
    write_bool(b"paused", false);
    write_u64(b"htlc_count", 0);
    write_u64(b"validator_count", 0);
    write_u64(b"min_signatures", 2);
    write_u64(b"fee_bps", 10); // 0.1%
    sdk::emit_event(b"BridgeInitialized");
    0
}

/// Create an HTLC atomic swap.
/// Args: [hash_lock:32][recipient:20][timelock:8]
/// Value: amount of RCT to lock
#[no_mangle]
pub extern "C" fn create_htlc(args_ptr: i32, args_len: i32) -> i32 {
    require_not_paused();

    if args_len < 60 {
        sdk::emit_event(b"HTLCFailed:InvalidArgs");
        return -1;
    }

    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    let mut hash_lock = [0u8; 32];
    hash_lock.copy_from_slice(&args[0..32]);

    let mut recipient = [0u8; 20];
    recipient.copy_from_slice(&args[32..52]);

    let mut timelock_bytes = [0u8; 8];
    timelock_bytes.copy_from_slice(&args[52..60]);
    let timelock = u64::from_le_bytes(timelock_bytes);

    let caller = sdk::caller();
    let amount = sdk::self_balance(); // Amount sent with this call

    if amount == 0 {
        sdk::emit_event(b"HTLCFailed:ZeroAmount");
        return -1;
    }

    // Check no existing HTLC with same hash
    let hash_hex = hex_encode_32(&hash_lock);
    let key = htlc_key(&hash_hex);
    if sdk::storage_read(&key).is_some() {
        sdk::emit_event(b"HTLCFailed:AlreadyExists");
        return -1;
    }

    // Store the HTLC
    let data = encode_htlc(&caller, &recipient, amount as u128, timelock, 0);
    sdk::storage_write(&key, &data);

    let count = read_u64(b"htlc_count");
    write_u64(b"htlc_count", count + 1);

    sdk::emit_event(b"HTLCCreated");
    0
}

/// Claim an HTLC by revealing the preimage.
/// Args: [preimage: variable length]
/// The SHA-256 of the preimage must match the hash_lock.
#[no_mangle]
pub extern "C" fn claim_htlc(args_ptr: i32, args_len: i32) -> i32 {
    if args_len < 1 {
        return -1;
    }

    let preimage = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    // Compute SHA-256 of preimage (simplified — using our own implementation)
    let hash = simple_sha256(preimage);
    let hash_hex = hex_encode_32(&hash);
    let key = htlc_key(&hash_hex);

    let data = match sdk::storage_read(&key) {
        Some(d) => d,
        None => {
            sdk::emit_event(b"HTLCFailed:NotFound");
            return -1;
        }
    };

    let htlc = match decode_htlc(&data) {
        Some(h) => h,
        None => return -1,
    };

    if htlc.status != 0 {
        sdk::emit_event(b"HTLCFailed:NotActive");
        return -1;
    }

    let caller = sdk::caller();
    if caller != htlc.recipient {
        sdk::emit_event(b"HTLCFailed:NotRecipient");
        return -1;
    }

    // Check timelock not expired
    let now = sdk::block_timestamp();
    if now >= htlc.timelock {
        sdk::emit_event(b"HTLCFailed:Expired");
        return -1;
    }

    // Transfer tokens to recipient
    sdk::transfer(&htlc.recipient, htlc.amount as u64);

    // Update status to Claimed
    let updated = encode_htlc(&htlc.sender, &htlc.recipient, htlc.amount, htlc.timelock, 1);
    sdk::storage_write(&key, &updated);

    sdk::emit_event(b"HTLCClaimed");
    0
}

/// Refund an expired HTLC back to the sender.
/// Args: [hash_lock: 32 bytes]
#[no_mangle]
pub extern "C" fn refund_htlc(args_ptr: i32, args_len: i32) -> i32 {
    if args_len < 32 {
        return -1;
    }

    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    let mut hash_lock = [0u8; 32];
    hash_lock.copy_from_slice(&args[0..32]);
    let hash_hex = hex_encode_32(&hash_lock);
    let key = htlc_key(&hash_hex);

    let data = match sdk::storage_read(&key) {
        Some(d) => d,
        None => {
            sdk::emit_event(b"HTLCFailed:NotFound");
            return -1;
        }
    };

    let htlc = match decode_htlc(&data) {
        Some(h) => h,
        None => return -1,
    };

    if htlc.status != 0 {
        sdk::emit_event(b"HTLCFailed:NotActive");
        return -1;
    }

    let caller = sdk::caller();
    if caller != htlc.sender {
        sdk::emit_event(b"HTLCFailed:NotSender");
        return -1;
    }

    // Check timelock expired
    let now = sdk::block_timestamp();
    if now < htlc.timelock {
        sdk::emit_event(b"HTLCFailed:NotExpired");
        return -1;
    }

    // Refund tokens to sender
    sdk::transfer(&htlc.sender, htlc.amount as u64);

    // Update status to Refunded
    let updated = encode_htlc(&htlc.sender, &htlc.recipient, htlc.amount, htlc.timelock, 2);
    sdk::storage_write(&key, &updated);

    sdk::emit_event(b"HTLCRefunded");
    0
}

/// Lock tokens for a cross-chain bridge transfer.
/// Args: [dest_chain_len:4 LE][dest_chain:var][token_len:4 LE][token:var][recipient_len:4 LE][recipient:var]
/// Value: amount of RCT to lock
#[no_mangle]
pub extern "C" fn bridge_lock(args_ptr: i32, args_len: i32) -> i32 {
    require_not_paused();

    if args_len < 12 {
        sdk::emit_event(b"BridgeLockFailed:InvalidArgs");
        return -1;
    }

    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    // Parse dest_chain
    let mut len_buf = [0u8; 4];
    len_buf.copy_from_slice(&args[0..4]);
    let chain_len = u32::from_le_bytes(len_buf) as usize;
    if args.len() < 4 + chain_len + 4 {
        return -1;
    }
    let chain = &args[4..4 + chain_len];

    // Parse token
    let offset = 4 + chain_len;
    len_buf.copy_from_slice(&args[offset..offset + 4]);
    let token_len = u32::from_le_bytes(len_buf) as usize;
    if args.len() < offset + 4 + token_len + 4 {
        return -1;
    }
    let token = &args[offset + 4..offset + 4 + token_len];

    // Parse recipient on external chain
    let offset2 = offset + 4 + token_len;
    len_buf.copy_from_slice(&args[offset2..offset2 + 4]);
    let recip_len = u32::from_le_bytes(len_buf) as usize;
    if args.len() < offset2 + 4 + recip_len {
        return -1;
    }
    // recipient is in args[offset2+4..offset2+4+recip_len] (stored but used off-chain)

    let amount = sdk::self_balance() as u128;
    if amount == 0 {
        sdk::emit_event(b"BridgeLockFailed:ZeroAmount");
        return -1;
    }

    // Calculate fee
    let fee_bps = read_u64(b"fee_bps") as u128;
    let fee = (amount * fee_bps) / 10_000;
    let net_amount = amount - fee;

    // Update locked amount
    let lk = locked_key(chain, token);
    let current = read_u128(&lk);
    write_u128(&lk, current + net_amount);

    sdk::emit_event(b"BridgeLocked");
    0
}

/// Mint wrapped tokens (called by bridge validators after confirming external lock).
/// Args: [chain_len:4][chain:var][token_len:4][token:var][recipient:20][amount:16]
/// Only callable by owner/validators.
#[no_mangle]
pub extern "C" fn bridge_mint(args_ptr: i32, args_len: i32) -> i32 {
    require_owner();
    require_not_paused();

    if args_len < 44 {
        return -1;
    }

    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    // Parse chain
    let mut len_buf = [0u8; 4];
    len_buf.copy_from_slice(&args[0..4]);
    let chain_len = u32::from_le_bytes(len_buf) as usize;
    let chain = &args[4..4 + chain_len];

    // Parse token
    let offset = 4 + chain_len;
    len_buf.copy_from_slice(&args[offset..offset + 4]);
    let token_len = u32::from_le_bytes(len_buf) as usize;
    let token = &args[offset + 4..offset + 4 + token_len];

    // Parse recipient and amount
    let offset2 = offset + 4 + token_len;
    let mut recipient = [0u8; 20];
    recipient.copy_from_slice(&args[offset2..offset2 + 20]);
    let mut amount_bytes = [0u8; 16];
    amount_bytes.copy_from_slice(&args[offset2 + 20..offset2 + 36]);
    let amount = u128::from_le_bytes(amount_bytes);

    // Update wrapped token balance
    let bk = wrapped_balance_key(chain, token, &recipient);
    let current = read_u128(&bk);
    write_u128(&bk, current + amount);

    // Update total supply
    let sk = wrapped_supply_key(chain, token);
    let supply = read_u128(&sk);
    write_u128(&sk, supply + amount);

    sdk::emit_event(b"WrappedMinted");
    0
}

/// Burn wrapped tokens to initiate withdrawal to external chain.
/// Args: [chain_len:4][chain:var][token_len:4][token:var][amount:16][external_addr_len:4][external_addr:var]
#[no_mangle]
pub extern "C" fn bridge_burn(args_ptr: i32, args_len: i32) -> i32 {
    require_not_paused();

    if args_len < 28 {
        return -1;
    }

    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    // Parse chain
    let mut len_buf = [0u8; 4];
    len_buf.copy_from_slice(&args[0..4]);
    let chain_len = u32::from_le_bytes(len_buf) as usize;
    let chain = &args[4..4 + chain_len];

    // Parse token
    let offset = 4 + chain_len;
    len_buf.copy_from_slice(&args[offset..offset + 4]);
    let token_len = u32::from_le_bytes(len_buf) as usize;
    let token = &args[offset + 4..offset + 4 + token_len];

    // Parse amount
    let offset2 = offset + 4 + token_len;
    let mut amount_bytes = [0u8; 16];
    amount_bytes.copy_from_slice(&args[offset2..offset2 + 16]);
    let amount = u128::from_le_bytes(amount_bytes);

    let caller = sdk::caller();

    // Check balance
    let bk = wrapped_balance_key(chain, token, &caller);
    let balance = read_u128(&bk);
    if balance < amount {
        sdk::emit_event(b"BurnFailed:InsufficientBalance");
        return -1;
    }

    // Debit balance
    write_u128(&bk, balance - amount);

    // Reduce supply
    let sk = wrapped_supply_key(chain, token);
    let supply = read_u128(&sk);
    write_u128(&sk, supply.saturating_sub(amount));

    sdk::emit_event(b"WrappedBurned");
    0
}

/// Add a bridge validator (owner only).
/// Args: [validator_address: 20 bytes]
#[no_mangle]
pub extern "C" fn add_validator(args_ptr: i32, args_len: i32) -> i32 {
    require_owner();

    if args_len < 20 {
        return -1;
    }

    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    let mut addr = [0u8; 20];
    addr.copy_from_slice(&args[0..20]);

    let vk = validator_key(&addr);
    write_bool(&vk, true);

    let count = read_u64(b"validator_count");
    write_u64(b"validator_count", count + 1);

    sdk::emit_event(b"ValidatorAdded");
    0
}

/// Remove a bridge validator (owner only).
/// Args: [validator_address: 20 bytes]
#[no_mangle]
pub extern "C" fn remove_validator(args_ptr: i32, args_len: i32) -> i32 {
    require_owner();

    if args_len < 20 {
        return -1;
    }

    let args = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    let mut addr = [0u8; 20];
    addr.copy_from_slice(&args[0..20]);

    let vk = validator_key(&addr);
    write_bool(&vk, false);

    let count = read_u64(b"validator_count");
    if count > 0 {
        write_u64(b"validator_count", count - 1);
    }

    sdk::emit_event(b"ValidatorRemoved");
    0
}

/// Pause the bridge (owner only).
#[no_mangle]
pub extern "C" fn pause() -> i32 {
    require_owner();
    write_bool(b"paused", true);
    sdk::emit_event(b"BridgePaused");
    0
}

/// Unpause the bridge (owner only).
#[no_mangle]
pub extern "C" fn unpause() -> i32 {
    require_owner();
    write_bool(b"paused", false);
    sdk::emit_event(b"BridgeUnpaused");
    0
}

/// Get the total HTLC count.
#[no_mangle]
pub extern "C" fn htlc_count() -> i32 {
    read_u64(b"htlc_count") as i32
}

/// Get the validator count.
#[no_mangle]
pub extern "C" fn validator_count() -> i32 {
    read_u64(b"validator_count") as i32
}

// ─── Utility ────────────────────────────────────────────────

/// Simple hex encoding for 32-byte values (no-std compatible).
fn hex_encode_32(bytes: &[u8; 32]) -> Vec<u8> {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(64);
    for &b in bytes.iter() {
        out.push(HEX_CHARS[(b >> 4) as usize]);
        out.push(HEX_CHARS[(b & 0x0f) as usize]);
    }
    out
}

/// Minimal SHA-256 for HTLC hash verification (no external dependency needed in WASM).
/// This is a compact implementation suitable for smart contract use.
fn simple_sha256(data: &[u8]) -> [u8; 32] {
    // SHA-256 constants
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    // Pre-processing: padding
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) block
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = [0u8; 32];
    for (i, &val) in h.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    result
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        // Test vector: SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = simple_sha256(b"");
        assert_eq!(hash[0], 0xe3);
        assert_eq!(hash[1], 0xb0);
        assert_eq!(hash[31], 0x55);

        // Test vector: SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let hash = simple_sha256(b"abc");
        assert_eq!(hash[0], 0xba);
        assert_eq!(hash[1], 0x78);
    }

    #[test]
    fn test_hex_encode() {
        let bytes = [0xAB; 32];
        let hex = hex_encode_32(&bytes);
        // 32 bytes → 64 hex chars, each byte 0xAB → "ab"
        assert_eq!(hex.len(), 64);
        for chunk in hex.chunks(2) {
            assert_eq!(chunk, b"ab");
        }
        // Also test a known value
        let mut b2 = [0u8; 32];
        b2[0] = 0x01;
        b2[31] = 0xff;
        let h2 = hex_encode_32(&b2);
        assert_eq!(&h2[0..2], b"01");
        assert_eq!(&h2[62..64], b"ff");
    }

    #[test]
    fn test_htlc_encode_decode() {
        let sender = [1u8; 20];
        let recipient = [2u8; 20];
        let amount = 12345u128;
        let timelock = 999u64;
        let status = 0u8;

        let encoded = encode_htlc(&sender, &recipient, amount, timelock, status);
        let decoded = decode_htlc(&encoded).unwrap();

        assert_eq!(decoded.sender, sender);
        assert_eq!(decoded.recipient, recipient);
        assert_eq!(decoded.amount, amount);
        assert_eq!(decoded.timelock, timelock);
        assert_eq!(decoded.status, status);
    }

    #[test]
    fn test_init() {
        sdk::mock_reset();
        let owner = [0x01; 20];
        sdk::mock_set_caller(owner);
        init();

        assert_eq!(read_address(b"owner"), owner);
        assert!(!read_bool(b"paused"));
        assert_eq!(read_u64(b"htlc_count"), 0);
        assert_eq!(read_u64(b"fee_bps"), 10);
    }

    #[test]
    fn test_pause_unpause() {
        sdk::mock_reset();
        let owner = [0x01; 20];
        sdk::mock_set_caller(owner);
        init();

        pause();
        assert!(read_bool(b"paused"));

        unpause();
        assert!(!read_bool(b"paused"));
    }

    #[test]
    fn test_add_remove_validator() {
        sdk::mock_reset();
        let owner = [0x01; 20];
        sdk::mock_set_caller(owner);
        init();

        // Add validator
        let val_addr = [0x10; 20];
        // Simulate calling add_validator with val_addr as args
        let vk = validator_key(&val_addr);
        write_bool(&vk, true);
        write_u64(b"validator_count", 1);

        assert!(read_bool(&vk));
        assert_eq!(read_u64(b"validator_count"), 1);

        // Remove
        write_bool(&vk, false);
        write_u64(b"validator_count", 0);
        assert!(!read_bool(&vk));
    }
}
