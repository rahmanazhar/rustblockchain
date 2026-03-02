# RustChain

A production-grade Proof of Stake blockchain built in Rust with WASM smart contracts, enterprise security, and full deployment tooling.

## Features

- **Proof of Stake Consensus** - Weighted validator selection, epoch-based rotation, BFT 2/3 finality, slashing for double-signing and downtime
- **WASM Smart Contracts** - Wasmtime-powered VM with gas metering, sandboxed execution, and a contract SDK
- **P2P Networking** - libp2p with Kademlia DHT, GossipSub, Noise protocol encryption, block sync, peer scoring and banning
- **REST + WebSocket API** - Full HTTP API with JWT authentication, RBAC, rate limiting, audit logging, Prometheus metrics
- **Smart Contract Support** - Deploy, call, and query WASM contracts; ERC20-like token contract included
- **Staking** - Stake/unstake tokens, validator activation/deactivation, epoch-boundary validator set updates
- **Transaction Receipts** - Full receipt tracking with status, gas used, logs, and return data
- **Enterprise Security** - TLS 1.3, encrypted keystores (scrypt + AES-256-GCM), zeroized key material, double-sign detection
- **Monitoring** - Prometheus metrics, Grafana dashboards
- **Deployment Ready** - Docker, Docker Compose, Kubernetes manifests with StatefulSets, NetworkPolicies, HPA

## Architecture

```
rustblockchain/
├── crates/
│   ├── crypto/       # Ed25519, Blake3, Merkle trees, BIP39, encrypted keystores
│   ├── core/         # Block, Transaction, Account, Validator, Genesis types
│   ├── storage/      # RocksDB persistence with column families
│   ├── vm/           # Wasmtime WASM smart contract engine
│   ├── consensus/    # PoS engine, finality, slashing, mempool
│   ├── network/      # libp2p P2P networking
│   ├── api/          # Axum REST + WebSocket server
│   ├── node/         # Main node binary
│   └── wallet/       # CLI wallet
├── contracts/
│   ├── sdk/          # Smart contract SDK
│   └── token/        # Example ERC20-like token
├── deploy/
│   ├── docker/       # Dockerfile + docker-compose
│   └── k8s/          # Kubernetes manifests
└── config/           # Configuration templates
```

## Quick Start

### Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs))
- LLVM/Clang (for RocksDB compilation)
- Git

### Build

```bash
cargo build --release
```

### Run a devnet node

```bash
cargo run --bin rustchain -- run --devnet
```

This starts a single-validator devnet that immediately produces and finalizes blocks. The API server listens on `http://localhost:8080`.

### Generate a validator key

```bash
cargo run --bin rustchain -- keygen --output ./keystore/validator.key
```

### Run in validator mode

```bash
cargo run --bin rustchain -- run --validator --keyfile ./keystore/validator.key
```

## Wallet CLI

### Create a wallet

```bash
cargo run --bin rustchain-wallet -- account create --words 24
```

### Import from mnemonic

```bash
cargo run --bin rustchain-wallet -- account import-mnemonic --phrase "word1 word2 ... word24"
```

### Import from private key

```bash
cargo run --bin rustchain-wallet -- account import-key --key <hex-private-key>
```

### List accounts

```bash
cargo run --bin rustchain-wallet -- account list
```

### Send a transfer

```bash
cargo run --bin rustchain-wallet -- transfer \
  --from <sender-hex> \
  --to <recipient-hex> \
  --amount 1000000
```

### Stake tokens

```bash
cargo run --bin rustchain-wallet -- stake \
  --from <validator-hex> \
  --amount 10000000000000000000
```

### Unstake tokens

```bash
cargo run --bin rustchain-wallet -- unstake \
  --from <validator-hex> \
  --amount 5000000000000000000
```

### Deploy a smart contract

```bash
cargo run --bin rustchain-wallet -- deploy \
  --from <deployer-hex> \
  --wasm ./contracts/token/target/wasm32-unknown-unknown/release/rustchain_token_contract.wasm \
  --gas-limit 1000000
```

