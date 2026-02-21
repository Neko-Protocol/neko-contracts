<h1 align="center">RWA Vault Contract</h1>

<p align="center">
  <strong>Part of the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> DeFi ecosystem on Stellar</strong>
</p>

A yield aggregator vault for Real-World Assets (RWAs) on Stellar Soroban. This contract accepts a single deposit token (e.g. CETES), automatically distributes it across multiple lending protocols to maximize yield, and issues **vTokens** (SEP-41) representing proportional ownership of the vault's NAV.

## Neko Protocol Integration

This vault is the yield optimization layer of Neko Protocol:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Neko Protocol                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────────┐    prices    ┌──────────────┐               │
│   │  RWA Oracle  │─────────────▶│  RWA Token   │               │
│   │   (SEP-40)   │              │   (SEP-41)   │               │
│   └──────────────┘              └──────┬───────┘               │
│                                        │ deposit_token          │
│                                        ▼                        │
│   ┌────────────────────────────────────────────┐               │
│   │               RWA Vault                     │               │
│   │  ┌──────────┐  ┌─────────┐  ┌───────────┐  │               │
│   │  │ vTokens  │  │   NAV   │  │ Optimizer │  │               │
│   │  │ (SEP-41) │  │ (share  │  │(rebalance)│  │               │
│   │  │          │  │  price) │  │           │  │               │
│   │  └──────────┘  └─────────┘  └───────────┘  │               │
│   └────────────┬───────────────────────────────┘               │
│                │ adapter calls                                   │
│                ▼                                                │
│   ┌────────────────────────────────────────────┐               │
│   │         adapter-rwa-lending                 │               │
│   └────────────────────┬───────────────────────┘               │
│                        │ deposit / withdraw                      │
│                        ▼                                        │
│   ┌────────────────────────────────────────────┐               │
│   │              RWA Lending                    │               │
│   │  (bTokens represent vault's lending share)  │               │
│   └────────────────────────────────────────────┘               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- **Deposit**: Users deposit RWA tokens and receive vTokens at the current NAV share price
- **Withdraw**: Users burn vTokens and receive the proportional underlying token amount
- **Rebalance**: Manager redistributes capital across protocols according to target allocations
- **Harvest**: Manager collects yield from all protocols; management and performance fees are accrued
- **SEP-41**: vTokens are fully transferable, approvable, and composable with the Stellar ecosystem

## Standards & Architecture

| Component                       | Description                                     | Implementation                                         |
| ------------------------------- | ----------------------------------------------- | ------------------------------------------------------ |
| **SEP-41 vToken**               | Vault shares as a token interface               | Internal persistent storage, same pattern as rwa-token |
| **NAV-based share pricing**     | Shares priced by Net Asset Value                | `NAV = liquid_reserve + Σ adapter.a_balance()`        |
| **Adapter pattern**             | Protocol-agnostic yield routing                 | `#[contractclient] trait IAdapter` — polymorphic       |
| **BPS fee model**               | Management + performance fees in basis points   | Accrued as newly minted shares to admin                |
| **High Water Mark**             | Performance fee only on new gains               | `share_price > hwm` before applying performance fee   |

## Features

- **vTokens (SEP-41)**: Vault shares fully implement the SEP-41 token interface — transfer, approve, transfer_from, burn, burn_from
- **NAV Accounting**: Net Asset Value = liquid reserve + sum of all adapter balances (in deposit_token units)
- **1:1 First Deposit**: First depositor always receives shares 1:1; subsequent deposits are NAV-proportional
- **Protocol Allocation**: Up to 10 protocols registered with per-protocol target BPS and risk tier
- **Risk Tiers**: Low / Medium / High — optimizer applies APY discount multipliers (100% / 85% / 70%)
- **Rebalancer**: Manager-triggered; only rebalances if allocation drift exceeds `rebalance_threshold_bps`
- **Harvester**: Calls `a_harvest()` on each protocol; accrues management fee and performance fee
- **Management Fee**: Annual fee accrued continuously as newly minted shares (`nav * fee_bps * elapsed / BPS / SECONDS_PER_YEAR`)
- **Performance Fee**: Applied on gains above high water mark; paid as shares to admin
- **Vault States**: Active → deposits allowed; Paused → withdrawals only; EmergencyExit → emergency-only
- **TTL Management**: Automatic storage TTL extension for instance and persistent data

## Project Structure

```
src/
├── lib.rs                      # Crate root, module declarations
├── contract.rs                 # #[contract] VaultContract entry point (SEP-41 + vault interface)
├── admin/
│   └── mod.rs                  # Admin struct (initialize, setters, protocol management, upgrade)
├── common/
│   ├── mod.rs
│   ├── error.rs                # Error enum (18 variants)
│   ├── events.rs               # Event types (9 events)
│   ├── storage.rs              # Storage helpers with TTL management
│   └── types.rs                # VaultStorage, VaultConfig, ProtocolAllocation, DataKey, etc.
├── vault/
│   ├── mod.rs
│   ├── deposit.rs              # Deposit logic: NAV → shares → mint
│   ├── withdraw.rs             # Withdraw logic: shares → NAV → token transfer
│   ├── nav.rs                  # NAV calculation, share price, shares↔amount conversion
│   └── shares.rs               # SEP-41 vToken balance/allowance storage operations
├── strategies/
│   ├── mod.rs
│   ├── optimizer.rs            # APY ranking with risk-tier multipliers, weighted APY
│   ├── rebalancer.rs           # Allocation rebalancing (withdraw excess, deploy deficit)
│   └── harvester.rs            # Harvest rewards, accrue fees
├── adapters/
│   └── mod.rs                  # #[contractclient] IAdapter trait definition
└── test/
    └── mod.rs                  # Tests (12 tests) + inline MockAdapter
```

## Core Concepts

### Decimal Precision

| Type                  | Decimals | Constant            | Example                          |
| --------------------- | -------- | ------------------- | -------------------------------- |
| Share price / fees    | 7        | `SCALAR_7 = 10^7`   | Share price 1.0 = 10,000,000     |
| b_rate conversions    | 12       | `SCALAR_12 = 10^12` | b_rate 1:1 = 1,000,000,000,000   |
| Allocation / fee BPS  | —        | `BPS = 10,000`      | 50% = 5,000; 0.5% = 50           |

### Net Asset Value (NAV)

NAV represents the total value of all assets managed by the vault, expressed in deposit_token units:

```
NAV = liquid_reserve + Σ adapter.a_balance(vault_address)
```

### Share Price

```
share_price = NAV × SCALAR_7 / total_shares
```

On first deposit (total_shares = 0), the share price is 1.0 (SCALAR_7 = 10,000,000) and shares are minted 1:1.

### Deposit / Withdraw

```
shares_minted = amount × total_shares / NAV       (NAV-proportional)
amount_out    = shares × NAV / total_shares
```

First deposit: `shares = amount` (1:1, no existing shares).

### VaultStorage (instance storage)

```rust
pub struct VaultStorage {
    pub status: VaultStatus,                              // Active / Paused / EmergencyExit
    pub deposit_token: Address,                           // e.g. CETES token
    pub token_name: String,                               // e.g. "Neko CETES Vault"
    pub token_symbol: String,                             // e.g. "vCETES"
    pub token_decimals: u32,                              // e.g. 7
    pub liquid_reserve: i128,                             // tokens held by vault (not deployed)
    pub total_shares: i128,                               // total vTokens minted
    pub high_water_mark: i128,                            // highest-ever share price (SCALAR_7)
    pub last_fee_accrual: u64,                            // timestamp for management fee
    pub protocol_ids: Vec<Symbol>,                        // ordered protocol ID list
    pub protocol_allocations: Map<Symbol, ProtocolAllocation>,
    pub config: VaultConfig,
    pub admin: Address,
    pub manager: Address,
}
```

### VaultConfig

```rust
pub struct VaultConfig {
    pub management_fee_bps: u32,      // Annual management fee, e.g. 50 = 0.5%
    pub performance_fee_bps: u32,     // Performance fee on gains above HWM, e.g. 1000 = 10%
    pub min_liquidity_bps: u32,       // Minimum liquid reserve, e.g. 500 = 5%
    pub max_protocol_bps: u32,        // Max allocation per protocol, e.g. 9000 = 90%
    pub rebalance_threshold_bps: u32, // Min drift to trigger rebalance, e.g. 200 = 2%
}
```

### SEP-41 vToken Storage (persistent storage)

vToken balances and allowances are stored in persistent storage with automatic TTL extension:

```rust
pub enum DataKey {
    Balance(Address),
    Allowance(Txn),
}

pub struct Txn(pub Address, pub Address);  // (owner, spender)

pub struct VaultAllowance {
    pub amount: i128,
    pub live_until_ledger: u32,
}
```

### IAdapter — Polymorphic Protocol Interface

All yield protocols are accessed through a single trait. The vault never imports any concrete protocol:

```rust
#[contractclient(name = "AdapterClient")]
pub trait IAdapter {
    fn a_deposit(env: Env, amount: i128, from: Address) -> i128;   // returns balance after deposit
    fn a_withdraw(env: Env, amount: i128, to: Address) -> i128;    // returns amount withdrawn
    fn a_balance(env: Env, from: Address) -> i128;                 // returns current value in deposit_token
    fn a_get_apy(env: Env) -> u32;                                 // APY in BPS
    fn a_harvest(env: Env, to: Address) -> i128;                   // returns harvested rewards
}
```

### RiskTier — APY Optimizer Weighting

```rust
pub enum RiskTier {
    Low,    // APY multiplier: 1.00x — e.g. CETES
    Medium, // APY multiplier: 0.85x — e.g. USDY
    High,   // APY multiplier: 0.70x — e.g. experimental pools
}
```

The optimizer ranks protocols by `adjusted_apy = raw_apy × risk_multiplier` and allocates to the highest-adjusted protocols first, subject to `max_protocol_bps`.

## Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    manager: Address,        // Can trigger rebalance and harvest
    deposit_token: Address,  // RWA token deposited by users (e.g. CETES)
    token_name: String,      // vToken name, e.g. "Neko CETES Vault"
    token_symbol: String,    // vToken symbol, e.g. "vCETES"
    token_decimals: u32,     // vToken decimals (matches deposit_token)
    config: VaultConfig,     // Fees, liquidity reserve, rebalance threshold
)
```

## Key Functions

### Admin Functions

```rust
// Upgrade contract WASM
vault.upgrade(&new_wasm_hash);

