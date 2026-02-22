<h1 align="center">Adapter: Blend Protocol</h1>

<p align="center">
  <strong>Part of the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> DeFi ecosystem on Stellar</strong>
</p>

A bridge adapter connecting the **rwa-vault** yield aggregator to an existing **Blend Protocol** lending pool on Stellar Soroban. This contract implements the `IAdapter` interface consumed by rwa-vault, translating generic `a_deposit` / `a_withdraw` calls into Blend's `pool.submit()` API while handling the cross-contract auth required for the token transfers involved. BLND emission rewards are claimable via `a_harvest`.

## Neko Protocol Integration

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
│                        │ a_harvest(to)                           │
│                        ▼                                        │
│   ┌──────────────────────────────────────────────────┐         │
│   │              adapter-blend                        │         │
│   │                                                   │         │
│   │  authorize_as_current_contract(                   │         │
│   │    token.transfer(adapter → blend_pool)           │         │
│   │  )                                                │         │
│   └────────────────────┬─────────────────────────────┘         │
│                        │ pool.submit() / pool.claim()            │
│                        ▼                                        │
│   ┌──────────────────────────────────────────────────┐         │
│   │           Blend Protocol Pool                     │         │
│   │  submit(from=adapter, spender=adapter, to=*)      │         │
│   │  claim(from=adapter, claim_ids, to=adapter)       │         │
│   │                                                   │         │
│   │  bTokens held by adapter ← vault's position       │         │
│   │  BLND emissions forwarded to vault on harvest     │         │
│   └──────────────────────────────────────────────────┘         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- **a_deposit**: Supplies tokens to Blend via `pool.submit([Supply])`, holding bTokens as the position
- **a_withdraw**: Redeems bTokens via `pool.submit([Withdraw])`, sending underlying directly to the vault
- **a_balance**: Returns the adapter's current position in deposit_token units (`b_tokens × b_rate / SCALAR_12`)
- **a_get_apy**: Returns 0 — yield is embedded in b_rate appreciation (no explicit on-chain APY)
- **a_harvest**: Claims BLND liquidity mining rewards via `pool.claim()` and forwards them to the vault

## Standards & Architecture

| Component               | Description                                        | Implementation                                                   |
| ----------------------- | -------------------------------------------------- | ---------------------------------------------------------------- |
| **IAdapter interface**  | Polymorphic vault adapter contract                 | Implements `a_deposit`, `a_withdraw`, `a_balance`, `a_harvest`   |
| **contractimport!**     | Compile-time Blend pool WASM import                | `blend::PoolClient` generated from pool.wasm at compile time     |
| **Cross-contract auth** | Pre-authorize sub-contract token transfers         | `env.authorize_as_current_contract` with `InvokerContractAuthEntry` |
| **bToken accounting**   | Position tracked via Blend bTokens                 | `balance = b_tokens × b_rate / SCALAR_12`                        |
| **BLND emissions**      | Liquidity mining rewards via `pool.claim()`        | `claim_id = reserve_id * 2 + 1` for bToken supply emissions      |

## Features

- **Consumes existing pools**: Each adapter instance connects to a pre-deployed Blend pool — no pool creation logic
- **Auto-resolved reserve**: On initialization, queries `pool.get_reserve(deposit_token)` to resolve `reserve_id` and `claim_ids` automatically
- **bToken position tracking**: Adapter holds bTokens in the Blend pool; their value in underlying grows as interest accrues
- **Cross-contract auth**: Uses `env.authorize_as_current_contract` to pre-authorize the `token.transfer` that Blend executes internally during deposit
- **Round-up b_token calculation**: Withdraw converts underlying amount to b_tokens with ceiling division to avoid dust
- **BLND harvest**: Claims BLND emissions to the adapter then forwards them to the vault via `token.transfer`

## Project Structure

```
src/
├── lib.rs              # Crate root: module declarations + contractimport!(pool.wasm)
├── contract.rs         # #[contract] BlendAdapter — IAdapter implementation
├── admin/
│   └── mod.rs          # Admin::initialize — queries pool to resolve reserve_id/claim_ids
├── blend_pool/
│   └── mod.rs          # supply, withdraw, claim, position_value — Blend API wrappers
├── common/
│   ├── mod.rs
│   ├── error.rs        # Error enum (8 variants)
│   ├── events.rs       # Event types: Deposited, Withdrawn, Harvested
│   ├── storage.rs      # AdapterStorage load/save with TTL
│   └── types.rs        # AdapterStorage, SCALAR_12, REQUEST_SUPPLY, REQUEST_WITHDRAW
└── test/
    └── mod.rs          # Tests (5 tests) using real Blend WASMs
```

