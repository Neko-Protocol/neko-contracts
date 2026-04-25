# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

NekoContracts is a multi-chain Real-World Asset (RWA) lending protocol with implementations for both EVM (Ethereum/Foundry) and Stellar (Soroban/Rust). Users can deposit tokenized real-world assets as collateral and borrow stablecoins.

## Build & Test Commands

### EVM Contracts (Foundry)
```bash
# Build
forge build
forge build --sizes                           # With contract sizes

# Test
forge test                                    # Run all tests
forge test -vvv                               # Verbose output
forge test --match-path test/LendingPool.t.sol  # Single test file
forge test --match-test testDeposit           # Single test function
forge test --fork-url $ETH_RPC_URL            # Fork testing
forge test --gas-report                       # Gas profiling

# Deploy
forge script script/Deploy.s.sol --rpc-url $ETH_RPC_URL --broadcast
forge script script/DeployPyth.s.sol --rpc-url $ETH_RPC_URL --broadcast
```

### Stellar Contracts (Cargo/Soroban)
```bash
# Build
cargo build --workspace --release
cargo build --package rwa-oracle --release            # Single package
cargo build --package adapter-blend --release         # Single adapter
cargo build --target wasm32v1-none --release          # WASM target

# Test
cargo test --workspace
cargo test --package neko-pool                        # Single package
cargo test --package adapter-neko                     # Adapter tests

# Fuzz (separate workspace; nightly recommended)
cd test-suites/fuzz && cargo build --release        # Compile harnesses
# cargo install cargo-fuzz && cargo fuzz run fuzz_pool_general
```

### Quick Setup (EVM)
```bash
cd evm-contracts/rwa-lending && bash setup.sh
```

## Architecture

### EVM (`evm-contracts/rwa-lending/`)
- **Core:** `LendingPool.sol` (main protocol), `Backstop.sol` (emergency reserves)
- **Tokens:** `OToken.sol` (collateral receipts), `DebtToken.sol` (debt tracking) - both non-transferable
- **Oracles:** Chainlink (`PriceOracle.sol`), Pyth (`PythPriceOracle.sol`), Custom (`CustomPriceOracle.sol`)
- **Math:** WadRayMath library for fixed-point precision (WAD=18, RAY=27 decimals)
- **Interfaces:** `ILendingPool`, `IBackstop`, `IOToken`, `IDebtToken`, `IPriceOracle`

### Stellar (`stellar-contracts/`)

#### Core Packages
- **rwa-oracle:** SEP-40 compliant RWA price feeds and metadata
- **rwa-token:** SEP-41 + SEP-57 regulated RWA token with compliance controls and oracle integration
- **rwa-lending:** Blend-based lending with Dutch auction liquidations
  - Operations: `backstop`, `bad_debt`, `borrowing`, `collateral`, `interest`, `interest_auction`, `lending`, `liquidations`, `oracles`
- **rwa-vault:** SEP-41 yield aggregator with NAV-based vToken pricing and multi-strategy rebalancing

#### Adapters (`stellar-contracts/adapters/`)
All adapters implement the IAdapter protocol to plug into rwa-vault:
- **adapter-blend:** Bridge vault to Blend Protocol lending pools with BLND token harvesting
- **adapter-soroswap:** Bridge vault to Soroswap AMM for single-asset LP positions
- **adapter-rwa-lending:** Bridge vault to the native rwa-lending protocol

#### WASMs (`stellar-contracts/wasms/`)
- **external_wasms/blend/:** `backstop.wasm`, `comet.wasm`, `emitter.wasm`, `pool.wasm`, `pool_factory.wasm`
- **external_wasms/soroswap/:** `factory.wasm`, `pair.wasm`, `router.wasm`
- **neko_wasms/:** Built Neko contract WASMs (generated, not committed)

### Shared Concepts
- Health factor calculations for liquidation triggers
- Dynamic interest rates based on utilization
- Collateral factors (LTV, liquidation thresholds)
- Reserve/backstop mechanisms for protocol safety
- IAdapter pattern for composable yield strategies in rwa-vault

## Environment Variables

**EVM:** `PRIVATE_KEY`, `ETH_RPC_URL`, `SEPOLIA_RPC_URL`, `ETHERSCAN_API_KEY`

## Key Conventions

- EVM uses Solidity 0.8.27 with optimizer (200 runs), Cancun EVM version
- Stellar uses Soroban SDK 25.3.0, Rust Edition 2024
- Tests mirror source structure in `test/` directories
- OpenZeppelin contracts for ERC20, AccessControl, ReentrancyGuard
- Adapters are `cdylib` crates; some also expose `rlib` for testing utilities
- Workspace Cargo.toml at `stellar-contracts/Cargo.toml` defines all members including adapters