// Pause the vault (disables deposits, withdrawals still allowed)
vault.pause();
vault.unpause();

// Emergency exit (disables all operations)
vault.emergency_exit();

// Update fee config and thresholds
vault.set_config(&VaultConfig {
    management_fee_bps: 50,
    performance_fee_bps: 1000,
    min_liquidity_bps: 500,
    max_protocol_bps: 9000,
    rebalance_threshold_bps: 200,
});

// Change manager
vault.set_manager(&new_manager);
```

### Protocol Management (Admin)

```rust
// Register a lending protocol with a target allocation
vault.add_protocol(
    &symbol_short!("POOL1"),   // unique protocol ID
    &adapter_address,          // adapter-rwa-lending contract
    &5000u32,                  // target 50% of NAV
    &RiskTier::Low,
);

// Disable a specific protocol
vault.set_protocol_active(&symbol_short!("POOL1"), &false);

// Remove a protocol entirely
vault.remove_protocol(&symbol_short!("POOL1"));
```

### User Functions

```rust
// Deposit RWA tokens and receive vTokens
let shares: i128 = vault.deposit(&user, &1000_0000000i128)?;

// Burn vTokens and receive RWA tokens
let received: i128 = vault.withdraw(&user, &shares)?;
```

### Manager Functions

```rust
// Rebalance capital across protocols
// Only executes if allocation drift > rebalance_threshold_bps
vault.rebalance()?;

