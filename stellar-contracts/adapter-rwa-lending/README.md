<h1 align="center">Adapter: RWA Lending</h1>

<p align="center">
  <strong>Part of the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> DeFi ecosystem on Stellar</strong>
</p>

A bridge adapter connecting the **rwa-vault** yield aggregator to the **rwa-lending** pool on Stellar Soroban. This contract implements the `IAdapter` interface consumed by rwa-vault, translating generic `a_deposit` / `a_withdraw` calls into concrete rwa-lending `deposit` / `withdraw` calls while handling the cross-contract auth required for the token transfers involved.

## Neko Protocol Integration

This adapter is the intermediary layer between the vault and the lending pool:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Neko Protocol                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────────────────────────────────────────────┐         │
│   │                   RWA Vault                       │         │
│   │           (rwa-vault, NAV accounting)             │         │
│   └────────────────────┬─────────────────────────────┘         │
│                        │ IAdapter calls                          │
│                        │ a_deposit(amount, from)                 │
│                        │ a_withdraw(amount, to)                  │
│                        │ a_balance(from)                         │
│                        ▼                                        │
│   ┌──────────────────────────────────────────────────┐         │
│   │           adapter-rwa-lending                     │         │
│   │                                                   │         │
│   │  authorize_as_current_contract(                   │         │
│   │    token.transfer(adapter ↔ lending_pool)         │         │
│   │  )                                                │         │
│   └────────────────────┬─────────────────────────────┘         │
│                        │ deposit / withdraw                      │
│                        ▼                                        │
│   ┌──────────────────────────────────────────────────┐         │
│   │              RWA Lending                          │         │
│   │  deposit(lender=adapter, asset, amount)           │         │
│   │  withdraw(lender=adapter, asset, b_tokens)        │         │
│   │                                                   │         │
│   │  bTokens held by adapter ← vault's position       │         │
│   └──────────────────────────────────────────────────┘         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- **a_deposit**: Forwards deposited tokens from vault into rwa-lending, holding bTokens as the position
- **a_withdraw**: Burns bTokens in rwa-lending and returns the underlying tokens to the vault
- **a_balance**: Returns the adapter's current position in deposit_token units (`b_tokens × b_rate / SCALAR_12`)
- **a_get_apy**: Placeholder for lending APY (yield is reflected in b_rate appreciation)
- **a_harvest**: No explicit harvest — yield is embedded in b_rate, realized on withdraw

## Standards & Architecture

| Component                       | Description                                    | Implementation                                            |
| ------------------------------- | ---------------------------------------------- | --------------------------------------------------------- |
| **IAdapter interface**          | Polymorphic vault adapter contract             | Implements `a_deposit`, `a_withdraw`, `a_balance`, etc.   |
| **contractimport!**             | Compile-time rwa-lending WASM import           | `rwa_lending::Client` generated from WASM at compile time |
| **Cross-contract auth**         | Pre-authorize sub-contract token transfers     | `env.authorize_as_current_contract` with `InvokerContractAuthEntry` |
| **bToken accounting**           | Position tracked via rwa-lending bTokens       | `balance = b_tokens × b_rate / SCALAR_12`                 |

## Features

- **Single-asset adapter**: Each adapter instance manages one RWA asset (e.g. CETES) in one rwa-lending pool
- **bToken position tracking**: The adapter holds bTokens in rwa-lending; their value in underlying grows as borrowers pay interest
- **Cross-contract auth**: Uses `env.authorize_as_current_contract` to pre-authorize the `token.transfer` calls that rwa-lending executes internally during deposit and withdraw
- **Round-up b_token calculation**: Withdraw converts underlying amount to b_tokens with ceiling division to avoid leaving dust in the pool
- **Vault-only access**: `a_deposit` and `a_withdraw` can only be called by the registered vault (enforced by `storage.vault.require_auth()`)
- **TTL Management**: Instance storage TTL auto-extended (30-day bump)

## Project Structure

```
src/
├── lib.rs              # Crate root: module declarations + contractimport!(rwa_lending WASM)
├── contract.rs         # #[contract] RwaLendingAdapter — IAdapter implementation
├── admin/
│   └── mod.rs          # Admin struct (initialize, require_vault, require_admin)
├── common/
│   ├── mod.rs
│   ├── error.rs        # Error enum (7 variants)
│   ├── events.rs       # Event types (2 events)
│   ├── storage.rs      # AdapterStorage load/save with TTL
│   └── types.rs        # AdapterStorage, SCALAR_12, TTL constants
└── test/
    └── mod.rs          # Tests (5 tests) using real rwa-lending + rwa-oracle WASMs
```

