# API Reference

RustChain exposes a REST API + WebSocket server via Axum. Default address: `http://localhost:8080`.

All responses follow the format:
```json
// Success
{"success": true, "data": {...}}

// Error
{"success": false, "error": {"code": 400, "message": "..."}}
```

---

## Table of Contents

- [Health](#health)
- [Chain](#chain)
- [Blocks](#blocks)
- [Transactions](#transactions)
- [Accounts](#accounts)
- [Validators](#validators)
- [Receipts](#receipts)
- [Contracts](#contracts)
- [Wallet](#wallet)
- [Bridge](#bridge)
- [Bridge HTLC](#bridge-htlc)
- [Bridge Validators](#bridge-validators)
- [Monitoring](#monitoring)
- [Authentication](#authentication)
- [Error Codes](#error-codes)

---

## Health

### GET /health

Liveness probe. Returns 200 if the server is running.

```bash
curl http://localhost:8080/health
```

### GET /health/ready

Readiness probe. Checks storage and consensus subsystems.

```bash
curl http://localhost:8080/health/ready
```

---

## Chain

### GET /chain/info

Chain metadata including height, epoch, finalized height, and active validators.

```bash
curl http://localhost:8080/chain/info
```

**Response:**
```json
{
  "success": true,
  "data": {
    "chain_id": 1,
    "chain_name": "rustchain-devnet",
    "height": 42,
    "finalized_height": 40,
    "epoch": 0,
    "active_validators": 1,
    "pending_transactions": 0
  }
}
```

### GET /chain/status

Lightweight status: height, syncing state, and pending transaction count.

```bash
curl http://localhost:8080/chain/status
```

---

## Blocks

### GET /blocks?page=1&per_page=20

Paginated block list, newest first.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | int | 1 | Page number |
| `per_page` | int | 20 | Blocks per page |

```bash
curl "http://localhost:8080/blocks?page=1&per_page=10"
```

### GET /blocks/latest

Latest block with full transaction list.

```bash
curl http://localhost:8080/blocks/latest
```

### GET /blocks/:id

Block by number or hex hash.

```bash
# By number
curl http://localhost:8080/blocks/1

# By hash
curl http://localhost:8080/blocks/0xabc123...
```

---

## Transactions

### GET /tx/:hash

Transaction by hash, including receipt if available.

```bash
curl http://localhost:8080/tx/<tx-hash>
```

### POST /tx

Submit a signed transaction. May be RBAC-gated when authentication is enabled.

```bash
curl -X POST http://localhost:8080/tx \
  -H "Content-Type: application/json" \
  -d '{"signed_transaction": "..."}'
```

---

## Accounts

### GET /accounts/:address

Account info: balance, nonce, code hash (for contracts).

```bash
curl http://localhost:8080/accounts/0x0101010101010101010101010101010101010101
```

**Response:**
```json
{
  "success": true,
  "data": {
    "address": "0x0101010101010101010101010101010101010101",
    "balance": 1000000000000000000000,
    "nonce": 5,
    "code_hash": null
  }
}
```

### GET /accounts/:address/balance

Balance only. Returns 0 for non-existent accounts.

```bash
curl http://localhost:8080/accounts/0x.../balance
```

---

## Validators

### GET /validators

All validators with stake, status, commission, jail info, and slash count.

```bash
curl http://localhost:8080/validators
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "address": "0x...",
      "public_key": "...",
      "stake": 10000000000000000000,
      "status": "Active",
      "commission": 0,
      "jailed": false,
      "slash_count": 0
    }
  ]
}
```

---

## Receipts

### GET /receipts/:hash

Transaction receipt: status, gas used, logs, contract address.

```bash
curl http://localhost:8080/receipts/<tx-hash>
```

**Response:**
```json
{
  "success": true,
  "data": {
    "tx_hash": "...",
    "status": "Success",
    "gas_used": 21000,
    "logs": [],
    "contract_address": null,
    "return_data": null
  }
}
```

---

## Contracts

### GET /contracts/:address

Contract info: balance, code hash, nonce.

```bash
curl http://localhost:8080/contracts/0x...
```

### GET /contracts/:address/storage/:key

Read contract storage by hex key.

```bash
curl http://localhost:8080/contracts/0x.../storage/0x01
```

### GET /contracts/:address/code

Contract bytecode and size.

```bash
curl http://localhost:8080/contracts/0x.../code
```

---

## Wallet

Server-side signing endpoints. The encrypted keystore + password are sent; decryption and signing happen in memory on the server. No private keys are transmitted in plaintext.

### POST /wallet/create

Create a new account with BIP39 mnemonic.

**Request:**
```json
{
  "password": "mypassword",
  "word_count": 12
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `password` | string | yes | Password for keystore encryption |
| `word_count` | int | no | 12 or 24 (default: 12) |

**Response:**
```json
{
  "success": true,
  "data": {
    "address": "0x...",
    "mnemonic": "word1 word2 ... word12",
    "keystore": { ... }
  }
}
```

### POST /wallet/import/mnemonic

Import account from BIP39 mnemonic phrase.

**Request:**
```json
{
  "mnemonic": "word1 word2 ... word12",
  "password": "mypassword"
}
```

### POST /wallet/import/private-key

Import account from hex-encoded private key.

**Request:**
```json
{
  "private_key": "0x<64-hex-chars>",
  "password": "mypassword"
}
```

### POST /wallet/transfer

Send a token transfer.

**Request:**
```json
{
  "keystore": { ... },
  "password": "mypassword",
  "to": "0x<recipient-address>",
  "value": "1000000",
  "gas_limit": 21000,
  "gas_price": 1
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "tx_hash": "0x...",
    "from": "0x...",
    "nonce": 0
  }
}
```

### POST /wallet/stake

Stake tokens for validation.

**Request:**
```json
{
  "keystore": { ... },
  "password": "mypassword",
  "value": "10000000000000000000"
}
```

### POST /wallet/unstake

Unstake tokens (subject to unbonding period).

**Request:**
```json
{
  "keystore": { ... },
  "password": "mypassword",
  "value": "5000000000000000000"
}
```

### POST /wallet/deploy

Deploy a WASM smart contract.

**Request:**
```json
{
  "keystore": { ... },
  "password": "mypassword",
  "bytecode": "<hex-encoded-wasm>",
  "gas_limit": 1000000
}
```

### POST /wallet/call

Call a smart contract function.

**Request:**
```json
{
  "keystore": { ... },
  "password": "mypassword",
  "contract": "0x<contract-address>",
  "function": "transfer_tokens",
  "args": "<hex-encoded-args>",
  "value": "0",
  "gas_limit": 500000
}
```

---

## Bridge

Cross-chain bridge endpoints. See [BRIDGE.md](BRIDGE.md) for detailed documentation.

### GET /bridge/chains

List all supported external chains with their tokens, fees, and limits.

```bash
curl http://localhost:8080/bridge/chains
```

**Response:**
```json
{
  "success": true,
  "data": {
    "chains": [
      {
        "name": "Bitcoin",
        "symbol": "BTC",
        "chain_id": 0,
        "is_evm": false,
        "tokens": [
          {
            "symbol": "BTC",
            "name": "Bitcoin",
            "decimals": 8,
            "enabled": true,
            "min_amount": "10000",
            "max_amount": "100000000000",
            "fee_bps": 10
          }
        ]
      },
      {
        "name": "Ethereum",
        "symbol": "ETH",
        "chain_id": 1,
        "is_evm": true,
        "tokens": [
          {"symbol": "ETH", "name": "Ether", "decimals": 18, "enabled": true, ...},
          {"symbol": "USDT", "name": "Tether USD", "decimals": 6, "enabled": true, ...},
          {"symbol": "USDC", "name": "USD Coin", "decimals": 6, "enabled": true, ...}
        ]
      }
    ]
  }
}
```

### GET /bridge/stats

Bridge statistics.

```bash
curl http://localhost:8080/bridge/stats
```

**Response:**
```json
{
  "success": true,
  "data": {
    "total_transfers": 5,
    "pending_transfers": 1,
    "active_htlcs": 2,
    "supported_chains": 5,
    "bridge_validators": 3,
    "liquidity": [...]
  }
}
```

### GET /bridge/fees

Fee schedule per chain.

```bash
curl http://localhost:8080/bridge/fees
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "chain": "Bitcoin",
      "token_symbol": "BTC",
      "fee_bps": 10,
      "min_fee": 1,
      "estimated_gas_cost": "~0.0001 BTC"
    }
  ]
}
```

### GET /bridge/liquidity

Available bridge liquidity per chain and token.

```bash
curl http://localhost:8080/bridge/liquidity
```

### GET /bridge/transfers

List bridge transfers (max 100, newest first).

```bash
curl http://localhost:8080/bridge/transfers
```

### GET /bridge/transfers/:id

Get a specific bridge transfer by ID.

```bash
curl http://localhost:8080/bridge/transfers/<transfer-id>
```

### POST /bridge/transfer

Initiate a cross-chain transfer.

**Request:**
```json
{
  "sender": "0x<rustchain-address>",
  "dest_chain": "Ethereum",
  "recipient": "0x<external-address>",
  "token_symbol": "ETH",
  "amount": "1000000000000000000"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sender` | string | yes | RustChain sender address (hex) |
| `dest_chain` | string | yes | Destination chain name (see aliases) |
| `recipient` | string | yes | Recipient address on destination chain |
| `token_symbol` | string | yes | Token to bridge (e.g., ETH, BTC, SOL) |
| `amount` | string | yes | Amount in smallest unit |

### POST /bridge/transfer/confirm

Validator confirms a bridge transfer.

**Request:**
```json
{
  "transfer_id": "<transfer-id>",
  "validator": "0x<validator-address>"
}
```

---

## Bridge HTLC

### GET /bridge/htlc

List HTLC atomic swaps (max 100, newest first).

```bash
curl http://localhost:8080/bridge/htlc
```

### GET /bridge/htlc/:id

Get HTLC swap by ID (the hex-encoded hash lock).

```bash
curl http://localhost:8080/bridge/htlc/<swap-id>
```

### POST /bridge/htlc/create

Create a new HTLC atomic swap.

**Request:**
```json
{
  "sender": "0x<sender-address>",
  "recipient": "0x<recipient-address>",
  "amount": "1000000",
  "hash_lock": "0x<64-hex-chars>",
  "external_chain": "Bitcoin",
  "external_address": "bc1q...",
  "external_amount": "100000",
  "timelock": 1709600000
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sender` | string | yes | Sender address on RustChain (hex) |
| `recipient` | string | yes | Recipient address on RustChain (hex) |
| `amount` | string | yes | Amount of RCT to lock |
| `hash_lock` | string | yes | SHA-256 hash of the secret (0x + 64 hex chars) |
| `external_chain` | string | yes | External chain for the counterparty swap |
| `external_address` | string | yes | Counterparty address on external chain |
| `external_amount` | string | yes | Expected amount on external chain |
| `timelock` | int | no | Unix timestamp for expiry (default: now + 1 hour) |

### POST /bridge/htlc/claim

Claim an HTLC by revealing the preimage.

**Request:**
```json
{
  "swap_id": "<htlc-id>",
  "preimage": "0x<secret-hex>",
  "claimer": "0x<recipient-address>"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `swap_id` | string | yes | HTLC swap ID |
| `preimage` | string | yes | The secret whose SHA-256 matches the hash lock |
| `claimer` | string | yes | Must match the HTLC recipient address |

### POST /bridge/htlc/refund

Refund an expired HTLC back to the sender.

**Request:**
```json
{
  "swap_id": "<htlc-id>",
  "refunder": "0x<sender-address>"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `swap_id` | string | yes | HTLC swap ID |
| `refunder` | string | yes | Must match the HTLC sender address |

---

## Bridge Validators

### GET /bridge/validators

List all bridge validators with their address, public key, stake, and confirmation count.

```bash
curl http://localhost:8080/bridge/validators
```

---

## Monitoring

### GET /metrics

Prometheus metrics endpoint (when enabled in configuration).

```bash
curl http://localhost:8080/metrics
```

### WS /ws

WebSocket connection for real-time events. Emits:

| Event | Description |
|-------|-------------|
| `NewBlock` | New block produced |
| `NewTransaction` | New transaction in mempool |
| `BlockFinalized` | Block finalized by consensus |

```bash
# Using websocat
websocat ws://localhost:8080/ws
```

---

## Authentication

When RBAC is enabled (`api.auth.enable_rbac = true`), write endpoints require JWT authentication.

### Roles

| Role | Permissions |
|------|------------|
| `Admin` | Full access |
| `Validator` | Read + submit transactions |
| `User` | Read + submit transactions |
| `ReadOnly` | Read-only access |

### Headers

```
Authorization: Bearer <jwt-token>
```

### Exempt Endpoints

The following endpoints do not require authentication:
- `GET /health`, `GET /health/ready`
- `GET /chain/*`, `GET /blocks/*`, `GET /tx/*`
- `GET /accounts/*`, `GET /validators`
- `POST /wallet/*` (uses password-based keystore auth)
- `GET /bridge/*`, `POST /bridge/*`
- `WS /ws`

---

## Error Codes

| Code | Meaning |
|------|---------|
| 400 | Bad Request — invalid parameters, missing fields |
| 401 | Unauthorized — missing or invalid JWT |
| 403 | Forbidden — insufficient role permissions |
| 404 | Not Found — resource does not exist |
| 429 | Too Many Requests — rate limit exceeded |
| 500 | Internal Server Error |
