<h1 align="center">Neko Contracts</h1>

<p align="center">
  <strong>Multi-chain Real-World Asset (RWA) lending and yield aggregation protocol</strong>
</p>

<p align="center">
  <a href="https://github.com/Neko-Protocol">Neko Protocol</a> · Ethereum (EVM) · Stellar Soroban
</p>

Neko Protocol enables users to deposit tokenized real-world assets (RWAs) as collateral, borrow stablecoins, and earn yield through automated vault strategies. This monorepo contains smart contract implementations for both EVM-compatible chains (Ethereum, Foundry/Solidity) and Stellar Soroban (Cargo/Rust).

## Multi-Chain Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              Neko Protocol                                    │
├─────────────────────────────┬────────────────────────────────────────────────┤
│      EVM (Ethereum)         │              Stellar (Soroban)                  │
│                             │                                                  │
│  ┌──────────────────────┐   │   ┌────────────────┐  ┌────────────────┐       │
│  │     LendingPool      │   │   │   rwa-oracle   │  │   rwa-token    │       │
│  │  ┌────────┐ ┌──────┐ │   │   │    (SEP-40)    │  │   (SEP-41)     │       │
│  │  │OToken  │ │Debt  │ │   │   └───────┬────────┘  └───────┬────────┘       │
│  │  │        │ │Token │ │   │           │  prices            │  collateral     │
│  │  └────────┘ └──────┘ │   │           └─────────┬──────────┘                │
│  └───────────┬──────────┘   │                     ▼                           │
│              │              │   ┌────────────────────────────────┐            │
│  ┌───────────▼──────────┐   │   │          rwa-lending           │            │
│  │     PriceOracle      │   │   │  bTokens · dTokens · Backstop  │            │
│  │  (Chainlink / Pyth)  │   │   │  Dutch auctions · AssetType    │            │
│  └───────────┬──────────┘   │   └───────────────┬────────────────┘            │
│              │              │                   │                              │
│  ┌───────────▼──────────┐   │   ┌────────────────────────────────┐            │
│  │       Backstop       │   │   │           rwa-vault             │            │
│  │  (Emergency reserves)│   │   │  vTokens (SEP-41) · NAV        │            │
│  └──────────────────────┘   │   │  Optimizer · Rebalancer · Fees  │            │
│                             │   └──────┬──────────┬──────────┬───┘            │
│                             │          │          │          │                 │
│                             │          ▼          ▼          ▼                 │
│                             │   ┌──────────┐ ┌────────┐ ┌─────────────┐      │
│                             │   │ adapter- │ │adapter-│ │  adapter-   │      │
│                             │   │ rwa-lend │ │ blend  │ │  soroswap   │      │
│                             │   │ IAdapter │ │IAdapter│ │  IAdapter   │      │
│                             │   └──────────┘ └────────┘ └─────────────┘      │
└─────────────────────────────┴────────────────────────────────────────────────┘
```

## Repository Structure

```
neko-contracts/
├── evm-contracts/
│   └── rwa-lending/            # Foundry — Solidity 0.8.27
│       ├── src/                # LendingPool, Backstop, OToken, DebtToken, Oracles
│       ├── test/               # Forge tests
│       └── script/             # Deployment scripts
└── stellar-contracts/          # Cargo workspace — Soroban SDK 23.0.4, Rust 2024
    ├── rwa-oracle/             # SEP-40 RWA price feeds and metadata
    ├── rwa-token/              # SEP-41 + SEP-57 regulated RWA token
    ├── rwa-lending/            # Blend-based lending with Dutch auctions
    ├── rwa-vault/              # Yield aggregator with NAV and vTokens
    ├── adapter-rwa-lending/    # IAdapter bridge: vault ↔ rwa-lending
    ├── adapter-blend/          # IAdapter bridge: vault ↔ Blend Protocol
    └── adapter-soroswap/       # IAdapter bridge: vault ↔ Soroswap AMM
