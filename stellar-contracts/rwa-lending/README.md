<h1 align="center">RWA Lending Contract</h1>

<p align="center">
  <strong>Part of the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> DeFi ecosystem on Stellar</strong>
</p>

A lending and borrowing protocol for Real-World Assets (RWAs) on Stellar Soroban. This contract enables users to lend both crypto assets **and RWA tokens** to earn yield, or borrow against RWA/crypto collateral, powering the lending, borrowing, and yield aggregation features of Neko Protocol.

## Neko Protocol Integration

This lending contract is a core component of the Neko Protocol ecosystem:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Neko Protocol                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────────┐    prices    ┌──────────────┐               │
│   │  RWA Oracle  │─────────────▶│  RWA Token   │               │
│   │   (SEP-40)   │              │   (SEP-41)   │               │
│   └──────┬───────┘              └──────┬───────┘               │
│          │                             │                        │
│          │ collateral prices           │ collateral             │
│          │                             │                        │
│          ▼                             ▼                        │
│   ┌────────────────────────────────────────────┐               │
│   │              RWA Lending                    │               │
│   │  ┌─────────┐  ┌─────────┐  ┌────────────┐  │               │
│   │  │ bTokens │  │ dTokens │  │  Backstop  │  │               │
│   │  │ (Lend)  │  │(Borrow) │  │ (Insurance)│  │               │
│   │  └─────────┘  └─────────┘  └────────────┘  │               │
│   └────────────────────────────────────────────┘               │
│          │                             │                        │
│          │ liquidations                │ mark prices            │
│          ▼                             ▼                        │
│   ┌──────────────┐              ┌──────────────┐               │
│   │Auction Status│              │  RWA Perps   │               │
│   │              │              │  (Futures)   │               │
│   └──────────────┘              └──────────────┘               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- **Dashboard**: Users view their lending positions, collateral, and borrowing capacity
- **Lending**: Deposit USDC/XLM **or RWA tokens** (CETES, USDY etc.) and earn yield via bTokens
- **Borrowing**: Use RWA tokens or crypto as collateral to borrow any registered asset
- **Liquidations**: Dutch auctions ensure protocol solvency
- **Bad Debt Auctions**: Backstop covers uncollateralized debt
- **Interest Auctions**: Distribute protocol interest to backstop holders

## Standards & Architecture

| Component                              | Description                                       | Implementation                                            |
| -------------------------------------- | ------------------------------------------------- | --------------------------------------------------------- |
| **Lending and Borrowing Architecture** | Based on Stellar's lending and borrowing protocol | bTokens/dTokens, interest accrual, unified Dutch auctions |
| **SEP-40 Integration**                 | Oracle price feeds with asset type routing        | RWA Oracle for RWA assets, Reflector for crypto assets    |
| **SEP-41 Tokens**                      | Token interface                                   | RWA tokens as collateral **and** as lending assets        |
| **AssetType Routing**                  | Generic oracle dispatch per asset                 | `Crypto` → Reflector, `Rwa` → RWA Oracle                  |

## Features

- **bTokens**: Represent lender deposits + accrued interest (yield-bearing)
- **dTokens**: Track borrower debt + accrued interest
- **RWA Lending Pools**: RWA tokens (CETES, USDY) can be deposited as lending assets, not only as collateral
- **Multi-Asset Collateral**: RWA tokens and crypto (USDC) can be used as collateral
- **AssetType Routing**: Oracle routing automatically — `Crypto` uses Reflector, `Rwa` uses RWA Oracle
- **Single Debt Asset**: A single asset borrowed at a time by the borrower
- **Dynamic Interest Rates**: 3-segment piecewise linear model with rate modifier (Blend V2 aligned)
- **Unified Dutch Auctions**: Single auction system for liquidations, bad debt, and interest distribution
- **Backstop Module**: First-loss capital to cover bad debt and protect lenders
- **Health Factor Guards**: MIN (1.1) and MAX (1.15) health factor constraints

## Project Structure

