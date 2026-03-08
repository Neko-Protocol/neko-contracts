<h1 align="center">Stellar Soroban Contracts</h1>

<p align="center">
  <strong>Part of the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> DeFi ecosystem on Stellar</strong>
</p>

Smart contracts for Neko Protocol on Stellar Soroban. This workspace contains seven interdependent Real-World Asset (RWA) contracts built with Soroban SDK 23.0.4, forming a complete yield aggregation stack — from tokenized assets and price feeds to lending pools, adapters, and vault management.

## Protocol Architecture

The contracts form a composable stack. Data and value flow from oracle → token → lending pool / external protocols ← adapters ← vault:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                        Neko Protocol — Stellar Soroban                        │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│   ┌──────────────┐    prices    ┌──────────────┐                              │
│   │  rwa-oracle  │─────────────▶│  rwa-token   │                              │
│   │   (SEP-40)   │              │   (SEP-41)   │                              │
│   └──────┬───────┘              └──────┬───────┘                              │
│          │ collateral prices           │ deposit_token / collateral             │
│          ▼                             ▼                                        │
│   ┌────────────────────────────────────────────────┐                           │
│   │                  rwa-lending                    │                           │
│   │  ┌──────────┐  ┌──────────┐  ┌─────────────┐  │                           │
│   │  │ bTokens  │  │ dTokens  │  │  Backstop   │  │                           │
│   │  │  (lend)  │  │ (borrow) │  │ (insurance) │  │                           │
│   │  └──────────┘  └──────────┘  └─────────────┘  │                           │
│   └──────────────────────────────────────────────┬─┘                           │
│                                                  │                              │
│   ┌──────────────────┐   ┌──────────────────┐   │                              │
│   │  Blend Protocol  │   │  Soroswap AMM    │   │                              │
│   │  (lending pool)  │   │  (LP positions)  │   │                              │
│   └────────┬─────────┘   └────────┬─────────┘   │                              │
│            │ pool.submit()        │ add/remove   │                              │
│            ▼                      ▼  liquidity   ▼                              │
│   ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐               │
│   │  adapter-blend   │ │adapter-soroswap  │ │adapter-rwa-lending│              │
│   │  IAdapter bridge │ │  IAdapter bridge │ │  IAdapter bridge  │              │
│   │  BLND harvest    │ │  single-asset    │ │  cross-contract   │              │
│   └────────┬─────────┘ └────────┬─────────┘ └────────┬──────────┘              │
│            └──────────┬─────────┘                    │                          │
│                       │  a_deposit / a_withdraw / a_balance / a_harvest         │
│                       ▼                                                          │
│   ┌────────────────────────────────────────────────┐                           │
│   │                  rwa-vault                      │                           │
│   │  ┌──────────┐  ┌──────┐  ┌───────────────────┐ │                           │
│   │  │ vTokens  │  │ NAV  │  │ Optimizer         │ │                           │
│   │  │ (SEP-41) │  │      │  │ Rebalancer        │ │                           │
│   │  └──────────┘  └──────┘  │ Harvester + Fees  │ │                           │
│   │                          └───────────────────┘ │                           │
│   └────────────────────────────────────────────────┘                           │
│                                                                                │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Contracts

| Contract | Description | Standard | Tests |
| -------- | ----------- | -------- | ----- |
| [rwa-oracle](./rwa-oracle) | RWA price feeds and metadata | SEP-40 | 27 |
| [rwa-token](./rwa-token) | Regulated fungible RWA token | SEP-41 + SEP-57 | 16 |
| [rwa-lending](./rwa-lending) | Lending and borrowing with Dutch auctions | Blend-based | 17 |
| [rwa-vault](./rwa-vault) | Yield aggregator with NAV and vTokens | SEP-41 | 12 |
| [adapter-rwa-lending](./adapter-rwa-lending) | IAdapter bridge: vault ↔ rwa-lending | IAdapter | 5 |
| [adapter-blend](./adapter-blend) | IAdapter bridge: vault ↔ Blend Protocol (BLND harvest) | IAdapter | 5 |
| [adapter-soroswap](./adapter-soroswap) | IAdapter bridge: vault ↔ Soroswap AMM (single-asset LP) | IAdapter | 6 |
| [rwa-faucet](./rwa-faucet) | Bulk mint for RWA tokens (testnet/dev) | — | 4 |

---

### rwa-oracle