// Harvest yield from all protocols and accrue fees
let total_harvested: i128 = vault.harvest_all()?;
```

### View Functions

```rust
let nav: i128           = vault.get_nav();           // Total NAV in deposit_token units
let price: i128         = vault.get_share_price();   // SCALAR_7 precision
let shares: i128        = vault.get_total_shares();  // Total vTokens minted
let reserve: i128       = vault.get_liquid_reserve();
let status: VaultStatus = vault.get_status();
let config: VaultConfig = vault.get_config();
let apy: u32            = vault.get_weighted_apy();  // Balance-weighted APY in BPS
let protocols           = vault.get_protocols();     // Vec<(Symbol, ProtocolAllocation)>
```

### SEP-41 Functions

```rust
// Transfer vTokens
vault.transfer(&from, &to, &amount);

// Approve a spender
vault.approve(&from, &spender, &amount, &live_until_ledger);
let allowance: i128 = vault.allowance(&from, &spender);

// Delegated transfer
vault.transfer_from(&spender, &from, &to, &amount);

// Burn vTokens directly
vault.burn(&from, &amount);
vault.burn_from(&spender, &from, &amount);

// Token metadata
let decimals: u32   = vault.decimals();
let name: String    = vault.name();
let symbol: String  = vault.symbol();
```

## Fee Model

### Management Fee

Accrued continuously as newly minted shares to the admin address:

```
elapsed = current_timestamp - last_fee_accrual
fee_shares = nav * management_fee_bps * elapsed / BPS / SECONDS_PER_YEAR / share_price
```

Minting fee shares dilutes existing holders proportionally, equivalent to an annual `management_fee_bps / BPS` drag on yield.

### Performance Fee

Applied whenever `share_price > high_water_mark` after a harvest:

```
gain_per_share = share_price - high_water_mark
fee_value = total_shares * gain_per_share * performance_fee_bps / SCALAR_7 / BPS
fee_shares = fee_value * total_shares / NAV
```

After applying the fee, `high_water_mark` is updated to the new share price.

## Vault States

| State           | Deposits | Withdrawals | Rebalance/Harvest |
| --------------- | -------- | ----------- | ----------------- |
| `Active`        | ✅        | ✅           | ✅                |
| `Paused`        | ❌        | ✅           | ❌                |
| `EmergencyExit` | ❌        | ✅           | ❌                |

## TTL Management

| Storage Type | Min TTL | Bump TTL | Use                     |
| ------------ | ------- | -------- | ----------------------- |
| Instance     | 30 days | 31 days  | VaultStorage, config    |
| Persistent   | max_ttl | max_ttl  | vToken balances/allowances |

## Error Codes

| Code | Name                     | Description                                         |
| ---- | ------------------------ | --------------------------------------------------- |
| 1    | `AlreadyInitialized`     | Contract already initialized                        |
| 2    | `NotInitialized`         | Contract not initialized                            |
| 3    | `NotAdmin`               | Caller is not the admin                             |
| 4    | `NotManager`             | Caller is not the manager                           |
| 5    | `VaultPaused`            | Vault is paused (no deposits)                       |
| 6    | `VaultNotActive`         | Vault is not in Active state                        |
| 7    | `ZeroAmount`             | Amount must be positive                             |
| 8    | `InsufficientShares`     | User has fewer shares than requested                |
| 9    | `InsufficientLiquidity`  | Not enough liquid reserve to cover withdrawal       |
| 10   | `ProtocolNotFound`       | Protocol ID not registered                         |
| 11   | `ProtocolAlreadyExists`  | Protocol ID already registered                     |
| 12   | `MaxProtocolsReached`    | Cannot add more than MAX_PROTOCOLS (10)             |
| 13   | `ArithmeticError`        | Overflow or division error                          |
| 14   | `InvalidConfig`          | BPS values out of range or inconsistent             |
| 15   | `InsufficientAllowance`  | Spender allowance too low                           |
| 16   | `InsufficientBalance`    | Account vToken balance too low                      |
| 17   | `InvalidLedgerInput`     | live_until_ledger in the past                       |
| 18   | `RebalanceThresholdNotMet` | Allocation drift below threshold                 |

## Events

| Event           | Topic                          | Data                              |
| --------------- | ------------------------------ | --------------------------------- |
| `deposit`       | `("vault", "deposit")`         | `(from, amount, shares)`          |
| `withdraw`      | `("vault", "withdraw")`        | `(to, amount, shares)`            |
| `rebalance`     | `("vault", "rebalance")`       | `nav`                             |
| `harvest`       | `("vault", "harvest")`         | `total_harvested`                 |
| `proto_add`     | `("vault", "proto_add")`       | `(id, adapter)`                   |
| `proto_rm`      | `("vault", "proto_rm")`        | `id`                              |
| `transfer`      | `("transfer",)`                | `(from, to, amount)`              |
| `approve`       | `("approve",)`                 | `(from, spender, amount, ledger)` |
| `burn`          | `("burn",)`                    | `(from, amount)`                  |

## Usage Example

### Vault CETES — yield aggregator sobre rwa-lending

```rust
// Initialize vault
vault.initialize(
    &admin,
    &manager,
    &cetes_token,
    &String::from_str(&env, "Neko CETES Vault"),
    &String::from_str(&env, "vCETES"),
    &7u32,
    &VaultConfig {
        management_fee_bps: 50,        // 0.5% annual
        performance_fee_bps: 1000,     // 10% on gains above HWM
        min_liquidity_bps: 500,        // 5% always liquid
        max_protocol_bps: 9000,        // max 90% per protocol
        rebalance_threshold_bps: 200,  // rebalance if >2% drift
    },
);

