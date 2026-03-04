# Cross-Chain Bridge

RustChain's cross-chain bridge enables trustless interoperability with **Bitcoin**, **Ethereum**, **BNB Chain**, **Polygon**, and **Solana**. It supports two complementary mechanisms: HTLC atomic swaps for peer-to-peer trading and a lock-and-mint bridge for wrapped token transfers.

---

## Table of Contents

- [Architecture](#architecture)
- [Supported Chains](#supported-chains)
- [HTLC Atomic Swaps](#htlc-atomic-swaps)
- [Lock-and-Mint Bridge](#lock-and-mint-bridge)
- [Chain Adapters](#chain-adapters)
- [Bridge Validators](#bridge-validators)
- [Fee System](#fee-system)
- [Bridge Contract](#bridge-contract)
- [API Endpoints](#api-endpoints)
- [Security Model](#security-model)
- [Error Reference](#error-reference)

---

## Architecture

```
                    External Chains
          ┌─────────┬─────────┬─────────┬─────────┐
          │ Bitcoin │Ethereum │BNB Chain│ Polygon │ Solana  │
          └────┬────┴────┬────┴────┬────┴────┬────┴────┬───┘
               │         │        │         │         │
          ┌────┴─────────┴────────┴─────────┴─────────┴───┐
          │              Chain Adapters                     │
          │  (poll events, submit unlocks, verify proofs)  │
          └──────────────────────┬─────────────────────────┘
                                 │
          ┌──────────────────────┴─────────────────────────┐
          │                  Relayer                        │
          │  (coordinates transfers, calculates fees)       │
          └──────────────────────┬─────────────────────────┘
                                 │
     ┌───────────────┬───────────┴───────────┬─────────────┐
     │  Bridge State │   HTLC Manager        │  Validator  │
     │  (transfers,  │   (atomic swaps,      │  Set        │
     │   liquidity,  │    hash locks,        │  (threshold │
     │   wrapped     │    timelocks)          │   signing)  │
     │   tokens)     │                       │             │
     └───────────────┴───────────────────────┴─────────────┘
                                 │
          ┌──────────────────────┴─────────────────────────┐
          │              Bridge API (14 endpoints)          │
          │           /bridge/* via Axum REST               │
          └────────────────────────────────────────────────┘
```

### Crate Structure

```
crates/bridge/src/
├── lib.rs          # Module exports and re-exports
├── error.rs        # BridgeError enum (21 variants)
├── types.rs        # ExternalChain, BridgeTransfer, HtlcStatus, BridgeConfig
├── htlc.rs         # HtlcManager — SHA-256 hash-locked atomic swaps
├── state.rs        # BridgeState — transfer lifecycle, liquidity, wrapped tokens
├── validator.rs    # BridgeValidatorSet — multi-sig threshold verification
├── relay.rs        # BridgeRelayer — orchestrates cross-chain transfers
├── registry.rs     # ChainRegistry — supported tokens per chain
└── chains/
    ├── mod.rs      # ChainAdapter trait definition
    ├── ethereum.rs # EvmAdapter (Ethereum, BNB Chain, Polygon)
    ├── bitcoin.rs  # BitcoinAdapter (HTLC scripts)
    └── solana.rs   # SolanaAdapter (program-based)
```

---

## Supported Chains

| Chain | Type | Chain ID | Confirmations | Native Token | Bridgeable Tokens |
|-------|------|----------|---------------|-------------|-------------------|
| Bitcoin | UTXO | 0 | 6 blocks (~60 min) | BTC | BTC |
| Ethereum | EVM | 1 | 12 blocks (~3 min) | ETH | ETH, USDT, USDC |
| BNB Chain | EVM | 56 | 15 blocks (~45 sec) | BNB | BNB, BUSD |
| Polygon | EVM | 137 | 128 blocks (~4 min) | MATIC | MATIC |
| Solana | Program | 501 | 32 slots (~13 sec) | SOL | SOL |

### Chain Name Aliases

When specifying chains in API requests, the following aliases are accepted (case-insensitive):

| Chain | Accepted Names |
|-------|---------------|
| Bitcoin | `bitcoin`, `btc` |
| Ethereum | `ethereum`, `eth` |
| BNB Chain | `bnb`, `bnbchain`, `bsc`, `binance` |
| Polygon | `polygon`, `matic` |
| Solana | `solana`, `sol` |

---

## HTLC Atomic Swaps

Hash Time-Locked Contracts (HTLCs) enable trustless peer-to-peer cross-chain swaps without a centralized intermediary.

### How It Works

```
Alice (RustChain) wants to trade 1000 RCT for 0.5 ETH with Bob (Ethereum)

1. Alice generates a random secret and computes hash_lock = SHA-256(secret)
2. Alice creates HTLC on RustChain: locks 1000 RCT with hash_lock, 1-hour timelock
3. Bob sees Alice's HTLC, creates matching HTLC on Ethereum: locks 0.5 ETH with same hash_lock
4. Alice claims Bob's ETH on Ethereum by revealing the secret
5. Bob sees the revealed secret, claims Alice's RCT on RustChain
6. If either party fails to act, timelocks expire and funds are refunded
```

### HTLC Lifecycle

```
Created (Active)
    │
    ├──→ Claimed (recipient reveals preimage)
    │       └── Tokens transferred to recipient
    │           Secret revealed on-chain (enables counterparty claim)
    │
    └──→ Expired (timelock passed)
            └── Refunded (sender reclaims tokens)
```

### HTLC States

| State | Description |
|-------|-------------|
| `Active` | HTLC created, waiting for claim or expiry |
| `Claimed` | Preimage revealed, tokens transferred to recipient |
| `Refunded` | Expired HTLC, tokens returned to sender |
| `Expired` | Past timelock, eligible for refund (not yet refunded) |

### Creating an HTLC

```bash
# 1. Generate a secret (any 32 bytes)
SECRET=$(openssl rand -hex 32)
echo "Secret: $SECRET"

# 2. Compute SHA-256 hash lock
HASH_LOCK=$(echo -n "$SECRET" | xxd -r -p | sha256sum | cut -d' ' -f1)
echo "Hash lock: $HASH_LOCK"

# 3. Create the HTLC on RustChain
curl -X POST http://localhost:8080/bridge/htlc/create \
  -H "Content-Type: application/json" \
  -d "{
    \"sender\": \"0x<your-address>\",
    \"recipient\": \"0x<counterparty-rustchain-address>\",
    \"amount\": \"1000000\",
    \"hash_lock\": \"0x$HASH_LOCK\",
    \"external_chain\": \"Ethereum\",
    \"external_address\": \"0x<counterparty-eth-address>\",
    \"external_amount\": \"500000000000000000\"
  }"
```

### Claiming an HTLC

```bash
# Claim with the preimage (secret)
curl -X POST http://localhost:8080/bridge/htlc/claim \
  -H "Content-Type: application/json" \
  -d "{
    \"swap_id\": \"<htlc-id>\",
    \"preimage\": \"0x$SECRET\",
    \"claimer\": \"0x<recipient-address>\"
  }"
```

### Refunding an Expired HTLC

```bash
# Only works after the timelock has expired
curl -X POST http://localhost:8080/bridge/htlc/refund \
  -H "Content-Type: application/json" \
  -d "{
    \"swap_id\": \"<htlc-id>\",
    \"refunder\": \"0x<sender-address>\"
  }"
```

### Default Configuration

- **Default timelock**: 3600 seconds (1 hour)
- **Hash algorithm**: SHA-256
- **Hash lock size**: 32 bytes (64 hex characters)

---

## Lock-and-Mint Bridge

The lock-and-mint bridge enables asset transfers between RustChain and external chains using a multi-validator security model.

### Outbound Transfer (RustChain → External Chain)

```
1. User calls POST /bridge/transfer with destination chain and recipient
2. Bridge locks/burns tokens on RustChain, creates transfer record (Pending)
3. Bridge validators independently verify the lock transaction
4. Each validator submits a confirmation (POST /bridge/transfer/confirm)
5. Once threshold confirmations reached, transfer status → Validated
6. Relayer triggers unlock on the external chain via the chain adapter
7. Transfer status → Completed with destination tx hash
```

### Inbound Transfer (External Chain → RustChain)

```
1. User locks tokens in the bridge contract on the external chain
2. Chain adapter detects the lock event via poll_lock_events()
3. Relayer calls process_inbound_event(), status → SourceConfirmed
4. Bridge validators verify the source chain transaction
5. Validators submit confirmations until threshold reached → Validated
6. Relayer mints wrapped tokens on RustChain → Completed
```

### Transfer Status Lifecycle

```
Pending → SourceConfirmed → RelaySubmitted → Validated → Completed
    │                                            │
    └──→ Failed                                  └──→ Failed
    └──→ Expired
    └──→ Refunded
```

| Status | Description |
|--------|-------------|
| `Pending` | Transfer initiated, awaiting source confirmation |
| `SourceConfirmed` | Source chain transaction confirmed |
| `RelaySubmitted` | Relayer has submitted proof to validators |
| `Validated` | Sufficient validator signatures collected |
| `Completed` | Destination chain transaction executed |
| `Failed` | Transfer failed or rejected |
| `Expired` | Transfer timed out (default 24 hours) |
| `Refunded` | Transfer was refunded to sender |

### Example: Bridge to Ethereum

```bash
# Initiate outbound transfer
curl -X POST http://localhost:8080/bridge/transfer \
  -H "Content-Type: application/json" \
  -d '{
    "sender": "0x<rustchain-address>",
    "dest_chain": "Ethereum",
    "recipient": "0x<ethereum-address>",
    "token_symbol": "ETH",
    "amount": "1000000000000000000"
  }'

# Check transfer status
curl http://localhost:8080/bridge/transfers/<transfer-id>

# Validator confirms (requires bridge validator key)
curl -X POST http://localhost:8080/bridge/transfer/confirm \
  -H "Content-Type: application/json" \
  -d '{
    "transfer_id": "<transfer-id>",
    "validator": "0x<validator-address>"
  }'
```

---

## Chain Adapters

Chain adapters are pluggable components that handle chain-specific operations. Each adapter implements the `ChainAdapter` trait:

```rust
pub trait ChainAdapter: Send + Sync {
    fn chain_name(&self) -> &str;
    fn rpc_url(&self) -> &str;
    fn is_connected(&self) -> bool;
    fn current_block_number(&self) -> Result<u64, BridgeError>;
    fn required_confirmations(&self) -> u64;
    fn poll_lock_events(&self, from_block: u64, to_block: u64) -> Result<Vec<ExternalLockEvent>, BridgeError>;
    fn submit_unlock(&self, recipient: &str, token_symbol: &str, amount: u128) -> Result<String, BridgeError>;
    fn verify_tx_proof(&self, tx_hash: &str, expected_amount: u128) -> Result<bool, BridgeError>;
    fn bridge_contract_address(&self) -> &str;
    fn estimate_unlock_gas(&self) -> Result<u128, BridgeError>;
}
```

### EVM Adapter (Ethereum, BNB Chain, Polygon)

A shared `EvmAdapter` handles all EVM-compatible chains. It differentiates by chain ID and confirmation requirements:

- **Ethereum**: 12 confirmations, bridge contract at `0xBridge...`
- **BNB Chain**: 15 confirmations, bridge contract at `0xBridge...`
- **Polygon**: 128 confirmations, bridge contract at `0xBridge...`

In production, the adapter uses `eth_getLogs` to poll for lock events and submits EVM transactions for unlocks.

### Bitcoin Adapter

The Bitcoin adapter uses HTLC scripts for bridging:

- **6 confirmations** required
- Generates P2SH HTLC scripts with `OP_IF/OP_ELSE` branches for claim/refund
- Simulates `sendrawtransaction` for unlock operations
- Estimated fee: ~0.0001 BTC per transaction

### Solana Adapter

The Solana adapter interacts with a bridge program account:

- **32 slot confirmations** (~12.8 seconds to finality)
- Uses `getSignaturesForAddress` + `getTransaction` for event polling
- Bridge program ID format: base58-encoded Ed25519 pubkey
- Estimated fee: ~5000 lamports (0.000005 SOL)

---

## Bridge Validators

Bridge operations require multi-validator threshold signing for security. The `BridgeValidatorSet` manages:

- **Adding/removing validators**: Each validator has an address, public key, and stake
- **Threshold verification**: Configurable quorum (default: 2 validators)
- **Confirmation tracking**: Records per-validator confirmation counts

### Validator Operations

```bash
# List bridge validators
curl http://localhost:8080/bridge/validators
```

Validators are separate from consensus validators. They are specifically authorized to confirm bridge transfers.

---

## Fee System

Bridge transfers incur fees calculated per-chain:

| Chain | Fee Rate | Minimum Fee | Estimated Gas |
|-------|----------|-------------|---------------|
| Bitcoin | 10 bps (0.1%) | 1 sat | ~0.0001 BTC |
| Ethereum | 10 bps (0.1%) | 1 wei | ~0.005 ETH |
| BNB Chain | 10 bps (0.1%) | 1 wei | ~0.001 BNB |
| Polygon | 10 bps (0.1%) | 1 wei | ~0.01 MATIC |
| Solana | 10 bps (0.1%) | 1 lamport | ~0.000005 SOL |

Fee formula: `fee = max(amount * 10 / 10000, 1)`

```bash
# View current fee schedule
curl http://localhost:8080/bridge/fees

# View bridge liquidity
curl http://localhost:8080/bridge/liquidity
```

---

## Bridge Contract

The bridge WASM smart contract (`contracts/bridge/`) provides on-chain operations that can be deployed to RustChain's VM.

### Entry Points

| Function | Description |
|----------|-------------|
| `init()` | Initialize bridge, set deployer as admin |
| `create_htlc(sender, recipient, amount, hash_lock, timelock)` | Create hash-locked swap |
| `claim_htlc(swap_id, preimage)` | Claim swap by revealing SHA-256 preimage |
| `refund_htlc(swap_id)` | Refund expired swap to sender |
| `bridge_lock(token, amount, dest_chain, dest_address)` | Lock tokens for outbound transfer |
| `bridge_mint(recipient, token, amount)` | Mint wrapped tokens (validator only) |
| `bridge_burn(token, amount)` | Burn wrapped tokens for outbound release |
| `add_validator(address)` | Add bridge validator (admin only) |
| `remove_validator(address)` | Remove bridge validator (admin only) |
| `pause()` | Emergency pause all operations (admin only) |
| `unpause()` | Resume operations (admin only) |
| `htlc_count()` | Get total HTLC count |
| `validator_count()` | Get total validator count |

### Building the Contract

```bash
rustup target add wasm32-unknown-unknown
cd contracts/bridge
cargo build --target wasm32-unknown-unknown --release
```

Output: `contracts/bridge/target/wasm32-unknown-unknown/release/rustchain_bridge_contract.wasm`

### Storage Layout

The contract uses key-value storage with the following key prefixes:

| Key | Type | Description |
|-----|------|-------------|
| `admin` | `[u8; 20]` | Bridge admin address |
| `htlc_count` | `u64` | Total HTLC count |
| `validator_count` | `u64` | Total validator count |
| `paused` | `bool` | Whether bridge is paused |
| `htlc:<id>` | `[u8; 85]` | HTLC data: sender(20) + recipient(20) + amount(16) + timelock(8) + status(1) |
| `htlc_hash:<id>` | `[u8; 32]` | Hash lock for HTLC |
| `validator:<n>` | `[u8; 20]` | Validator address at index n |
| `locked:<token>` | `u128` | Total locked amount per token |
| `minted:<token>` | `u128` | Total minted wrapped amount per token |

---

## API Endpoints

All bridge endpoints are nested under `/bridge/`. Responses follow the standard format: `{"success": true, "data": {...}}`.

### Read Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/bridge/chains` | List all supported chains with their tokens |
| GET | `/bridge/stats` | Bridge statistics (transfers, HTLCs, chains, validators, liquidity) |
| GET | `/bridge/fees` | Fee schedule per chain |
| GET | `/bridge/liquidity` | Available bridge liquidity per chain/token |
| GET | `/bridge/transfers` | List bridge transfers (max 100, newest first) |
| GET | `/bridge/transfers/:id` | Get transfer by ID |
| GET | `/bridge/htlc` | List HTLC swaps (max 100, newest first) |
| GET | `/bridge/htlc/:id` | Get HTLC swap by ID |
| GET | `/bridge/validators` | List bridge validators |

### Write Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/bridge/transfer` | Initiate cross-chain transfer |
| POST | `/bridge/transfer/confirm` | Validator confirms a transfer |
| POST | `/bridge/htlc/create` | Create HTLC atomic swap |
| POST | `/bridge/htlc/claim` | Claim HTLC with preimage |
| POST | `/bridge/htlc/refund` | Refund expired HTLC |

### Request/Response Examples

#### Create HTLC

**Request:**
```json
{
  "sender": "0x0101010101010101010101010101010101010101",
  "recipient": "0x0202020202020202020202020202020202020202",
  "amount": "1000000",
  "hash_lock": "0x9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
  "external_chain": "Ethereum",
  "external_address": "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",
  "external_amount": "500000000000000000",
  "timelock": 1709600000
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
    "hash_lock": [159, 134, 208, ...],
    "preimage": null,
    "sender": "0x0101010101010101010101010101010101010101",
    "recipient": "0x0202020202020202020202020202020202020202",
    "amount": 1000000,
    "external_chain": "Ethereum",
    "external_address": "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",
    "external_amount": "500000000000000000",
    "timelock": 1709600000,
    "status": "Active",
    "created_at": 1709596400,
    "settled_at": null
  }
}
```

#### Initiate Transfer

**Request:**
```json
{
  "sender": "0x0101010101010101010101010101010101010101",
  "dest_chain": "Ethereum",
  "recipient": "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",
  "token_symbol": "ETH",
  "amount": "1000000000000000000"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "bridge-out-<hash>",
    "direction": "Outbound",
    "source_chain": "RustChain",
    "dest_chain": "Ethereum",
    "sender": "0x0101010101010101010101010101010101010101",
    "recipient": "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",
    "token_symbol": "ETH",
    "amount": 1000000000000000000,
    "fee": 1000000000000000,
    "status": "Pending",
    "confirmations": 0,
    "required_confirmations": 2
  }
}
```

---

## Security Model

### Multi-Validator Threshold

- Bridge transfers require a configurable number of independent validator confirmations (default: 2)
- Each validator independently verifies the source chain transaction before signing
- Non-validators cannot submit confirmations (enforced by the validator set)

### HTLC Security

- **Hash locks**: SHA-256 ensures the preimage cannot be guessed or brute-forced
- **Timelocks**: Funds are automatically refundable after expiry, preventing indefinite locking
- **Access control**: Only the designated recipient can claim; only the sender can refund
- **Atomicity**: Either both sides complete (preimage revealed on-chain) or neither does

### Bridge Contract Safety

- **Admin controls**: Only the deployer can add/remove validators or pause the bridge
- **Pause mechanism**: Emergency pause halts all bridge operations
- **On-chain verification**: HTLC preimages are verified via SHA-256 in the WASM contract

### Transfer Protections

- **Confirmation requirements**: Chain-specific confirmation counts (6 for BTC, 12 for ETH, etc.)
- **Fee validation**: Transfers must be greater than the fee amount
- **Amount validation**: Zero-amount transfers are rejected
- **Duplicate prevention**: HTLC IDs derived from hash locks prevent duplicate swaps
- **Expiry**: Transfers expire after 24 hours if not completed

---

## Error Reference

| Error | Description |
|-------|-------------|
| `UnsupportedChain` | Chain name not recognized |
| `InvalidAmount` | Zero amount, below minimum, or less than fee |
| `HtlcNotFound` | HTLC swap ID does not exist |
| `HtlcAlreadyExists` | Duplicate hash lock |
| `HtlcExpired` | HTLC past its timelock |
| `HtlcNotExpired` | Attempted refund before timelock |
| `HtlcAlreadyClaimed` | HTLC already claimed |
| `HtlcAlreadyRefunded` | HTLC already refunded |
| `InvalidPreimage` | SHA-256(preimage) does not match hash lock |
| `Unauthorized` | Caller not authorized (wrong recipient, non-validator, etc.) |
| `TransferNotFound` | Bridge transfer ID does not exist |
| `TransferAlreadyProcessed` | Transfer already completed or failed |
| `InsufficientLiquidity` | Not enough bridge liquidity for the transfer |
| `InsufficientSignatures` | Not enough validator confirmations |
| `InvalidSignature` | Validator signature verification failed |
| `AdapterError` | Chain adapter communication failure |
| `BridgePaused` | Bridge operations are paused |
| `BelowMinimum` | Amount below minimum for the token |
| `AboveMaximum` | Amount above maximum for the token |
| `Storage` | Bridge storage error |
| `Serialization` | Data serialization/deserialization error |