## Core Concepts

### Cross-Contract Auth Pattern (Deposit)

rwa-lending's `deposit()` function calls `token.transfer(lender, pool, amount)` internally. Since the adapter is the lender, it must pre-authorize this transfer before calling `lending.deposit()`:

```
vault → token.transfer(vault, adapter, amount)    [vault self-auth]
vault → adapter.a_deposit(amount, vault)
  adapter → authorize_as_current_contract([
    token.transfer(adapter, lending_pool, amount)
  ])
  adapter → lending.deposit(adapter, asset, amount)
    lending: lender.require_auth()                [adapter is invoker → PASS]
    lending: token.transfer(adapter, pool, amount)[pre-authorized → PASS]
    lending: returns b_tokens to adapter
  adapter: returns balance_in_underlying to vault
```

### Cross-Contract Auth Pattern (Withdraw)

Similarly, rwa-lending's `withdraw()` calls `token.transfer(pool, lender, amount)` internally:

```
vault → adapter.a_withdraw(amount, vault_addr)
  adapter → authorize_as_current_contract([
    token.transfer(lending_pool, adapter, underlying_out)
  ])
  adapter → lending.withdraw(adapter, asset, b_tokens)
    lending: token.transfer(pool, adapter, amount) [pre-authorized → PASS]
    lending: returns actual_amount
  adapter → token.transfer(adapter, vault, actual_amount)
  adapter → returns actual_amount to vault
```

### Balance Calculation

The adapter's position in deposit_token units is computed from the bTokens it holds and the current b_rate:

```
balance = b_tokens × b_rate / SCALAR_12
```

As borrowers pay interest into the pool, `b_rate` increases over time — the same number of bTokens becomes worth more deposit_token, capturing lending yield passively.

### bToken → Underlying Conversion (Withdraw)

To avoid dust, the b_token amount to burn is computed with ceiling division:

```
b_tokens_to_burn = ceil(amount × SCALAR_12 / b_rate)
                 = (amount × SCALAR_12 + b_rate - 1) / b_rate
```

The actual withdrawn amount may be slightly higher than requested; the excess stays in the vault's liquid reserve.

### AdapterStorage (instance storage)

```rust
pub struct AdapterStorage {
    pub vault: Address,          // Authorized vault address
    pub lending_pool: Address,   // rwa-lending contract address
    pub deposit_token: Address,  // RWA token (e.g. CETES token contract)
    pub rwa_asset: Symbol,       // Asset symbol used in rwa-lending (e.g. symbol_short!("CETES"))
    pub admin: Address,
}
```

## Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    vault: Address,         // rwa-vault address — only caller allowed for a_deposit/a_withdraw
    lending_pool: Address,  // rwa-lending contract address
    deposit_token: Address, // RWA token contract (e.g. CETES)
    rwa_asset: Symbol,      // Asset symbol in rwa-lending (e.g. symbol_short!("CETES"))
)
```

## Key Functions

### IAdapter Interface

```rust
// Deposit tokens into rwa-lending.
// Pre-condition: vault has already transferred `amount` tokens to this adapter.
// Returns the adapter's current balance in deposit_token units after deposit.
let balance: i128 = adapter.a_deposit(&amount, &vault_address)?;

// Withdraw tokens from rwa-lending and transfer to `to` (the vault).
// Returns the actual amount withdrawn in deposit_token units.
let received: i128 = adapter.a_withdraw(&amount, &vault_address)?;

// Returns the adapter's current position value in deposit_token units.
// value = b_tokens * b_rate / SCALAR_12
let balance: i128 = adapter.a_balance(&vault_address);

// Returns APY in BPS (placeholder, 0 for MVP — yield reflected in b_rate).
let apy: u32 = adapter.a_get_apy();