```

---

## EVM Contracts (`evm-contracts/rwa-lending/`)

A permissionless lending protocol for tokenized RWA assets on Ethereum. Users deposit Ondo/Backed RWA tokens as collateral and borrow stablecoins (USDC, USDT).

### Contracts

| Contract | Description |
| -------- | ----------- |
| `LendingPool.sol` | Core protocol — collateral management, borrowing, liquidation |
| `Backstop.sol` | Emergency reserve for bad debt coverage |
| `OToken.sol` | Non-transferable collateral receipt token (ERC20) |
| `DebtToken.sol` | Non-transferable debt tracking token (ERC20) |
| `PriceOracle.sol` | Chainlink price feed integration |
| `PythPriceOracle.sol` | Pyth network price feed integration |
| `CustomPriceOracle.sol` | Manual price updates for assets without on-chain feeds |

**Key features:** Chainlink / Pyth oracles · WadRayMath fixed-point math (WAD=18, RAY=27) · ReentrancyGuard · Dynamic interest rates · Emergency liquidation via backstop · OpenZeppelin AccessControl

### Quick Start

```bash
cd evm-contracts/rwa-lending
bash setup.sh

# Build
forge build

# Test
forge test -vvv

# Gas report
forge test --gas-report

# Deploy
forge script script/Deploy.s.sol --rpc-url $ETH_RPC_URL --broadcast
```

See [evm-contracts/rwa-lending/README.md](./evm-contracts/rwa-lending/README.md) for full documentation.

---

## Stellar Contracts (`stellar-contracts/`)

A composable DeFi stack for RWAs on Stellar Soroban — from oracle price feeds and regulated tokens to a full lending protocol, yield aggregator vault, and protocol adapter.

### Contracts

| Contract | Description | Standard | Tests |
| -------- | ----------- | -------- | ----- |
| `rwa-oracle` | RWA price feeds and metadata | SEP-40 | 27 |
| `rwa-token` | Regulated fungible RWA token | SEP-41 + SEP-57 | 16 |
| `rwa-lending` | Lending, borrowing, Dutch auctions | Blend-based | 17 |
| `rwa-vault` | Yield aggregator with vTokens and NAV | SEP-41 | 12 |
| `adapter-rwa-lending` | IAdapter bridge: vault ↔ rwa-lending | IAdapter | 5 |
| `adapter-blend` | IAdapter bridge: vault ↔ Blend Protocol (+ BLND harvest) | IAdapter | 5 |
| `adapter-soroswap` | IAdapter bridge: vault ↔ Soroswap AMM (single-asset LP) | IAdapter | 6 |

**Key features:** SEP-40/41/57 standards · Dutch auction liquidations · Yield aggregation · Cross-contract auth pattern · Blend V2 3-segment interest rates · AssetType oracle routing · Multi-protocol adapter system (lending + AMM + external pools)

### Quick Start

```bash
cd stellar-contracts

# Build all
cargo build --workspace --release

# Test all
cargo test --workspace

# Build WASM (dependency order)
cargo build --target wasm32v1-none --release -p rwa-oracle
cargo build --target wasm32v1-none --release -p rwa-lending
cargo build --target wasm32v1-none --release -p adapter-rwa-lending
cargo build --target wasm32v1-none --release -p adapter-blend
cargo build --target wasm32v1-none --release -p adapter-soroswap
```

See [stellar-contracts/README.md](./stellar-contracts/README.md) for full documentation.

---

## Shared Protocol Concepts

Both implementations share the same core lending mechanics, adapted to each chain's design patterns:

| Concept | EVM | Stellar |
| ------- | --- | ------- |
| Collateral tokens | OTokens (ERC20) | bTokens (12-decimal rate) |
| Debt tokens | DebtTokens (ERC20) | dTokens (12-decimal rate) |
| Price feeds | Chainlink / Pyth | SEP-40 (RWA Oracle + Reflector) |
| Interest model | 2-segment piecewise | 3-segment piecewise + rate modifier |
| Liquidation | Direct liquidation (50% cap) | Dutch auction (200 blocks, ~17 min) |
| Emergency reserve | Backstop contract | Backstop module |
| Health factor | < 1.0 triggers liquidation | min 1.1 / max 1.15 constraints |
| Math precision | WAD (18) / RAY (27) | SCALAR_7 (rates) / SCALAR_12 (token rates) |

## Environment Variables

### EVM

```bash
PRIVATE_KEY=          # Deployer private key
ETH_RPC_URL=          # Ethereum mainnet RPC
SEPOLIA_RPC_URL=      # Sepolia testnet RPC
ETHERSCAN_API_KEY=    # Contract verification
```

## Tech Stack

| Layer | EVM | Stellar |
| ----- | --- | ------- |
| Language | Solidity 0.8.27 | Rust (Edition 2024) |
| Framework | Foundry | Cargo / Soroban SDK 23.0.4 |
| EVM version | Cancun | — |
| Optimizer | 200 runs | `opt-level = "z"`, LTO |
| Libraries | OpenZeppelin | soroban-sdk |

## License

MIT

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