```
src/
├── lib.rs                  # Crate root, module declarations, oracle import
├── contract.rs             # #[contract] LendingContract entry point
├── admin/
│   └── mod.rs              # Admin struct (initialize, setters, upgrade)
├── common/
│   ├── mod.rs
│   ├── error.rs            # Error enum (30+ variants)
│   ├── events.rs           # Event types (13 events)
│   ├── storage.rs          # Storage helpers with TTL management
│   └── types.rs            # CDP, AuctionData, InterestRateParams, ReserveData, etc.
├── operations/
│   ├── mod.rs
│   ├── backstop.rs         # Backstop deposits/withdrawals
│   ├── bad_debt.rs         # Bad debt auction creation/filling
│   ├── borrowing.rs        # Borrow, repay, dToken management
│   ├── collateral.rs       # Add/remove collateral
│   ├── interest.rs         # Interest rate calculations, accrual
│   ├── interest_auction.rs # Interest auction creation/filling
│   ├── lending.rs          # Deposit, withdraw, bToken management
│   ├── liquidations.rs     # User liquidation auctions
│   └── oracles.rs          # Oracle price queries
└── test/
    └── mod.rs              # Tests (17 tests)
```

## Core Concepts

### Decimal Precision (Blend V2 Aligned)

| Type                         | Decimals | Constant            | Example                 |
| ---------------------------- | -------- | ------------------- | ----------------------- |
| Interest rate params         | 7        | `SCALAR_7 = 10^7`   | 75% = 7,500,000         |
| Token rates (b_rate, d_rate) | 12       | `SCALAR_12 = 10^12` | 1:1 = 1,000,000,000,000 |
| Health factor                | 7        | -                   | 1.1 = 11,000,000        |

### bTokens (Lender Tokens)

bTokens represent a lender's share of the pool. They automatically accrue interest:

```
bTokens = deposited_amount × SCALAR_12 / bTokenRate
underlying_value = bTokens × bTokenRate / SCALAR_12
```

As borrowers pay interest, `bTokenRate` increases, making bTokens worth more.

### dTokens (Debt Tokens)

dTokens track a borrower's debt. They automatically accrue interest:

```
dTokens = borrowed_amount × SCALAR_12 / dTokenRate
actual_debt = dTokens × dTokenRate / SCALAR_12
```

As interest accrues, `dTokenRate` increases, meaning more must be repaid.

### Reserve Data (Blend V2 Aligned)

```rust
pub struct ReserveData {
    pub b_rate: i128,          // bToken rate (12 decimals)
    pub d_rate: i128,          // dToken rate (12 decimals)
    pub ir_mod: i128,          // Interest rate modifier (7 decimals)
    pub b_supply: i128,        // Total bToken supply
    pub d_supply: i128,        // Total dToken supply
    pub backstop_credit: i128, // Accumulated interest for backstop
    pub last_time: u64,        // Last accrual timestamp
}
```

### AssetType — Oracle Routing

Every registered asset carries an `AssetType` that determines which oracle is
used for price queries:

```rust
pub enum AssetType {
    Crypto,  // Uses the Reflector Oracle (USDC, XLM, etc.)
    Rwa,     // Uses the RWA Oracle (CETES, USDY, NVDA, etc.)
}
```

This enables any asset to participate as either a **lending asset** or
**collateral**, regardless of whether it is a traditional crypto token or an
RWA token.

### Collateralized Debt Position (CDP)

```rust
pub struct CDP {
    pub collateral: Map<Address, i128>,  // RWA token -> amount
    pub debt_asset: Option<Symbol>,       // USDC, XLM, etc.
    pub d_tokens: i128,                   // Debt tokens
    pub created_at: u64,
    pub last_update: u64,
}
```

### Health Factor

```
Health Factor = (Collateral Value × Collateral Factor) / Debt Value
```

- **HF < 1.0**: Position is insolvent, can be liquidated
- **MIN_HEALTH_FACTOR (1.1)**: Minimum after borrow/remove collateral
- **MAX_HEALTH_FACTOR (1.15)**: Maximum after liquidation (prevents over-liquidation)

## Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    rwa_oracle: Address,           // RWA Oracle for collateral prices
    reflector_oracle: Address,     // Reflector Oracle for debt prices
    backstop_threshold: i128,      // Minimum backstop to activate pool
    backstop_take_rate: u32,       // Interest share for backstop (7 decimals, e.g., 500_000 = 5%)
)
```

## Key Functions

### Admin Functions

```rust
// Register an RWA token as a lending asset (pool CETES)
lending.set_token_contract(&symbol_short!("CETES"), &cetes_addr, &AssetType::Rwa);
lending.set_interest_rate_params(&symbol_short!("CETES"), &params);

// Register crypto as a lending asset (pool USDC)
lending.set_token_contract(&symbol_short!("USDC"), &usdc_addr, &AssetType::Crypto);
lending.set_interest_rate_params(&symbol_short!("USDC"), &params);

// Collateral RWA (NVDA as collateral — uses RWA oracle)
lending.set_collateral_factor(&nvda_token, &7_500_000, &AssetType::Rwa, &symbol_short!("NVDA"));

// Collateral Crypto (USDC as collateral — uses Reflector oracle)
lending.set_collateral_factor(&usdc_addr, &8_500_000, &AssetType::Crypto, &symbol_short!("USDC"));

// Set pool state (Active, OnIce, Frozen)
lending.set_pool_state(&PoolState::Active);

// Set backstop token contract
lending.set_backstop_token(&token_address);

// Upgrade contract
lending.upgrade(&new_wasm_hash);
```

### Lending Functions (bTokens)

```rust
// Deposit crypto asset and receive bTokens
let b_tokens = lending.deposit(&lender, &Symbol::new(&env, "USDC"), &1000_0000000)?;

// Withdraw by burning bTokens
let amount = lending.withdraw(&lender, &Symbol::new(&env, "USDC"), &b_tokens)?;

// Query balances and rates
let balance = lending.get_b_token_balance(&lender, &asset);
let rate = lending.get_b_token_rate(&asset);      // 12 decimals
let supply = lending.get_b_token_supply(&asset);
```

### Borrowing Functions (dTokens)

```rust
// Borrow crypto asset (requires collateral first)
let d_tokens = lending.borrow(&borrower, &Symbol::new(&env, "USDC"), &500_0000000)?;

// Repay debt by burning dTokens
let repaid = lending.repay(&borrower, &Symbol::new(&env, "USDC"), &d_tokens)?;

// Query debt
let d_balance = lending.get_d_token_balance(&borrower, &asset);
let d_rate = lending.get_d_token_rate(&asset);    // 12 decimals
let limit = lending.calculate_borrow_limit(&borrower)?;
```

### Collateral Functions

```rust
// Add RWA token as collateral
lending.add_collateral(&borrower, &nvda_token, &100_0000000)?;

// Remove collateral (checks health factor)
lending.remove_collateral(&borrower, &nvda_token, &50_0000000)?;

// Query collateral
let amount = lending.get_collateral(&borrower, &nvda_token);
```

### Interest Functions

```rust
// Get current interest rate for an asset
let rate = lending.get_interest_rate(&asset)?;

// Manually accrue interest (also happens automatically)
lending.accrue_interest(&asset)?;
```

### Liquidation Functions

```rust
// Initiate liquidation for insolvent position (creates Dutch auction)
let auction_id: u32 = lending.initiate_liquidation(
    &borrower,
    &rwa_token,
    &debt_asset,
    &5_000_000  // liquidation percent (50% in 7 decimals)
)?;

// Fill auction as liquidator
lending.fill_auction(&auction_id, &liquidator)?;

// Calculate health factor
let hf = lending.calculate_health_factor(&borrower)?;  // 7 decimals
```

### Bad Debt Auction Functions

```rust
// Check if position has bad debt (debt but no collateral)
let has_bad_debt = lending.has_bad_debt(&borrower);

// Create bad debt auction
let auction_id: u32 = lending.create_bad_debt_auction(&borrower, &debt_asset)?;

// Fill bad debt auction (pay debt, receive backstop tokens)
let backstop_received = lending.fill_bad_debt_auction(&auction_id, &bidder, &amount)?;
```

### Interest Auction Functions

```rust
// Check if interest auction can be created
let can_create = lending.can_create_interest_auction(&asset);