Oracle contract for Real-World Asset metadata and price feeds. Implements the SEP-40 Oracle Consumer Interface and extends it with comprehensive RWA metadata management: 9 asset types (stocks, bonds, commodities, real estate, etc.), 6 valuation methods, tokenization tracking, price history up to 1,000 records per asset, and configurable staleness.

---

**Key features:** SEP-40 compatible · RWA metadata + asset classification · Price history with auto-pruning · Configurable staleness · TTL management

See [rwa-oracle/README.md](./rwa-oracle/README.md) for detailed documentation.

---

### rwa-token

Regulated fungible token for tokenized RWAs on Stellar. Implements the SEP-41 Token Interface with SEP-57 (T-REX) compliance — freeze enforcement, delegated compliance contract, and identity verifier configuration. Integrates with rwa-oracle for real-time price queries.

**Key features:** SEP-41 full implementation · SEP-57 freeze + delegated compliance · Oracle price integration · Admin mint/clawback/upgrade · MuxedAddress support

See [rwa-token/README.md](./rwa-token/README.md) for detailed documentation.

---

### rwa-lending

Lending and borrowing protocol for Real-World Assets based on the Blend protocol design. Supports both crypto (USDC, XLM) and RWA tokens as lending assets and collateral. Routes oracle calls automatically via `AssetType` — `Crypto` assets use the Reflector oracle, `Rwa` assets use the RWA oracle.

**Key features:** bTokens/dTokens · AssetType oracle routing · 3-segment piecewise interest rate model · Unified Dutch auction system (liquidation, bad debt, interest) · Backstop insurance · Health factor guards (min 1.1, max 1.15)

See [rwa-lending/README.md](./rwa-lending/README.md) for detailed documentation.

---

### rwa-vault

Yield aggregator vault for RWAs. Accepts a single deposit token, distributes capital across lending protocols via the IAdapter interface, and issues SEP-41 vTokens representing proportional NAV ownership. Includes protocol optimizer with risk-tier APY weighting, manager-triggered rebalancing, harvesting, and management + performance fee model with a high water mark.

**Key features:** SEP-41 vTokens (transfer, approve, burn) · NAV-based share pricing · IAdapter protocol routing · Risk-tier optimizer (Low/Medium/High) · Management + performance fees · High water mark · Vault states (Active/Paused/EmergencyExit)

See [rwa-vault/README.md](./rwa-vault/README.md) for detailed documentation.

---

### adapter-rwa-lending

Bridge adapter connecting rwa-vault to rwa-lending. Implements the IAdapter interface with cross-contract auth so the vault can deposit and withdraw through the adapter while the adapter holds bTokens in the lending pool. Each adapter instance manages a single RWA asset in a single rwa-lending pool.

**Key features:** IAdapter (`a_deposit`, `a_withdraw`, `a_balance`, `a_get_apy`, `a_harvest`) · `authorize_as_current_contract` for token transfers · Ceiling division on b_token conversion · Vault-only access control · TTL management

See [adapter-rwa-lending/README.md](./adapter-rwa-lending/README.md) for detailed documentation.

---

### adapter-blend

Bridge adapter connecting rwa-vault to an existing Blend Protocol lending pool. Translates generic `a_deposit` / `a_withdraw` calls into Blend's `pool.submit()` API, holds bTokens as its position, and claims BLND liquidity mining rewards via `a_harvest`. Auto-resolves `reserve_id` and `claim_ids` on initialization by querying `pool.get_reserve(deposit_token)`.

**Key features:** IAdapter full implementation including `a_harvest` · `authorize_as_current_contract` for Blend's internal token transfer · bToken accounting (`balance = b_tokens × b_rate / SCALAR_12`) · Ceiling division on b_token withdraw conversion · BLND emissions forwarded to vault

See [adapter-blend/README.md](./adapter-blend/README.md) for detailed documentation.

---

### adapter-soroswap

Bridge adapter connecting rwa-vault to a Soroswap AMM liquidity pool. Enables single-asset entry: the adapter automatically swaps half the deposit into the pair token, adds liquidity, holds LP tokens as its position, and unwinds back to the deposit token on withdrawal. The pair address is auto-resolved at initialization via `router.router_pair_for(token_a, token_b)`.

**Key features:** Single-asset entry (vault deposits only token_a) · Auto pair resolution via Soroswap router · Pre-computes `b_optimal` before `add_liquidity` to avoid auth mismatch · Proportional LP burn on partial withdrawals · Round-trip swap back to token_a on withdraw · AMM trading fees accrue passively into LP value