// Register rwa-lending via adapter
vault.add_protocol(
    &symbol_short!("CLEND"),
    &adapter_contract,
    &8000u32,       // target 80% of NAV
    &RiskTier::Low,
);

// User deposits CETES → receives vCETES
let shares = vault.deposit(&user, &10000_0000000i128)?;

// Manager rebalances and harvests
vault.rebalance()?;
vault.harvest_all()?;

// User burns vCETES → receives CETES + yield
let received = vault.withdraw(&user, &shares)?;

// Transfer vCETES to another user
vault.transfer(&user, &user2, &5000_0000000i128);
```

## Constants

| Constant           | Value             | Description                              |
| ------------------ | ----------------- | ---------------------------------------- |
| `SCALAR_7`         | 10,000,000        | 7 decimal precision (share price, fees)  |
| `SCALAR_12`        | 1,000,000,000,000 | 12 decimal precision (b_rate conversion) |
| `BPS`              | 10,000            | 100% in basis points                     |
| `SECONDS_PER_YEAR` | 31,536,000        | Management fee accrual                   |
| `MAX_PROTOCOLS`    | 10                | Maximum registered protocols             |
| `ONE_DAY_LEDGERS`  | 17,280            | ~5 sec/ledger                            |
| `INSTANCE_TTL`     | 518,400           | 30 days in ledgers                       |

## Testing

```bash
cargo test -p rwa-vault
```

12 tests covering:

- Initialization and double-initialize guard
- First deposit (1:1 shares)
- Proportional shares on subsequent deposits
- Withdraw from liquid reserve (partial and full)
- Add and remove protocols
- Pause/unpause state transitions
- SEP-41 transfer, approve, transfer_from
- Token metadata (decimals, name, symbol)

## Building

```bash
# Build vault WASM
cargo build --target wasm32v1-none --release -p rwa-vault

# Output: target/wasm32v1-none/release/rwa_vault.wasm
```

## Related Contracts

| Contract                                          | Description                     | Relationship                                     |
| ------------------------------------------------- | ------------------------------- | ------------------------------------------------ |
| [adapter-rwa-lending](../adapter-rwa-lending)     | Adapter for rwa-lending pool    | Called by vault to deposit/withdraw RWA tokens   |
| [rwa-lending](../rwa-lending)                     | Lending and borrowing protocol  | Receives capital from adapter, returns bTokens   |
| [rwa-token](../rwa-token)                         | SEP-41 + SEP-57 regulated token | Used as the vault's deposit_token                |
| [rwa-oracle](../rwa-oracle)                       | SEP-40 price feed + RWA metadata | Pricing used by rwa-lending                     |

## License

Apache-2.0

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