// Get accumulated interest
let interest = lending.get_accumulated_interest(&asset);

// Create interest auction
let auction_id: u32 = lending.create_interest_auction(&asset)?;

// Fill interest auction (pay backstop tokens, receive interest)
let (interest_received, backstop_paid) = lending.fill_interest_auction(
    &auction_id,
    &bidder,
    &asset,
    &5_000_000  // 50% fill (7 decimals)
)?;
```

### Backstop Functions

```rust
// Deposit to backstop module
lending.deposit_to_backstop(&depositor, &amount)?;

// Withdraw from backstop (17-day queue)
lending.withdraw_from_backstop(&depositor, &amount)?;
```

## Interest Rate Model (Blend V2 Aligned)

Dynamic utilization-based interest rate with 3 segments and a rate modifier:

```
Utilization = Total Borrowed / Total Deposited

Segment 1 (0% - target_util):
  rate = r_base + (util / target_util) × r_one
  rate = rate × ir_mod / SCALAR_7

Segment 2 (target_util - max_util):
  rate = r_base + r_one + ((util - target_util) / (max_util - target_util)) × r_two
  rate = rate × ir_mod / SCALAR_7

Segment 3 (max_util - 100%):
  rate = r_base + r_one + r_two + ((util - max_util) / (1 - max_util)) × r_three
  // No ir_mod multiplication in segment 3