## Core Concepts

### Cross-Contract Auth Pattern (Deposit)

Blend's `pool.submit([Supply])` calls `token.transfer(lender, pool, amount)` internally. Since the adapter is the lender, it must pre-authorize this transfer before calling `pool.submit()`:

```
vault → token.transfer(vault, adapter, amount)     [vault self-auth]
vault → adapter.a_deposit(amount, vault)
  adapter → authorize_as_current_contract([
    token.transfer(adapter, blend_pool, amount)
  ])
  adapter → pool.submit(adapter, adapter, adapter, [Supply(token, amount)])
    blend: lender.require_auth()                   [adapter is invoker → PASS]
    blend: token.transfer(adapter, pool, amount)   [pre-authorized → PASS]
    blend: mints bTokens to adapter's position
  adapter → returns balance_in_underlying to vault
```

### Cross-Contract Auth Pattern (Withdraw)

Blend's withdraw sends tokens directly to the `to` address — no pre-auth needed from the adapter side:

```
vault → adapter.a_withdraw(amount, vault_addr)
  adapter → pool.submit(adapter, adapter, vault, [Withdraw(token, actual_amount)])
    blend: withdraws underlying and transfers directly to vault
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

The result is capped at the adapter's actual bToken balance to prevent over-withdrawal.

### AdapterStorage (instance storage)

```rust
pub struct AdapterStorage {
    pub vault: Address,         // Authorized vault address
    pub blend_pool: Address,    // Blend pool contract address
    pub deposit_token: Address, // Underlying token (e.g. USDC)
    pub blend_token: Address,   // BLND reward token address
    pub reserve_id: u32,        // Auto-resolved from pool.get_reserve()
    pub claim_ids: Vec<u32>,    // [reserve_id * 2 + 1] for bToken emissions
    pub admin: Address,
}
```

### BLND Emissions (Harvest)

Blend distributes BLND tokens as liquidity mining rewards to lenders. The `claim_id` for bToken supply emissions is `reserve_id * 2 + 1`. On harvest:

1. `pool.claim(adapter, claim_ids, adapter)` — claims BLND to the adapter
2. `blnd_token.transfer(adapter, vault, amount)` — forwards BLND to the vault

The vault accumulates BLND in its `liquid_reserve`. Swapping BLND → deposit_token is left to the vault manager.

## Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    vault: Address,         // rwa-vault address
    blend_pool: Address,    // Existing Blend pool contract address
    deposit_token: Address, // Underlying token contract (e.g. USDC)
    blend_token: Address,   // BLND token contract address
)
```

`reserve_id` and `claim_ids` are resolved automatically by querying `pool.get_reserve(deposit_token)` — no manual configuration needed.

## Key Functions

### IAdapter Interface

```rust
// Deposit tokens into Blend pool.
// Pre-condition: vault has already transferred `amount` tokens to this adapter.
// Returns the adapter's current balance in deposit_token units after deposit.
let balance: i128 = adapter.a_deposit(&amount, &vault_address);

// Withdraw tokens from Blend pool and transfer directly to `to` (the vault).
// Returns the actual amount withdrawn in deposit_token units.
let received: i128 = adapter.a_withdraw(&amount, &vault_address);

// Returns the adapter's current position value in deposit_token units.
// value = b_tokens * b_rate / SCALAR_12
let balance: i128 = adapter.a_balance(&vault_address);

// Returns APY in BPS (returns 0 — yield is reflected in b_rate appreciation).
let apy: u32 = adapter.a_get_apy();

// Claim BLND emissions and forward to `to` (the vault).
// Returns the BLND amount harvested.
let harvested: i128 = adapter.a_harvest(&vault_address);
```

### View Functions

```rust
let vault: Address     = adapter.get_vault();
let pool: Address      = adapter.get_blend_pool();
let reserve: u32       = adapter.get_reserve_id();
```

## Blend Pool WASM Import

The adapter imports the Blend pool WASM at compile time, generating the `blend::PoolClient`:

```rust
pub mod blend {
    soroban_sdk::contractimport!(file = "../external_wasms/blend/pool.wasm");
    pub type PoolClient<'a> = Client<'a>;
}
```