### Call a smart contract function

```bash
cargo run --bin rustchain-wallet -- call \
  --from <caller-hex> \
  --contract <contract-hex> \
  --function transfer_tokens \
  --args <hex-encoded-args> \
  --gas-limit 500000
```

### Query chain info

```bash
cargo run --bin rustchain-wallet -- query chain-info
```

### Query balance

```bash
cargo run --bin rustchain-wallet -- query balance --address <hex-address>
```

### Query a block

```bash
cargo run --bin rustchain-wallet -- query block --id 1
```

### Query a transaction

```bash
cargo run --bin rustchain-wallet -- query transaction --hash <tx-hash>
```

## API Endpoints

All endpoints return JSON in the format `{"success": true, "data": {...}}`.

### Health

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Liveness probe |
| `/health/ready` | GET | Readiness probe (storage + consensus checks) |

### Chain

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/chain/info` | GET | Chain metadata (chain_id, height, epoch, finality, validators) |
| `/chain/status` | GET | Lightweight status (height, syncing state, pending tx count) |

### Blocks

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/blocks` | GET | List blocks (paginated: `?page=1&per_page=20`) |
| `/blocks/latest` | GET | Latest block with full details |
| `/blocks/:id` | GET | Block by number or hash |

### Transactions

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/tx/:hash` | GET | Transaction by hash |
| `/tx` | POST | Submit a signed transaction |

### Accounts

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/accounts/:address` | GET | Account info (balance, nonce, code_hash) |
| `/accounts/:address/balance` | GET | Account balance only |

### Validators

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/validators` | GET | List all validators with stake, status, uptime |

### Receipts

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/receipts/:hash` | GET | Transaction receipt (status, gas_used, logs, contract_address) |

### Contracts

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/contracts/:address` | GET | Contract info (balance, code_hash, nonce) |
| `/contracts/:address/storage/:key` | GET | Read contract storage by hex key |
| `/contracts/:address/code` | GET | Contract bytecode and code hash |

### Monitoring

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/metrics` | GET | Prometheus metrics (when enabled) |
| `/ws` | WS | WebSocket subscriptions for new blocks, transactions, finality |

## API Curl Examples

All examples assume the node is running on `localhost:8080` (default devnet).

### Health & Status

```bash
# Liveness check
curl http://localhost:8080/health

# Readiness check (verifies storage + consensus)
curl http://localhost:8080/health/ready
```

### Chain Information

```bash
# Full chain info (chain_id, height, epoch, finalized height, active validators)
curl http://localhost:8080/chain/info

# Lightweight status (good for polling/monitoring)
curl http://localhost:8080/chain/status
```

### Blocks

```bash
# List blocks (paginated, newest first)
curl "http://localhost:8080/blocks?page=1&per_page=10"

# Get the latest block
curl http://localhost:8080/blocks/latest

# Get block by number
curl http://localhost:8080/blocks/1

# Get block by hash
curl http://localhost:8080/blocks/0xabc123...
```

### Transactions

```bash
# Get transaction by hash
curl http://localhost:8080/tx/0xabc123...

# Submit a signed transaction
curl -X POST http://localhost:8080/tx \
  -H "Content-Type: application/json" \
  -d '{
    "chain_id": 9999,
    "nonce": 0,
    "from": "d832f4e7b04224b04c796a2d6c583900fb8f8395",
    "to": "1234567890abcdef1234567890abcdef12345678",
    "value": "1000000",
    "tx_type": "transfer",
    "gas_limit": 21000,
    "gas_price": 1,
    "data": "",
    "timestamp": 1700000000000,
    "public_key": "<hex-encoded-ed25519-public-key>",
    "signature": "<hex-encoded-ed25519-signature>"
  }'
```

Transaction types for `tx_type`: `transfer`, `stake`, `unstake`, `contract_deploy`, `contract_call`.

### Accounts

