# RustChain

A production-grade Proof of Stake blockchain built in Rust with WASM smart contracts, a web dashboard, enterprise security, and full deployment tooling.

**78 tests** | **11 crates** | **Zero unsafe in application code** | **Zero clippy warnings**

---

## Table of Contents

- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Web Dashboard](#web-dashboard)
- [Wallet CLI](#wallet-cli)
- [API Reference](#api-reference)
- [Smart Contracts](#smart-contracts)
- [Configuration](#configuration)
- [Deployment](#deployment)
- [Security](#security)
- [Testing](#testing)
- [License](#license)

---

## Features

### Consensus & Chain
- **Proof of Stake** - Weighted validator selection, epoch-based rotation (configurable epoch length), BFT 2/3+ finality
- **Slashing** - Double-sign detection with configurable slash fraction, downtime jailing, minimum stake enforcement
- **Staking** - Stake/unstake with unbonding periods, automatic validator activation/deactivation at epoch boundaries
- **Transaction Receipts** - Full receipt tracking with status, gas used, event logs, return data, and contract addresses

### Smart Contracts
- **WASM VM** - Wasmtime-powered execution engine with gas metering, memory limits, and call depth limits
- **Contract SDK** - Rust SDK for writing contracts with storage, events, transfers, and chain introspection
- **Cross-platform SDK** - Thread-local mock environment for native testing on x86_64/aarch64 without WASM toolchain
- **Example Contract** - ERC20-like token contract with init, transfer, approve, and balance queries

### Networking
- **libp2p** - Kademlia DHT for peer discovery, GossipSub for block/tx propagation, Noise protocol encryption
- **Peer Management** - Scoring, banning, connection limits, mDNS for local discovery, configurable bootnodes

### API & Dashboard
- **REST API** - 22 endpoints covering chain, blocks, transactions, accounts, validators, contracts, receipts, wallet operations
- **WebSocket** - Real-time subscriptions for new blocks, transactions, and finality events
- **Web Dashboard** - Built-in blockchain explorer and wallet UI served at `/` (no external dependencies)
- **Middleware** - JWT authentication, RBAC (Admin/Validator/User/ReadOnly), rate limiting, CORS, request body limits

### Security
- **Transport** - Noise protocol (P2P), TLS 1.3 (API), certificate auto-generation
- **Key Management** - Encrypted keystores (scrypt + AES-256-GCM), BIP39 mnemonics, zeroize-on-drop
- **Consensus Safety** - Double-sign detection, slashing, epoch-boundary validator set changes

### Operations
- **Monitoring** - Prometheus metrics endpoint, Grafana dashboard support
- **Deployment** - Docker, Docker Compose (3-node testnet + monitoring), Kubernetes (StatefulSet, HPA, NetworkPolicy, PDB)
- **CLI Tools** - Full-featured node binary and wallet CLI

---

## Architecture

```
rustblockchain/
├── crates/
│   ├── crypto/       # Ed25519 keys, Blake3 hashing, Merkle trees, BIP39, encrypted keystores
│   ├── core/         # Block, Transaction, Account, Validator, Genesis types
│   ├── storage/      # RocksDB persistence with column families
│   ├── vm/           # Wasmtime WASM smart contract engine with gas metering
│   ├── consensus/    # PoS engine, finality, slashing, mempool, epoch management
│   ├── network/      # libp2p P2P networking, peer scoring, block sync
│   ├── api/          # Axum REST + WebSocket server, web dashboard, wallet API
│   ├── node/         # Main node binary with CLI
│   └── wallet/       # CLI wallet for account management and transactions
├── contracts/
│   ├── sdk/          # Smart contract SDK (WASM + native mock for testing)
│   └── token/        # Example ERC20-like token contract
├── deploy/
│   ├── docker/       # Dockerfile + docker-compose (3 validators + Prometheus + Grafana)
│   ├── k8s/          # Kubernetes manifests (StatefulSet, Service, Ingress, HPA, etc.)
│   └── scripts/      # Genesis generation utilities
└── config/           # Configuration templates (node.toml)
```

### Crate Dependency Graph

```
                    ┌─────────┐
                    │  crypto  │  Ed25519, Blake3, BIP39, Keystores
                    └────┬────┘
                         │
                    ┌────┴────┐
                    │  core   │  Block, Transaction, Account, Genesis
                    └────┬────┘
                    ┌────┴────┐
              ┌─────┤ storage │  RocksDB persistence
              │     └────┬────┘
              │     ┌────┴────┐
              │     │   vm    │  Wasmtime WASM execution
              │     └────┬────┘
              │  ┌───────┴────────┐
              └──┤  consensus     │  PoS, finality, slashing, mempool
                 └───────┬────────┘
           ┌─────────────┼─────────────┐
      ┌────┴────┐   ┌────┴────┐   ┌────┴────┐
      │ network │   │   api   │   │  wallet │
      └────┬────┘   └────┬────┘   └─────────┘
           └─────┬───────┘
            ┌────┴────┐
            │  node   │  Main binary
            └─────────┘
```

---

## Quick Start

### Prerequisites

- **Rust 1.75+** (install via [rustup](https://rustup.rs))
- **LLVM/Clang** (for RocksDB compilation)
- **Git**

### Build

```bash
cargo build --release
```

### Run a Devnet Node

```bash
cargo run --bin rustchain -- run --devnet
```

This starts a single-validator devnet that produces and finalizes blocks every 5 seconds. The API server and web dashboard are available at `http://localhost:8080`.

### Generate a Validator Key

```bash
cargo run --bin rustchain -- keygen --output ./keystore/validator.key
```

### Initialize from Genesis

```bash
cargo run --bin rustchain -- init --genesis ./config/genesis.toml
```

### Run in Validator Mode

```bash
cargo run --bin rustchain -- run --validator --keyfile ./keystore/validator.key
```

### Node CLI Reference

```
rustchain [OPTIONS] <COMMAND>

Commands:
  run       Start the blockchain node
  init      Initialize chain from genesis config
  keygen    Generate a new validator keypair
  version   Show version and build info

Global Options:
  --config <PATH>      Configuration file [default: config/node.toml]
  --log-level <LEVEL>  Log level: trace|debug|info|warn|error [default: info]
  --data-dir <PATH>    Data directory [default: ./data]

Run Options:
  --validator          Enable block production
  --keyfile <PATH>     Validator keyfile path
  --devnet             Single-node devnet with auto-generated config
```

---

## Web Dashboard

The node serves a built-in web dashboard at `http://localhost:8080/` with no external dependencies.

### Explorer (Public)
- **Overview** - Live stats (height, finalized, epoch, validators, pending txs), real-time block feed via WebSocket, chain status panel
- **Blocks** - Paginated block list, click-through to block detail with transaction list
- **Transactions** - Recent transactions from blocks, click-through to tx detail with receipt
- **Validators** - Validator table with stake percentage bars, status, commission, slash count
- **Accounts** - Address lookup showing balance, nonce, and account type (EOA/Contract)
- **Contracts** - Contract lookup showing code hash, bytecode, and storage queries

### Wallet (Interactive)
- **Manage** - Create new accounts (BIP39 mnemonic), import from mnemonic or private key, list/remove saved accounts
- **Send** - Transfer tokens with configurable gas limit and gas price
- **Stake** - Stake and unstake tokens for validator operations
- **Contracts** - Deploy WASM contracts (hex paste or .wasm file upload), call contract functions

Wallet accounts are stored as encrypted keystores in browser `localStorage`. The keystore uses scrypt + AES-256-GCM, so it is safe to persist client-side. Transaction signing happens server-side — the encrypted keystore + password are sent to the API, which decrypts in memory, signs, submits, and returns the tx hash.

---

## Wallet CLI

The wallet CLI provides account management and transaction capabilities from the command line.

```bash
cargo run --bin rustchain-wallet -- [OPTIONS] <COMMAND>

Global Options:
  --node <URL>        Node API endpoint [default: http://localhost:8080]
  --keystore <PATH>   Keystore directory [default: ./keystore]
```

### Account Management

```bash
# Create a new account (generates BIP39 mnemonic)
cargo run --bin rustchain-wallet -- account create --words 24

# Import from mnemonic phrase
cargo run --bin rustchain-wallet -- account import-mnemonic --phrase "word1 word2 ... word24"

# Import from hex private key
cargo run --bin rustchain-wallet -- account import-key --key <64-hex-chars>

# List all local accounts
cargo run --bin rustchain-wallet -- account list
```

### Transactions

```bash
# Send a transfer
cargo run --bin rustchain-wallet -- transfer \
  --from <sender-hex> --to <recipient-hex> --amount 1000000

# Stake tokens
cargo run --bin rustchain-wallet -- stake \
  --from <validator-hex> --amount 10000000000000000000

# Unstake tokens
cargo run --bin rustchain-wallet -- unstake \
  --from <validator-hex> --amount 5000000000000000000
```

### Smart Contract Operations

```bash
# Deploy a WASM contract
cargo run --bin rustchain-wallet -- deploy \
  --from <deployer-hex> \
  --wasm ./contracts/token/target/wasm32-unknown-unknown/release/rustchain_token_contract.wasm \
  --gas-limit 1000000

# Call a contract function
cargo run --bin rustchain-wallet -- call \
  --from <caller-hex> --contract <contract-hex> \
  --function transfer_tokens --args <hex-args> --gas-limit 500000
```

### Queries

```bash
# Chain info
cargo run --bin rustchain-wallet -- query chain-info

# Account balance
cargo run --bin rustchain-wallet -- query balance --address <hex-address>

# Block by number or hash
cargo run --bin rustchain-wallet -- query block --id 1

# Transaction by hash
cargo run --bin rustchain-wallet -- query transaction --hash <tx-hash>
```

---

## API Reference

All endpoints return JSON: `{"success": true, "data": {...}}` on success, `{"success": false, "error": {"code": N, "message": "..."}}` on error.

### Health

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Liveness probe |
| GET | `/health/ready` | Readiness probe (storage + consensus) |

### Chain

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/chain/info` | Chain metadata: chain_id, height, epoch, finalized_height, active_validators |
| GET | `/chain/status` | Lightweight status: height, syncing, pending_transactions |

### Blocks

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/blocks?page=1&per_page=20` | Paginated block list (newest first) |
| GET | `/blocks/latest` | Latest block with transactions |
| GET | `/blocks/:id` | Block by number or hex hash |

### Transactions

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/tx/:hash` | Transaction by hash |
| POST | `/tx` | Submit a signed transaction (RBAC-gated when enabled) |

### Accounts

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/accounts/:address` | Account info: balance, nonce, code_hash |
| GET | `/accounts/:address/balance` | Balance only (returns 0 for non-existent) |

### Validators

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/validators` | All validators: stake, status, commission, jail, slash count |

### Receipts

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/receipts/:hash` | Transaction receipt: status, gas_used, logs, contract_address |

### Contracts

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/contracts/:address` | Contract info: balance, code_hash, nonce |
| GET | `/contracts/:address/storage/:key` | Read contract storage by hex key |
| GET | `/contracts/:address/code` | Contract bytecode and size |

### Wallet API

Server-side signing endpoints. The encrypted keystore + password are sent; decryption happens in memory.

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/wallet/create` | Create account: `{password, word_count?}` → address, mnemonic, keystore |
| POST | `/wallet/import/mnemonic` | Import from mnemonic: `{mnemonic, password}` → address, keystore |
| POST | `/wallet/import/private-key` | Import from key: `{private_key, password}` → address, keystore |
| POST | `/wallet/transfer` | Send transfer: `{keystore, password, to, value, gas_limit?, gas_price?}` → tx_hash |
| POST | `/wallet/stake` | Stake tokens: `{keystore, password, value}` → tx_hash |
| POST | `/wallet/unstake` | Unstake tokens: `{keystore, password, value}` → tx_hash |
| POST | `/wallet/deploy` | Deploy contract: `{keystore, password, bytecode, gas_limit?}` → tx_hash |
| POST | `/wallet/call` | Call contract: `{keystore, password, contract, function, args?, value?, gas_limit?}` → tx_hash |

### Monitoring

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/metrics` | Prometheus metrics (when enabled) |
| WS | `/ws` | WebSocket: NewBlock, NewTransaction, BlockFinalized events |

### Curl Examples

```bash
# Chain info
curl http://localhost:8080/chain/info

# List blocks
curl "http://localhost:8080/blocks?page=1&per_page=10"

# Get block by number
curl http://localhost:8080/blocks/1

# Get latest block
curl http://localhost:8080/blocks/latest

# Account balance
curl http://localhost:8080/accounts/<address>/balance

# Validators
curl http://localhost:8080/validators

# Transaction receipt
curl http://localhost:8080/receipts/<tx-hash>

# Contract info
curl http://localhost:8080/contracts/<address>

# Contract storage
curl http://localhost:8080/contracts/<address>/storage/<hex-key>

# Create wallet account
curl -X POST http://localhost:8080/wallet/create \
  -H "Content-Type: application/json" \
  -d '{"password": "mypassword", "word_count": 12}'

# Send transfer via wallet API
curl -X POST http://localhost:8080/wallet/transfer \
  -H "Content-Type: application/json" \
  -d '{
    "keystore": <keystore-json>,
    "password": "mypassword",
    "to": "0x1234...",
    "value": "1000000"
  }'

# Deploy contract
curl -X POST http://localhost:8080/wallet/deploy \
  -H "Content-Type: application/json" \
  -d '{
    "keystore": <keystore-json>,
    "password": "mypassword",
    "bytecode": "<hex-encoded-wasm>",
    "gas_limit": 1000000
  }'

# WebSocket (using websocat)
websocat ws://localhost:8080/ws
```

---

## Smart Contracts

### Writing Contracts

Contracts are written in Rust and compiled to WASM. The SDK provides host function wrappers:

```rust
use rustchain_sdk as sdk;

#[no_mangle]
pub extern "C" fn init() -> i32 {
    let caller = sdk::caller();
    sdk::storage_write(b"owner", &caller);
    sdk::storage_write(b"total", &1000u64.to_le_bytes());
    sdk::emit_event(b"Initialized");
    0 // success
}

#[no_mangle]
pub extern "C" fn get_balance(args_ptr: i32, args_len: i32) -> i32 {
    let caller = sdk::caller();
    let key = [b"bal:".as_slice(), &caller].concat();
    match sdk::storage_read(&key) {
        Some(v) => u64::from_le_bytes(v.try_into().unwrap()) as i32,
        None => 0,
    }
}
```

### SDK Functions

| Function | Description |
|----------|-------------|
| `storage_read(key) -> Option<Vec<u8>>` | Read from contract storage |
| `storage_write(key, value)` | Write to contract storage |
| `caller() -> [u8; 20]` | Get caller's address |
| `self_address() -> [u8; 20]` | Get contract's own address |
| `block_number() -> u64` | Current block number |
| `block_timestamp() -> u64` | Current block timestamp |
| `chain_id() -> u64` | Chain ID |
| `self_balance() -> u64` | Contract's balance |
| `transfer(to, amount) -> bool` | Transfer tokens to an address |
| `emit_event(data)` | Emit an event |
| `abort(msg) -> !` | Abort execution |

### Building Contracts

```bash
# Add the WASM target
rustup target add wasm32-unknown-unknown

# Build the example token contract
cd contracts/token
cargo build --target wasm32-unknown-unknown --release
```

The compiled WASM binary will be at:
```
contracts/token/target/wasm32-unknown-unknown/release/rustchain_token_contract.wasm
```

### Native Testing

The SDK includes a thread-local mock environment for testing contracts on x86_64/aarch64 without a WASM runtime:

```rust
use rustchain_sdk as sdk;

#[test]
fn test_transfer() {
    sdk::mock_reset();

    // Configure mock environment
    let alice = [1u8; 20];
    let bob = [2u8; 20];
    sdk::mock_set_caller(alice);
    sdk::mock_set_balance(1000);

    // Run contract logic
    sdk::storage_write(b"bal:alice", &500u64.to_le_bytes());
    assert_eq!(sdk::storage_read(b"bal:alice"), Some(500u64.to_le_bytes().to_vec()));

    // Check transfers
    assert!(sdk::transfer(&bob, 100));
    assert_eq!(sdk::self_balance(), 900);
    assert_eq!(sdk::mock_get_transfers(), vec![(bob, 100)]);

    // Check events
    sdk::emit_event(b"Transfer");
    assert_eq!(sdk::mock_get_events(), vec![b"Transfer".to_vec()]);
}
```

**Mock Configuration Functions:**

| Function | Description |
|----------|-------------|
| `mock_reset()` | Reset environment to defaults (call between tests) |
| `mock_set_caller(addr)` | Set the caller address |
| `mock_set_self_address(addr)` | Set the contract address |
| `mock_set_block_number(n)` | Set block number |
| `mock_set_block_timestamp(ts)` | Set block timestamp |
| `mock_set_chain_id(id)` | Set chain ID |
| `mock_set_balance(bal)` | Set contract balance |
| `mock_get_events() -> Vec<Vec<u8>>` | Get emitted events |
| `mock_get_transfers() -> Vec<([u8; 20], u64)>` | Get transfers made |

### Contract Call Data Encoding

Contract calls encode function name and arguments as:
```
[function_name_length: 4 bytes LE][function_name: UTF-8][arguments: raw bytes]
```

---

## Configuration

The node reads configuration from `config/node.toml`. All sections are optional — sensible defaults are used.

### Genesis

```toml
[genesis]
chain_id = 1
chain_name = "rustchain"
timestamp = 0
gas_limit = 10000000

[[genesis.initial_validators]]
address = "0x..."
public_key = "..."
stake = 10000000000000000000  # 10 RCT (18 decimals)

[[genesis.initial_accounts]]
address = "0x..."
balance = 1000000000000000000000  # 1000 RCT

[genesis.consensus_params]
block_time_ms = 5000           # 5 second block time
epoch_length = 100             # 100 blocks per epoch
min_validators = 1
max_validators = 100
min_stake = 1000000000000000000  # 1 RCT minimum stake
slash_fraction_double_sign = 500 # 5% slash for double-signing
slash_fraction_downtime = 100    # 1% slash for downtime
downtime_jail_duration_ms = 600000  # 10 min jail
unbonding_period_epochs = 10
max_missed_blocks = 50
signed_blocks_window = 100
```

### Storage

```toml
[storage]
path = "./data/chaindb"
cache_size_mb = 256
max_open_files = 512
write_buffer_size_mb = 64
enable_statistics = false
```

### Network

```toml
[network]
listen_addresses = ["/ip4/0.0.0.0/tcp/30303"]
external_addresses = []
bootnodes = []
max_peers = 50
min_peers = 5
enable_mdns = true
enable_kademlia = true
request_timeout_secs = 30
connection_timeout_secs = 10
max_message_size = 4194304  # 4 MB
```

### Consensus

```toml
[consensus]
chain_id = 1
block_time_ms = 5000
max_transactions_per_block = 1000
mempool_size = 10000
min_gas_price = 1
enable_block_production = false
gas_limit_per_block = 10000000
```

### API

```toml
[api]
bind_address = "0.0.0.0:8080"
cors_origins = ["*"]
max_request_body_size = 2097152  # 2 MB
ws_max_connections = 1024
metrics_enabled = true

[api.rate_limit]
requests_per_second = 100
burst_size = 200

[api.auth]
jwt_secret = "change-me-in-production"
jwt_expiration_secs = 3600
enable_rbac = false
admin_addresses = []

# Optional TLS
[api.tls]
cert_path = "./certs/server.crt"
key_path = "./certs/server.key"
```

### VM

```toml
[vm]
max_memory_pages = 256     # 16 MB WASM memory
max_call_depth = 256
max_code_size = 262144     # 256 KB max contract size
max_stack_size = 1048576   # 1 MB stack
fuel_metering = true
```

### Logging

```toml
[logging]
level = "info"       # trace, debug, info, warn, error
format = "pretty"    # pretty or json
```

---

## Deployment

### Docker

```bash
# Build the image
cd deploy/docker
docker build -t rustchain .

# Run a 3-validator testnet with monitoring
docker-compose up -d
```

Docker Compose starts:
- 3 validator nodes (ports 8080, 8081, 8082)
- Prometheus (port 9100)
- Grafana (port 3000, default credentials: admin/admin)

### Kubernetes

```bash
# Create namespace and config
kubectl apply -f deploy/k8s/namespace.yaml
kubectl apply -f deploy/k8s/configmap.yaml
kubectl apply -f deploy/k8s/secret.yaml

# Deploy nodes
kubectl apply -f deploy/k8s/statefulset.yaml
kubectl apply -f deploy/k8s/service.yaml
kubectl apply -f deploy/k8s/ingress.yaml

# Security and availability
kubectl apply -f deploy/k8s/networkpolicy.yaml
kubectl apply -f deploy/k8s/pdb.yaml
kubectl apply -f deploy/k8s/hpa.yaml
```

Kubernetes features:
- **StatefulSet** with persistent volume claims for chain data
- **Service** for internal and external networking
- **Ingress** for HTTP/HTTPS routing
- **HPA** for horizontal pod autoscaling based on CPU/memory
- **NetworkPolicy** for pod-level firewall rules
- **PDB** for disruption tolerance during upgrades
- **Prometheus** monitoring via `deploy/k8s/monitoring/prometheus.yml`

---

## Security

### Cryptography
- **Signatures**: Ed25519 (via ed25519-dalek)
- **Hashing**: Blake3 for all block/tx hashing, Merkle tree construction
- **Key Derivation**: BIP39 mnemonics (12 or 24 words) → HKDF-SHA256
- **Keystores**: scrypt (N=2^14, r=8, p=1) + AES-256-GCM encryption
- **Memory Safety**: `zeroize` crate for key material, `Drop` trait zeroing

### Transport
- **P2P**: Noise protocol encryption (via libp2p)
- **API**: TLS 1.3 with rustls (optional, configurable)

### Authentication & Authorization
- **JWT**: Configurable secret and expiration
- **RBAC Roles**: Admin, Validator, User, ReadOnly
- **Rate Limiting**: Token bucket with configurable requests/second and burst size

### Network Security
- **Peer Scoring**: Reputation-based peer management
- **Connection Limits**: Configurable max peers
- **Message Limits**: Max message size enforcement (default 4 MB)
- **Peer Banning**: Automatic ban for protocol violations

### Consensus Security
- **Double-Sign Detection**: Detects and slashes validators signing conflicting blocks
- **Downtime Slashing**: Jails validators missing too many blocks
- **Epoch Boundaries**: Validator set changes only at epoch boundaries
- **Minimum Stake**: Configurable minimum stake for validator activation

### Smart Contract Sandboxing
- **WASM Isolation**: Each contract runs in its own Wasmtime instance
- **Gas Metering**: Fuel-based gas system prevents infinite loops
- **Memory Limits**: Configurable max memory pages (default 16 MB)
- **Call Depth**: Max 256 nested calls
- **Code Size**: Max 256 KB contract size

---

## Testing

```bash
# Run all 78 tests
cargo test --workspace

# Run tests for specific crates
cargo test -p rustchain-crypto     # 30 tests: keys, hashing, merkle, BIP39, keystores
cargo test -p rustchain-core       # 21 tests: blocks, transactions, accounts, genesis, validators
cargo test -p rustchain-consensus  # 16 tests: PoS selection, finality, slashing, mempool
cargo test -p rustchain-sdk        #  8 tests: mock storage, caller, transfers, events, abort
cargo test -p rustchain-vm         #  3 tests: gas metering

# Lint check (0 warnings)
cargo clippy --workspace

# Format check
cargo fmt --all -- --check
```

### Test Coverage by Module

| Crate | Tests | Coverage Areas |
|-------|-------|----------------|
| rustchain-crypto | 30 | Ed25519 key generation, signing/verification, address derivation, Blake3 hashing, Merkle tree proofs, BIP39 mnemonic generation/import, keystore encrypt/decrypt |
| rustchain-core | 21 | Block creation/validation/signing, transaction signing/verification, account state, genesis config validation, validator set management |
| rustchain-consensus | 16 | Weighted validator selection, deterministic proposer, BFT finality quorum, double-sign detection/slashing, downtime slashing, mempool insert/dedup/selection, gas price filtering |
| rustchain-sdk | 8 | Mock storage read/write, caller/address configuration, block info, token transfers (success + insufficient), event emission, abort |
| rustchain-vm | 3 | Gas meter consumption, exact limit, overflow protection |

---

## License

MIT OR Apache-2.0
