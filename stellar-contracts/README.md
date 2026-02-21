# Stellar Soroban Contracts

Smart contracts for the Neko Protocol on Stellar Soroban. This workspace contains Real-World Asset (RWA) related contracts built using Soroban SDK.

## Project Structure

This workspace is part of the Neko-DApp monorepo and uses Cargo workspaces:

```text
stellar-contracts/
├── Cargo.toml              # Workspace configuration
├── README.md
├── rwa-oracle/             # RWA Oracle contract for metadata and price feeds
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── rwa-token/              # RWA Token contract with oracle integration
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── rwa-lending/            # RWA Lending contract (Blend-based protocol)
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
├── rwa-vault/              # RWA Yield Aggregator vault (SEP-41 vTokens, NAV, IAdapter)
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
└── adapter-rwa-lending/    # IAdapter bridge: vault ↔ rwa-lending (cross-contract auth)
    ├── Cargo.toml
    ├── README.md
    └── src/
```

## Contracts

### rwa-oracle

Oracle contract for Real-World Asset metadata and price feeds. Extends SEP-40 Oracle Consumer Interface with comprehensive RWA metadata support.

**Key Features:**

- SEP-40 compatible price feed interface
- RWA metadata management
- Regulatory compliance tracking
- Support for multiple asset types (stocks, bonds, commodities, real estate, etc.)

See [rwa-oracle/README.md](./rwa-oracle/README.md) for detailed documentation.

### rwa-token

Fungible token contract for Real-World Assets with integrated RWA Oracle price feeds.

**Key Features:**

- Standard fungible token operations
- RWA Oracle integration for price feeds
- Admin controls and token management

See [rwa-token/README.md](./rwa-token/README.md) for detailed documentation.

### rwa-lending

Lending and borrowing protocol for Real-World Assets based on the Blend protocol design.

**Key Features:**

- Lending and borrowing operations
- Collateral management
- Interest rate calculations
- Liquidation mechanisms
- Integration with RWA Oracle and RWA Token contracts

See [rwa-lending/README.md](./rwa-lending/README.md) for detailed documentation.

### rwa-vault

Yield aggregator vault for Real-World Assets. Accepts a deposit token, distributes across protocols via adapters, and mints **vTokens** (SEP-41) representing proportional NAV ownership.

**Key Features:**

- SEP-41 vToken shares (transfer, approve, burn)
- NAV accounting and share pricing (1:1 first deposit)
- IAdapter trait for protocol-agnostic yield routing
- Strategies: Optimizer, Rebalancer, Harvester (management + performance fees)
- High water mark for performance fee

See [rwa-vault/README.md](./rwa-vault/README.md) for detailed documentation.

### adapter-rwa-lending

Bridge adapter connecting rwa-vault to rwa-lending. Implements IAdapter with cross-contract auth so the vault can deposit/withdraw through the adapter while the adapter holds bTokens in the lending pool.

**Key Features:**

- IAdapter: `a_deposit`, `a_withdraw`, `a_balance`, `a_get_apy`, `a_harvest`
- Cross-contract auth via `authorize_as_current_contract` for token transfers
- Single-asset adapter per instance; vault-only access for deposit/withdraw
- Yield reflected in b_rate, realized on withdraw

See [adapter-rwa-lending/README.md](./adapter-rwa-lending/README.md) for detailed documentation.

## Getting Started

### Prerequisites

- Rust (latest stable version)
- Soroban CLI - Install from [Soroban Docs](https://soroban.stellar.org/docs/getting-started/setup)
- Stellar account for deployment

### Build All Contracts

From this directory:

```bash
cargo build --workspace --release
```

### Build Specific Contract

```bash
cargo build --package rwa-oracle --release
cargo build --package rwa-token --release
cargo build --package rwa-lending --release
cargo build --package rwa-vault --release
cargo build --package adapter-rwa-lending --release
```

### Run Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for specific contract
cargo test --package rwa-oracle
cargo test --package rwa-token
cargo test --package rwa-lending
cargo test --package rwa-vault
cargo test --package adapter-rwa-lending
```

### Build WASM Contracts

WASM files are built to `target/wasm32-unknown-unknown/release/` (or `target/wasm32v1-none/release/` when using that target for contractimport! paths):

```bash
# Build oracle first so rwa-token and rwa-lending can resolve contractimport!
cargo build --package rwa-oracle --target wasm32-unknown-unknown --release
# Then build the rest
cargo build --workspace --target wasm32-unknown-unknown --release
```

## Contract Dependencies

- **rwa-token** depends on **rwa-oracle** (contractimport! of oracle WASM)
- **rwa-lending** depends on **rwa-oracle** and **rwa-token** (contractimport! of oracle WASM)
- **rwa-vault** has no WASM imports; uses IAdapter via contractclient at runtime
- **adapter-rwa-lending** depends on **rwa-lending** (contractimport! of rwa-lending WASM)

When building contracts that import WASM files from other contracts, ensure the dependency contracts are built first:

```bash
# Build in dependency order (oracle first for rwa-token / rwa-lending)
cargo build --package rwa-oracle --target wasm32-unknown-unknown --release
# Copy oracle WASM for contractimport! path if needed:
# mkdir -p target/wasm32v1-none/release && cp target/wasm32-unknown-unknown/release/rwa_oracle.wasm target/wasm32v1-none/release/
cargo build --package rwa-token --target wasm32-unknown-unknown --release
cargo build --package rwa-lending --target wasm32-unknown-unknown --release
# For adapter: build rwa-lending WASM first, then adapter-rwa-lending
cargo build --package adapter-rwa-lending --target wasm32-unknown-unknown --release
```

## Workspace Configuration

- **Rust Edition**: 2024
- **Soroban SDK**: 23.0.4
- **License**: Apache-2.0
- **Author**: OppiaLabs

## Development

This workspace is optimized for release builds with:

- Maximum optimization (`opt-level = "z"`)
- Link-time optimization (LTO)
- Panic abort for smaller binary size
- Overflow checks enabled

For development builds with logging, use:

```bash
cargo build --profile release-with-logs --target wasm32v1-none-unknown
```

## Documentation

For detailed documentation on each contract, see their respective README files:

- [rwa-oracle/README.md](./rwa-oracle/README.md)
- [rwa-token/README.md](./rwa-token/README.md)
- [rwa-lending/README.md](./rwa-lending/README.md)
- [rwa-vault/README.md](./rwa-vault/README.md)
- [adapter-rwa-lending/README.md](./adapter-rwa-lending/README.md)

## Resources

- [Soroban Documentation](https://soroban.stellar.org/docs)
- [Soroban SDK Reference](https://docs.rs/soroban-sdk/)
- [Stellar Developer Portal](https://developers.stellar.org/)
