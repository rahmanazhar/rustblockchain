# RustChain

A production-grade Proof of Stake blockchain built in Rust with WASM smart contracts, enterprise security, and full deployment tooling.

## Features

- **Proof of Stake Consensus** - Weighted validator selection, epoch-based rotation, BFT 2/3 finality, slashing for double-signing and downtime
- **WASM Smart Contracts** - Wasmtime-powered VM with gas metering, sandboxed execution, and a contract SDK
- **P2P Networking** - libp2p with Kademlia DHT, GossipSub, Noise protocol encryption, block sync
- **REST + WebSocket API** - Full HTTP API with JWT authentication, RBAC, rate limiting, audit logging
- **Enterprise Security** - TLS 1.3, encrypted keystores (scrypt + AES-256-GCM), zeroized key material
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

### Build

```bash
cargo build --release
```

### Run a devnet node

```bash
cargo run --bin rustchain -- run --devnet --validator
```

### Create a wallet

```bash
cargo run --bin rustchain-wallet -- account create --words 24
```

### Query chain info

```bash
cargo run --bin rustchain-wallet -- query chain-info --node http://localhost:8080
```

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

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Liveness probe |
| `/health/ready` | GET | Readiness probe |
| `/chain/info` | GET | Chain information |
| `/blocks` | GET | List blocks (paginated) |
| `/blocks/latest` | GET | Latest block |
| `/blocks/:id` | GET | Block by number or hash |
| `/tx/:hash` | GET | Transaction by hash |
| `/tx` | POST | Submit signed transaction |
| `/accounts/:addr` | GET | Account info |
| `/accounts/:addr/balance` | GET | Account balance |
| `/validators` | GET | List validators |
| `/ws` | WS | WebSocket subscriptions |
| `/metrics` | GET | Prometheus metrics |

## Security

- **Transport**: Noise protocol (P2P), TLS 1.3 (API)
- **Authentication**: JWT tokens with configurable expiration
- **Authorization**: Role-based access control (Admin, Validator, User, ReadOnly)
- **Cryptography**: Ed25519 signatures, Blake3 hashing, BIP39 mnemonics
- **Key Management**: Encrypted keystores (scrypt + AES-256-GCM), zeroize-on-drop
- **Network**: Peer scoring, connection limits, message size limits, rate limiting
- **Consensus**: Slashing for double-signing, epoch-boundary validator set changes

## Configuration

See `config/node.toml` for the full configuration reference. Key sections:

- `[genesis]` - Chain ID, name, initial validators and accounts
- `[storage]` - RocksDB cache, file limits
- `[network]` - Listen addresses, bootnodes, peer limits
- `[consensus]` - Block time, epoch length, slashing parameters
- `[api]` - Bind address, TLS, CORS, rate limiting, JWT auth
- `[vm]` - WASM memory limits, gas metering

## License

MIT OR Apache-2.0