```

**Rate Modifier**: Adjusts automatically to maintain target utilization. Range: 0.1x to 10x.

### InterestRateParams (7 decimals)

```rust
pub struct InterestRateParams {
    pub target_util: u32,    // e.g., 7_500_000 = 75%
    pub max_util: u32,       // e.g., 9_500_000 = 95%
    pub r_base: u32,         // e.g., 100_000 = 1%
    pub r_one: u32,          // e.g., 500_000 = 5%
    pub r_two: u32,          // e.g., 5_000_000 = 50%
    pub r_three: u32,        // e.g., 15_000_000 = 150%
    pub reactivity: u32,     // e.g., 200 = 0.00002
}
```

## Unified Dutch Auction System

All auctions use the same `AuctionData` structure:

```rust
pub struct AuctionData {
    pub auction_type: AuctionType,  // UserLiquidation, BadDebt, Interest
    pub user: Address,               // Affected user/protocol
    pub bid: Map<Address, i128>,     // What filler pays
    pub lot: Map<Address, i128>,     // What filler receives
    pub block: u32,                  // Start block
}
```

### Auction Types

| Type                | Trigger                       | Bid (Filler Pays) | Lot (Filler Receives) |
| ------------------- | ----------------------------- | ----------------- | --------------------- |
| **UserLiquidation** | HF < 1.0                      | Debt tokens       | Collateral            |
| **BadDebt**         | Debt with no collateral       | Debt asset        | Backstop tokens       |
| **Interest**        | Accumulated protocol interest | Backstop tokens   | Interest tokens       |

### Auction Mechanics

- **Duration**: 200 blocks (~17 minutes)
- **Lot Modifier**: 0% → 100% over duration (more collateral offered)
- **Bid Modifier**: 100% → 0% over duration (less debt to repay)

## Pool States

| State    | Deposits | Borrows | Liquidations |
| -------- | -------- | ------- | ------------ |
| `Active` | ✅       | ✅      | ✅           |
| `OnIce`  | ✅       | ❌      | ✅           |
| `Frozen` | ❌       | ❌      | ✅           |

## TTL Management

Storage TTL is automatically extended:

| Storage Type | Min TTL  | Bump TTL | Use                |
| ------------ | -------- | -------- | ------------------ |
| Instance     | 30 days  | 31 days  | Pool config, admin |
| Persistent   | 100 days | 120 days | User CDPs          |

## Error Codes

| Range | Category    | Examples                                                                       |
| ----- | ----------- | ------------------------------------------------------------------------------ |
| 1-3   | Admin       | `NotAuthorized`, `NotInitialized`, `AlreadyInitialized`                        |
| 4-6   | General     | `NotPositive`, `ArithmeticError`, `InvalidLedgerSequence`                      |
| 10-13 | Pool        | `PoolFrozen`, `PoolOnIce`, `InsufficientPoolBalance`                           |
| 20-22 | Lending     | `InsufficientBTokenBalance`, `InsufficientDepositAmount`                       |
| 30-36 | Borrowing   | `InsufficientCollateral`, `InsufficientBorrowLimit`, `DebtAssetAlreadySet`     |
| 40-42 | Collateral  | `CollateralNotFound`, `InvalidCollateralFactor`                                |
| 50-53 | Interest    | `InvalidInterestRateParams`, `InvalidUtilizationRatio`                         |
| 60-67 | Liquidation | `CDPNotInsolvent`, `AuctionNotFound`, `AuctionNotActive`, `InvalidFillPercent` |
| 70-74 | Backstop    | `InsufficientBackstopDeposit`, `WithdrawalQueueActive`, `BadDebtNotCovered`    |
| 80-84 | Oracle      | `OraclePriceFetchFailed`, `InvalidOraclePrice`, `TokenContractNotSet`          |

## Events

| Event                         | Data                                                        |
| ----------------------------- | ----------------------------------------------------------- |
| `DepositEvent`                | lender, asset, amount, b_tokens                             |
| `WithdrawEvent`               | lender, asset, amount, b_tokens                             |
| `BorrowEvent`                 | borrower, asset, amount, d_tokens                           |
| `RepayEvent`                  | borrower, asset, amount, d_tokens                           |
| `AddCollateralEvent`          | borrower, rwa_token, amount                                 |
| `RemoveCollateralEvent`       | borrower, rwa_token, amount                                 |
| `LiquidationInitiatedEvent`   | borrower, rwa_token, debt_asset, amounts, auction_id        |
| `LiquidationFilledEvent`      | auction_id, liquidator, collateral_received, debt_paid      |
| `InterestAccruedEvent`        | asset, b_token_rate, d_token_rate, rate_modifier            |
| `BadDebtAuctionCreatedEvent`  | auction_id, borrower, debt_asset, debt_amount               |
| `BadDebtAuctionFilledEvent`   | auction_id, bidder, debt_covered, backstop_tokens           |
| `InterestAuctionCreatedEvent` | auction_id, asset, interest_amount                          |
| `InterestAuctionFilledEvent`  | auction_id, bidder, asset, interest_received, backstop_paid |

## Usage Example

### Pool USDC clásico (crypto lending)

```rust
lending.initialize(&admin, &rwa_oracle, &reflector_oracle, &1000_0000000, &500_000);

// USDC as a lending asset (Reflector oracle)
lending.set_token_contract(&symbol_short!("USDC"), &usdc_token, &AssetType::Crypto);
lending.set_interest_rate_params(&symbol_short!("USDC"), &InterestRateParams {
    target_util: 7_500_000,  max_util: 9_500_000,
    r_base: 100_000,         r_one: 500_000,
    r_two: 5_000_000,        r_three: 15_000_000,
    reactivity: 200,
});

// NVDA as collateral RWA (75% factor, RWA oracle)
lending.set_collateral_factor(&nvda_token, &7_500_000, &AssetType::Rwa, &symbol_short!("NVDA"));
lending.set_pool_state(&PoolState::Active);

// Lender deposits USDC → receives bUSDC
lending.deposit(&lender, &symbol_short!("USDC"), &10000_0000000)?;

// Borrower puts NVDA as collateral → borrows USDC
lending.add_collateral(&borrower, &nvda_token, &100_0000000)?;
lending.borrow(&borrower, &symbol_short!("USDC"), &5000_0000000)?;
lending.repay(&borrower, &symbol_short!("USDC"), &d_tokens)?;
lending.withdraw(&lender, &symbol_short!("USDC"), &b_tokens)?;
```

### Pool CETES (RWA lending — para yield aggregator)

```rust
// CETES as a lending asset (RWA oracle)
lending.set_token_contract(&symbol_short!("CETES"), &cetes_token, &AssetType::Rwa);
lending.set_interest_rate_params(&symbol_short!("CETES"), &InterestRateParams {
    target_util: 6_000_000,  // 60% — RWA pools typically have lower utilization
    max_util: 8_000_000,     // 80%
    r_base: 200_000,         // 2% base (RWA tiene premium de riesgo)
    r_one: 800_000,          r_two: 6_000_000,
    r_three: 20_000_000,     reactivity: 200,
});