See [adapter-soroswap/README.md](./adapter-soroswap/README.md) for detailed documentation.

---

### rwa-faucet

Bulk mint contract for RWA tokens. Mints multiple tokens to multiple recipients in a single invocation — useful for testnet airdrops and dev environments. The faucet must be the admin of each rwa-token it mints; deploy the faucet first, then deploy tokens with `admin = faucet address`.

**Key features:** Bulk mint in one call · Compatible with rwa-token and Stellar Asset Contract · `transfer_token_admins` for handing off token admin

See [rwa-faucet/README.md](./rwa-faucet/README.md) for integration details.

---

## Contract Dependencies

| Contract | WASM imports |
| -------- | ------------ |
| `rwa-oracle` | — |
| `rwa-token` | `rwa-oracle` WASM (`contractimport!`) |
| `rwa-lending` | `rwa-oracle` WASM (`contractimport!`) |
| `rwa-vault` | — (uses `IAdapter` via `contractclient` at runtime) |
| `adapter-rwa-lending` | `rwa-lending` WASM (`contractimport!`) |
| `adapter-blend` | `external_wasms/blend/pool.wasm` (`contractimport!`) |
| `adapter-soroswap` | `external_wasms/soroswap/router.wasm` + `pair.wasm` (`contractimport!`) |
| `rwa-faucet` | — |

Contracts that import WASMs must be built after their dependencies. The external Blend and Soroswap WASMs must be present in `external_wasms/` before building the corresponding adapters.

## Build & Test

### Prerequisites

- Rust (latest stable)
- Soroban CLI — [setup guide](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)

### Build All Contracts

```bash
cargo build --workspace --release
```

### Build WASM (dependency order)

```bash
# 1. Oracle — imported by rwa-token and rwa-lending
cargo build --target wasm32v1-none --release -p rwa-oracle

# 2. Token and lending — lending imported by adapter-rwa-lending
cargo build --target wasm32v1-none --release -p rwa-token
cargo build --target wasm32v1-none --release -p rwa-lending

# 3. Vault and adapters
cargo build --target wasm32v1-none --release -p rwa-vault
cargo build --target wasm32v1-none --release -p adapter-rwa-lending

# 4. External-protocol adapters (require external_wasms/ to be present)
cargo build --target wasm32v1-none --release -p adapter-blend
cargo build --target wasm32v1-none --release -p adapter-soroswap
```

WASM output: `target/wasm32v1-none/release/<contract_name>.wasm`

### Run Tests

```bash
# All contracts
cargo test --workspace

# Individual contracts
cargo test -p rwa-oracle
cargo test -p rwa-token
cargo test -p rwa-lending
cargo test -p rwa-vault
cargo test -p adapter-rwa-lending
cargo test -p adapter-blend
cargo test -p adapter-soroswap
```

## Workspace Configuration

| Setting | Value |
| ------- | ----- |
| Rust Edition | 2024 |
| Soroban SDK | 23.0.4 |
| Optimization | `opt-level = "z"`, LTO enabled |
| Panic | abort (smaller binary) |
| Overflow checks | enabled |
| License | MIT |
| Author | OppiaLabs |

## Workspace Structure

```
stellar-contracts/
├── Cargo.toml                  # Workspace configuration
├── README.md
├── external_wasms/             # Pre-compiled WASMs for compile-time imports
│   ├── blend/                  # Blend Protocol: pool.wasm
│   └── soroswap/               # Soroswap: router.wasm, pair.wasm, factory.wasm
├── rwa-oracle/                 # SEP-40 RWA price feeds and metadata
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── rwa-token/                  # SEP-41 + SEP-57 regulated RWA token
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── rwa-lending/                # Blend-based lending and borrowing
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── rwa-vault/                  # Yield aggregator with vTokens and NAV
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── adapter-rwa-lending/        # IAdapter bridge: vault ↔ rwa-lending
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── adapter-blend/              # IAdapter bridge: vault ↔ Blend Protocol
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
└── adapter-soroswap/           # IAdapter bridge: vault ↔ Soroswap AMM
    ├── Cargo.toml
    ├── README.md
    └── src/
```

## Resources

- [Soroban Documentation](https://developers.stellar.org/docs/build/smart-contracts)
- [Soroban SDK Reference](https://docs.rs/soroban-sdk/)
- [SEP-40 Oracle Standard](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0040.md)
- [SEP-41 Token Standard](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
- [Stellar Developer Portal](https://developers.stellar.org/)

## License

MIT

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