The path is relative to `Cargo.toml`. The WASM must be present in `external_wasms/blend/` before building.

## Constants

| Constant           | Value             | Description                                      |
| ------------------ | ----------------- | ------------------------------------------------ |
| `SCALAR_12`        | 1,000,000,000,000 | b_rate decimal precision (12 decimals)           |
| `REQUEST_SUPPLY`   | 0                 | Blend request type for supply                    |
| `REQUEST_WITHDRAW` | 1                 | Blend request type for withdraw                  |

## Error Codes

| Code | Name                  | Description                                    |
| ---- | --------------------- | ---------------------------------------------- |
| 1    | `AlreadyInitialized`  | Contract already initialized                   |
| 2    | `NotInitialized`      | Contract not initialized                       |
| 3    | `NotVault`            | Caller is not the registered vault             |
| 4    | `NotAdmin`            | Caller is not the admin                        |
| 5    | `ZeroAmount`          | Amount must be positive                        |
| 6    | `ArithmeticError`     | Overflow or division by zero                   |
| 7    | `InsufficientBalance` | Adapter has no bTokens to withdraw             |
| 8    | `InvalidReserve`      | deposit_token not found as a reserve in the pool |

## Events

| Event       | Fields                                              |
| ----------- | --------------------------------------------------- |
| `Deposited` | `adapter: Address, asset: Address, amount: i128`    |
| `Withdrawn` | `adapter: Address, asset: Address, amount: i128`    |
| `Harvested` | `adapter: Address, blend_token: Address, blnd_amount: i128` |

## Usage Example

### Connecting rwa-vault to a USDC Blend pool

```rust
// 1. Deploy adapter
adapter.initialize(
    &admin,
    &vault_contract,   // rwa-vault address
    &blend_pool,       // existing Blend pool address
    &usdc_token,       // USDC token contract
    &blnd_token,       // BLND token contract
);

// 2. Register adapter in vault
vault.add_protocol(
    &symbol_short!("BLEND"),
    &adapter_contract,
    &8000u32,          // 80% target allocation
    &RiskTier::Medium,
);

// 3. Vault deposit flow (called internally by vault on rebalance)
//    vault → usdc.transfer(vault, adapter, amount)
//    vault → adapter.a_deposit(amount, vault)
//      → adapter pre-authorizes usdc.transfer(adapter, blend_pool, amount)
//      → pool.submit(adapter, adapter, adapter, [Supply(usdc, amount)])
//      → returns balance in underlying (bTokens × b_rate / SCALAR_12)

// 4. Vault withdraw flow
//    vault → adapter.a_withdraw(amount, vault)
//      → pool.submit(adapter, adapter, vault, [Withdraw(usdc, actual)])
//      → Blend sends USDC directly to vault
//      → returns actual amount

// 5. Harvest BLND rewards
//    vault → adapter.a_harvest(vault)
//      → pool.claim(adapter, [claim_id], adapter)
//      → blnd.transfer(adapter, vault, blnd_amount)
//      → returns blnd_amount
```

## Testing

The tests use real Blend Protocol WASMs from `external_wasms/blend/`. No mocking of the Blend contracts — the full on-chain initialization sequence is replicated in the test fixture.

```bash
cargo test -p adapter-blend
```

5 tests covering:

- Initialization and auto-resolution of `reserve_id`
- Balance starts at zero before any deposit
- Deposit creates a position with positive balance
- Withdraw transfers tokens back to vault
- APY placeholder returns 0

## Building

```bash
# adapter-blend only depends on soroban-sdk — no local WASM build order needed
cargo build --target wasm32v1-none --release -p adapter-blend

# Output: target/wasm32v1-none/release/adapter_blend.wasm
```

## Related Contracts

| Contract                                          | Description                        | Relationship                                           |
| ------------------------------------------------- | ---------------------------------- | ------------------------------------------------------ |
| [rwa-vault](../rwa-vault)                         | Yield aggregator vault             | Calls this adapter via IAdapter interface              |
| [adapter-rwa-lending](../adapter-rwa-lending)     | RWA Lending adapter                | Sibling adapter — same IAdapter interface              |
| [external_wasms/blend](../external_wasms/blend)   | Blend Protocol WASMs               | pool.wasm imported at compile time; all 5 used in tests |

## License

MIT

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