```bash
# Get full account info (balance, nonce, code_hash if contract)
curl http://localhost:8080/accounts/d832f4e7b04224b04c796a2d6c583900fb8f8395

# Get balance only
curl http://localhost:8080/accounts/d832f4e7b04224b04c796a2d6c583900fb8f8395/balance
```

### Validators

```bash
# List all validators (address, stake, active status, commission, jail status, uptime)
curl http://localhost:8080/validators
```

### Transaction Receipts

```bash
# Get receipt by transaction hash (status, gas_used, logs, contract_address)
curl http://localhost:8080/receipts/0xabc123...
```

### Smart Contracts

```bash
# Get contract info (address, balance, code_hash, nonce)
curl http://localhost:8080/contracts/1234567890abcdef1234567890abcdef12345678

# Read contract storage by hex-encoded key
curl http://localhost:8080/contracts/1234567890abcdef1234567890abcdef12345678/storage/0x746f74616c5f737570706c79

# Get contract bytecode
curl http://localhost:8080/contracts/1234567890abcdef1234567890abcdef12345678/code
```

### Prometheus Metrics

```bash
# Scrape Prometheus metrics
curl http://localhost:8080/metrics
```

Available metrics:
- `rustchain_chain_height` - Current blockchain height
- `rustchain_blocks_processed_total` - Total blocks processed
- `rustchain_transactions_processed_total` - Total transactions processed
- `rustchain_peer_count` - Connected P2P peers
- `rustchain_active_connections` - Active API/WebSocket connections

### WebSocket

```bash
# Connect to WebSocket for real-time events
websocat ws://localhost:8080/ws
```

Events streamed: `NewBlock`, `NewTransaction`, `BlockFinalized`.

## Docker

```bash
# Run a 3-validator testnet with monitoring
cd deploy/docker
docker-compose up -d
```

This starts:
- 3 validator nodes (ports 8080-8082)
- Prometheus (port 9100)
- Grafana (port 3000, admin/admin)

## Kubernetes

```bash
kubectl apply -f deploy/k8s/namespace.yaml
kubectl apply -f deploy/k8s/configmap.yaml
kubectl apply -f deploy/k8s/secret.yaml
kubectl apply -f deploy/k8s/statefulset.yaml
kubectl apply -f deploy/k8s/service.yaml
kubectl apply -f deploy/k8s/ingress.yaml
kubectl apply -f deploy/k8s/networkpolicy.yaml
kubectl apply -f deploy/k8s/pdb.yaml
kubectl apply -f deploy/k8s/hpa.yaml
```

## Security

- **Transport**: Noise protocol (P2P), TLS 1.3 (API)
- **Authentication**: JWT tokens with configurable expiration
- **Authorization**: Role-based access control (Admin, Validator, User, ReadOnly)
- **Cryptography**: Ed25519 signatures, Blake3 hashing, BIP39 mnemonics
- **Key Management**: Encrypted keystores (scrypt + AES-256-GCM), zeroize-on-drop, interactive password prompts
- **Network**: Peer scoring, connection limits, message size limits, rate limiting, peer banning
- **Consensus**: Double-sign detection and slashing, epoch-boundary validator set changes, minimum stake enforcement
- **Smart Contracts**: WASM sandboxing, gas metering, memory limits, call depth limits (256), code size limits

## Configuration

See `config/node.toml` for the full configuration reference. Key sections:

- `[genesis]` - Chain ID, name, initial validators and accounts
- `[storage]` - RocksDB cache, file limits
- `[network]` - Listen addresses, bootnodes, peer limits
- `[consensus]` - Block time, epoch length, slashing parameters
- `[api]` - Bind address, TLS, CORS, rate limiting, JWT auth
- `[vm]` - WASM memory limits, gas metering

## Testing

```bash
# Run all tests (70 tests across all crates)
cargo test --workspace

# Run tests for a specific crate
cargo test -p rustchain-crypto
cargo test -p rustchain-core
cargo test -p rustchain-consensus

# Lint check
cargo clippy --workspace
```

## License

MIT OR Apache-2.0