// USDC as collateral to borrow CETES (Reflector oracle)
lending.set_collateral_factor(&usdc_token, &8_500_000, &AssetType::Crypto, &symbol_short!("USDC"));

// Lender (or yield aggregator vault) deposits CETES → receives bCETES
lending.deposit(&vault_adapter, &symbol_short!("CETES"), &50000_0000000)?;

// Borrower pone USDC → pide CETES prestado (e.g. para short o repo)
lending.add_collateral(&borrower, &usdc_token, &10000_0000000)?;
lending.borrow(&borrower, &symbol_short!("CETES"), &5000_0000000)?;
```

## Constants

| Constant                         | Value             | Description                                             |
| -------------------------------- | ----------------- | ------------------------------------------------------- |
| `SCALAR_7`                       | 10,000,000        | 7 decimal precision (rates, utilization, health factor) |
| `SCALAR_12`                      | 1,000,000,000,000 | 12 decimal precision (token rates)                      |
| `SECONDS_PER_YEAR`               | 31,536,000        | Interest calculation                                    |
| `AUCTION_DURATION_BLOCKS`        | 200               | ~17 minutes                                             |
| `BACKSTOP_WITHDRAWAL_QUEUE_DAYS` | 17                | Withdrawal queue                                        |
| `MIN_HEALTH_FACTOR`              | 11,000,000        | 1.1 (7 decimals)                                        |
| `MAX_HEALTH_FACTOR`              | 11,500,000        | 1.15 (7 decimals)                                       |
| `ONE_DAY_LEDGERS`                | 17,280            | ~5 sec/ledger                                           |
| `INSTANCE_TTL`                   | 518,400           | 30 days in ledgers                                      |
| `USER_TTL`                       | 1,728,000         | 100 days in ledgers                                     |

## Oracle Integration

The contract uses two oracles and routes automatically based on the `AssetType`:

| Oracle | Assets | When to use |
|--------|--------|---------------|
| **RWA Oracle** (`rwa-oracle`) | CETES, USDY, NVDA, TSLA, AAPL | `AssetType::Rwa` |
| **Reflector Oracle** | USDC, XLM, crypto | `AssetType::Crypto` |

Las funciones de routing en `oracles.rs`:
- `get_price_for_lending_asset(env, symbol)` — to value debt when borrowing
- `get_price_for_collateral(env, token_address)` — to value collateral in borrow limit and liquidations

```rust
pub mod rwa_oracle {
    soroban_sdk::contractimport!(file = "../target/wasm32v1-none/release/rwa_oracle.wasm");
}
```

## Testing

```bash
cargo test -p rwa-lending
```

17 tests covering:

- Initialization and admin functions
- Token rates and supplies
- Pool state management
- Collateral factors
- Bad debt auction edge cases
- Interest auction edge cases

## Building

```bash
# Build oracle WASM first (required for lending import)
cargo build --target wasm32v1-none --release -p rwa-oracle

# Build lending WASM
cargo build --target wasm32v1-none --release -p rwa-lending

# Output: target/wasm32v1-none/release/rwa_lending.wasm
```

## Related Contracts

| Contract                    | Description                      | Relationship                                     |
| --------------------------- | -------------------------------- | ------------------------------------------------ |
| [rwa-oracle](../rwa-oracle) | SEP-40 price feed + RWA metadata | Provides RWA asset prices                        |
| [rwa-token](../rwa-token)   | SEP-41 + SEP-57 regulated token  | Used as collateral **and** as lending asset      |
| rwa-perps                   | Perpetual futures                | Shares oracle infrastructure                     |

## License

MIT

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