// Harvest rewards (0 for rwa-lending — yield is via b_rate appreciation).
let harvested: i128 = adapter.a_harvest(&vault_address);
```

### View Functions

```rust
let vault: Address        = adapter.get_vault();
let pool: Address         = adapter.get_lending_pool();
```

## rwa-lending Import

The adapter imports the rwa-lending WASM at compile time, generating the `rwa_lending::Client`:

```rust
pub mod rwa_lending {
    soroban_sdk::contractimport!(
        file = "../target/wasm32v1-none/release/rwa_lending.wasm"
    );
}
```

This means the rwa-lending WASM **must be built before building or testing the adapter**:

```bash
# Build order
cargo build --target wasm32v1-none --release -p rwa-oracle
cargo build --target wasm32v1-none --release -p rwa-lending
cargo build --target wasm32v1-none --release -p adapter-rwa-lending
```

## Constants

| Constant          | Value             | Description                                    |
| ----------------- | ----------------- | ---------------------------------------------- |
| `SCALAR_12`       | 1,000,000,000,000 | b_rate decimal precision (matches rwa-lending) |
| `ONE_DAY_LEDGERS` | 17,280            | ~5 sec/ledger                                  |
| `INSTANCE_TTL`    | 518,400           | 30 days in ledgers                             |

## Error Codes

| Code | Name                 | Description                              |
| ---- | -------------------- | ---------------------------------------- |
| 1    | `AlreadyInitialized` | Contract already initialized             |
| 2    | `NotInitialized`     | Contract not initialized                 |
| 3    | `NotVault`           | Caller is not the registered vault       |
| 4    | `NotAdmin`           | Caller is not the admin                  |
| 5    | `ZeroAmount`         | Amount must be positive                  |
| 6    | `ArithmeticError`    | Overflow or division by zero             |
| 7    | `InsufficientBalance`| Adapter has no b_tokens to withdraw      |

## Events

| Event     | Topic                       | Data                                    |
| --------- | --------------------------- | --------------------------------------- |
| `deposit` | `("adapter", "deposit")`    | `(adapter, asset, amount, b_tokens)`    |
| `withdraw`| `("adapter", "withdraw")`   | `(adapter, asset, amount_withdrawn)`    |

## Usage Example

### Connecting rwa-vault to a CETES lending pool

```rust
// 1. Deploy adapter
adapter.initialize(
    &admin,
    &vault_contract,        // rwa-vault address
    &lending_pool,          // rwa-lending contract address
    &cetes_token,           // CETES token contract
    &symbol_short!("CETES"),
);

// 2. Register adapter in vault
vault.add_protocol(
    &symbol_short!("CLEND"),
    &adapter_contract,
    &8000u32,             // 80% target allocation
    &RiskTier::Low,
);

// 3. Vault deposit flow (called internally by vault on user deposit + rebalance)
//    vault → token.transfer(vault, adapter, amount)
//    vault → adapter.a_deposit(amount, vault)
//      → adapter pre-authorizes token.transfer(adapter, pool, amount)
//      → lending.deposit(adapter, "CETES", amount) — adapter becomes lender
//      → returns balance in underlying

// 4. Vault withdraw flow (called internally by vault on user withdraw)
//    vault → adapter.a_withdraw(amount, vault)
//      → adapter pre-authorizes token.transfer(pool, adapter, amount)
//      → lending.withdraw(adapter, "CETES", b_tokens) — burns bTokens
//      → adapter.token.transfer(adapter, vault, actual) — returns to vault
//      → returns actual amount withdrawn

// 5. Balance query (used by vault for NAV calculation)
let position: i128 = adapter.a_balance(&vault_contract);
// = b_tokens × b_rate / SCALAR_12 — grows as lending interest accrues
```

## Testing

```bash
# Build dependencies first
cargo build --target wasm32v1-none --release -p rwa-oracle
cargo build --target wasm32v1-none --release -p rwa-lending

# Run adapter tests
cargo test -p adapter-rwa-lending
```

5 tests covering:

- Initialization and vault/pool address storage
- Balance starts at zero before any deposit
- Deposit updates b_token balance in rwa-lending
- Withdraw transfers tokens back to vault
- APY placeholder returns 0; harvest returns 0

## Building

```bash
# Build oracle WASM first (required by rwa-lending)
cargo build --target wasm32v1-none --release -p rwa-oracle

# Build rwa-lending WASM (required by adapter contractimport!)
cargo build --target wasm32v1-none --release -p rwa-lending

# Build adapter WASM
cargo build --target wasm32v1-none --release -p adapter-rwa-lending

# Output: target/wasm32v1-none/release/adapter_rwa_lending.wasm
```

## Related Contracts

| Contract                        | Description                      | Relationship                                              |
| ------------------------------- | -------------------------------- | --------------------------------------------------------- |
| [rwa-vault](../rwa-vault)       | Yield aggregator vault           | Calls this adapter via IAdapter interface                 |
| [rwa-lending](../rwa-lending)   | Lending and borrowing protocol   | Receives deposits; adapter holds bTokens as its position  |
| [rwa-oracle](../rwa-oracle)     | SEP-40 price feed + RWA metadata | Required by rwa-lending for collateral pricing            |
| [rwa-token](../rwa-token)       | SEP-41 + SEP-57 regulated token  | Deposit token transferred between vault, adapter, pool    |

## License

MIT

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
