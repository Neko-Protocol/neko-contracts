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
```

### Stellar Contracts (Cargo/Soroban)
```bash
# Build
cargo build --workspace --release
cargo build --package rwa-oracle --release    # Single package
cargo build --target wasm32v1-none --release  # WASM

# Test
cargo test --workspace
cargo test --package rwa-lending              # Single package
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

### Stellar (`stellar-contracts/`)
- **rwa-lending:** Blend-based lending with Dutch auction liquidations
- **rwa-oracle:** SEP-40 compliant RWA price feeds
- **rwa-token:** RWA token with oracle integration
- Modular design: operations split into `admin/`, `borrowing/`, `collateral/`, `lending/`, `liquidations/`, `backstop/`

### Shared Concepts
- Health factor calculations for liquidation triggers
- Dynamic interest rates based on utilization
- Collateral factors (LTV, liquidation thresholds)
- Reserve/backstop mechanisms for protocol safety

## Environment Variables

**EVM:** `PRIVATE_KEY`, `ETH_RPC_URL`, `SEPOLIA_RPC_URL`, `ETHERSCAN_API_KEY`

## Key Conventions

- EVM uses Solidity 0.8.27 with optimizer (200 runs), Cancun EVM version
- Stellar uses Soroban SDK 23.0.4, Rust Edition 2024
- Tests mirror source structure in `test/` directories
- OpenZeppelin contracts for ERC20, AccessControl, ReentrancyGuard
