<h1 align="center">Adapter: Soroswap AMM</h1>

<p align="center">
  <strong>Part of the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> DeFi ecosystem on Stellar</strong>
</p>

A bridge adapter connecting the **rwa-vault** yield aggregator to a **Soroswap** AMM liquidity pool on Stellar Soroban. This contract implements the `IAdapter` interface consumed by rwa-vault, enabling single-asset entry into a Soroswap pair: the adapter automatically swaps half the deposit into the pair token, provides liquidity, holds LP tokens as its position, and unwinds back to the deposit token on withdrawal.

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
│                        ▼                                        │
│   ┌──────────────────────────────────────────────────┐         │
│   │             adapter-soroswap                      │         │
│   │                                                   │         │
│   │  deposit:                                         │         │
│   │    swap half token_a → token_b                    │         │
│   │    add_liquidity → LP tokens held by adapter      │         │
│   │                                                   │         │
│   │  withdraw:                                        │         │
│   │    remove_liquidity → (token_a, token_b)          │         │
│   │    swap token_b → token_a → vault                 │         │
│   └────────────────────┬─────────────────────────────┘         │
│                        │ router.add/remove_liquidity             │
│                        │ router.swap_exact_tokens_for_tokens     │
│                        ▼                                        │
│   ┌──────────────────────────────────────────────────┐         │
│   │           Soroswap AMM Pair                       │         │
│   │  token_a / token_b reserves                       │         │
│   │  LP tokens represent the pool share               │         │
│   │  Trading fees accrue into reserves → grow NAV     │         │
│   └──────────────────────────────────────────────────┘         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- **a_deposit**: Swaps half of token_a to token_b, adds liquidity to the pair, holds LP tokens
- **a_withdraw**: Removes proportional liquidity, swaps token_b back to token_a, sends to vault
- **a_balance**: Returns the adapter's LP position value in token_a units (via AMM spot price)
- **a_get_apy**: Returns 0 — AMM yield (trading fees) is reflected in LP token value growth
- **a_harvest**: Returns 0 — Soroswap fees accrue into the pair reserves automatically

## Standards & Architecture

| Component               | Description                                          | Implementation                                                    |
| ----------------------- | ---------------------------------------------------- | ----------------------------------------------------------------- |
| **IAdapter interface**  | Polymorphic vault adapter contract                   | Implements `a_deposit`, `a_withdraw`, `a_balance`                 |
| **contractimport!**     | Compile-time Soroswap WASM imports                   | `router::RouterClient`, `pair::PairClient` from WASMs             |
| **Cross-contract auth** | Pre-authorize exact token transfers                  | `env.authorize_as_current_contract` with `InvokerContractAuthEntry` |
| **LP position**         | Position held as Soroswap LP tokens                  | `balance = share_a + share_b × (reserve_a / reserve_b)`          |
| **Auto pair resolution**| Pair address resolved at initialize                  | `router.router_pair_for(token_a, token_b)` called once            |

## Features

- **Single-asset entry**: Vault deposits only token_a — the adapter handles the swap internally
- **Auto pair resolution**: On initialize, queries the router to resolve the pair address for `(token_a, token_b)` — no manual pair address needed
- **Exact pre-auth amounts**: Before `add_liquidity`, queries pair reserves and computes `b_optimal = remaining_a × reserve_b / reserve_a` to pre-authorize the exact amount the router will use (avoids auth mismatch from router's internal ratio adjustment)
- **Proportional withdrawal**: Burns LP tokens proportional to `amount / total_balance` for partial exits
- **Round-trip to token_a**: On withdraw, all received token_b is swapped back to token_a before sending to vault
- **No impermanent loss tracking**: NAV is computed at spot price; impermanent loss is implicit

## Project Structure

```
src/
├── lib.rs                # Crate root: contractimport! for router.wasm and pair.wasm
├── contract.rs           # #[contract] SoroswapAdapter — IAdapter implementation
├── admin/
│   └── mod.rs            # Admin::initialize — queries router to resolve pair address
├── soroswap_pool/
│   └── mod.rs            # deposit, withdraw, position_value — Soroswap API wrappers
├── common/
│   ├── mod.rs
│   ├── error.rs          # Error enum (8 variants)
│   ├── events.rs         # Event types: Deposited, Withdrawn
│   ├── storage.rs        # AdapterStorage load/save with TTL
│   └── types.rs          # AdapterStorage struct
└── test/
    └── mod.rs            # Tests (6 tests) using real Soroswap WASMs
```

## Core Concepts

### Deposit Flow (single-asset entry)

```
vault → token_a.transfer(vault, adapter, amount)    [vault self-auth]
vault → adapter.a_deposit(amount, vault)
  adapter → swap_amount = amount / 2
  adapter → authorize_as_current_contract([token_a.transfer(adapter, pair, swap_amount)])
  adapter → router.swap_exact_tokens_for_tokens(swap_amount, 0, [A,B], adapter, deadline)
    → b_received (token_b received from swap)

  adapter → query pair.get_reserves() + pair.token_0()
  adapter → b_optimal = remaining_a × reserve_b / reserve_a
  adapter → b_to_add  = min(b_optimal, b_received)

  adapter → authorize_as_current_contract([
    token_a.transfer(adapter, pair, remaining_a),
    token_b.transfer(adapter, pair, b_to_add),
  ])
  adapter → router.add_liquidity(A, B, remaining_a, b_to_add, 0, 0, adapter, deadline)
    → LP tokens minted to adapter
  adapter → returns position_value(adapter) in token_a units
```

### Why `b_optimal` must be pre-computed

Soroswap's `add_liquidity` internally adjusts the token_b amount to maintain the optimal pool ratio:

```
b_optimal = amount_a × reserve_b / reserve_a
```

If we pre-authorize with `b_received` (the raw swap output) but the router uses `b_optimal < b_received`, the `authorize_as_current_contract` entry won't match — the auth verification fails because argument values must match exactly. By querying the reserves and computing `b_optimal` ourselves before the call, we pre-authorize with the exact amount the router will use.

### Withdraw Flow

```
vault → adapter.a_withdraw(amount, vault)
  adapter → lp_to_burn = lp_balance × (amount / total_balance)
  adapter → authorize_as_current_contract([pair.transfer(adapter, pair, lp_to_burn)])
  adapter → router.remove_liquidity(A, B, lp_to_burn, 0, 0, adapter, deadline)
    → (a_out, b_out) received by adapter

  if b_out > 0:
    adapter → authorize_as_current_contract([token_b.transfer(adapter, pair, b_out)])
    adapter → router.swap_exact_tokens_for_tokens(b_out, 0, [B,A], adapter, deadline)
    → swapped_a received

  adapter → token_a.transfer(adapter, vault, a_out + swapped_a)
  adapter → returns total_a
```

### Balance Calculation

The position value in token_a units is computed from the adapter's LP share and the pair's current reserves:

```
share_a   = reserve_a × lp_balance / total_lp
share_b   = reserve_b × lp_balance / total_lp
b_in_a    = share_b × reserve_a / reserve_b      (AMM spot price conversion)
balance   = share_a + b_in_a
```

As traders pay fees into the pair, the reserves grow — the same number of LP tokens becomes worth more token_a over time.

### AdapterStorage (instance storage)

```rust
pub struct AdapterStorage {
    pub vault:   Address,  // Authorized rwa-vault address
    pub router:  Address,  // Soroswap router contract
    pub pair:    Address,  // Soroswap pair (token_a / token_b), auto-resolved on init
    pub token_a: Address,  // deposit_token (single-asset entry, e.g. USDC)
    pub token_b: Address,  // pair token (e.g. XLM)
    pub admin:   Address,
}
```

## Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    vault: Address,   // rwa-vault address
    router: Address,  // Soroswap router contract address
    token_a: Address, // deposit token (e.g. USDC) — single-asset entry point
    token_b: Address, // pair token (e.g. XLM)
)
```

The `pair` address is resolved automatically by calling `router.router_pair_for(token_a, token_b)`. The pair must already exist (created via Soroswap factory).

## Key Functions

### IAdapter Interface

```rust
// Deposit token_a (swap half → add liquidity → hold LP tokens).
// Pre-condition: vault has already transferred `amount` token_a to this adapter.
// Returns the adapter's position value in token_a units after deposit.
let balance: i128 = adapter.a_deposit(&amount, &vault_address);

// Withdraw token_a from the pair and transfer to vault.
// Burns LP proportional to amount / total_balance.
// Returns the actual token_a amount sent to vault.
let received: i128 = adapter.a_withdraw(&amount, &vault_address);

// Returns the adapter's LP position value in token_a units.
// value = share_a + share_b × (reserve_a / reserve_b)
let balance: i128 = adapter.a_balance(&vault_address);

// Returns 0 — AMM yield is reflected in LP token value growth.
let apy: u32 = adapter.a_get_apy();

// Returns 0 — Soroswap fees accrue into reserves automatically (no explicit claim).
let harvested: i128 = adapter.a_harvest(&vault_address);
```

### View Functions

```rust
let vault:   Address = adapter.get_vault();
let router:  Address = adapter.get_router();
let pair:    Address = adapter.get_pair();
let token_a: Address = adapter.get_token_a();
let token_b: Address = adapter.get_token_b();
```

## Soroswap WASM Imports

```rust
pub mod soroswap_router {
    soroban_sdk::contractimport!(file = "../external_wasms/soroswap/router.wasm");
    pub type RouterClient<'a> = Client<'a>;
}

pub mod soroswap_pair {
    soroban_sdk::contractimport!(file = "../external_wasms/soroswap/pair.wasm");
    pub type PairClient<'a> = Client<'a>;
}
```

Paths are relative to `Cargo.toml`. WASMs must be present in `external_wasms/soroswap/`.

## Error Codes

| Code | Name                  | Description                                      |
| ---- | --------------------- | ------------------------------------------------ |
| 1    | `AlreadyInitialized`  | Contract already initialized                     |
| 2    | `NotInitialized`      | Contract not initialized                         |
| 3    | `NotVault`            | Caller is not the registered vault               |
| 4    | `NotAdmin`            | Caller is not the admin                          |
| 5    | `ZeroAmount`          | Amount must be positive                          |
| 6    | `ArithmeticError`     | Overflow or division by zero                     |
| 7    | `InsufficientBalance` | Adapter has no LP tokens to withdraw             |
| 8    | `PairNotFound`        | Pair for (token_a, token_b) does not exist       |

## Events

| Event       | Fields                                                                  |
| ----------- | ----------------------------------------------------------------------- |
| `Deposited` | `adapter: Address, token_a: Address, amount_a: i128, lp_minted: i128`  |
| `Withdrawn` | `adapter: Address, token_a: Address, amount_out: i128`                  |

## Usage Example

### Connecting rwa-vault to a USDC/XLM Soroswap pool

```rust
// 1. Deploy adapter (pair must already exist on Soroswap)
adapter.initialize(
    &admin,
    &vault_contract,    // rwa-vault address
    &soroswap_router,   // Soroswap router contract
    &usdc_token,        // token_a: single-asset entry (USDC)
    &xlm_token,         // token_b: pair token (XLM)
);
// pair address is auto-resolved from router.router_pair_for(usdc, xlm)

// 2. Register adapter in vault
vault.add_protocol(
    &symbol_short!("SRSWP"),
    &adapter_contract,
    &2000u32,           // 20% target allocation
    &RiskTier::Medium,
);

// 3. Vault deposit flow (called internally by vault on rebalance)
//    vault → usdc.transfer(vault, adapter, 1000 USDC)
//    vault → adapter.a_deposit(1000 USDC, vault)
//      → swap 500 USDC → ~500 XLM (at current price)
//      → add_liquidity(500 USDC, ~500 XLM) → LP tokens held by adapter
//      → returns position value in USDC

// 4. Vault withdraw flow
//    vault → adapter.a_withdraw(500 USDC, vault)
//      → burns proportional LP tokens
//      → remove_liquidity → (~250 USDC, ~250 XLM)
//      → swap ~250 XLM → USDC
//      → transfers ~500 USDC to vault

// 5. Balance grows as trading fees accrue into the pair reserves
let position: i128 = adapter.a_balance(&vault_contract);
```

## Testing

Tests use real Soroswap WASMs (factory, router, pair) from `external_wasms/soroswap/`.

```bash
cargo test -p adapter-soroswap
```

6 tests covering:

- Initialization and auto-resolution of `pair` address
- Balance starts at zero before any deposit
- Deposit creates LP position with positive value
- Withdraw transfers token_a back to vault
- APY returns 0
- Harvest returns 0

## Building

```bash
# adapter-soroswap only depends on soroban-sdk — no local WASM build order needed
cargo build --target wasm32v1-none --release -p adapter-soroswap

# Output: target/wasm32v1-none/release/adapter_soroswap.wasm
```

## Related Contracts

| Contract                                          | Description                        | Relationship                                           |
| ------------------------------------------------- | ---------------------------------- | ------------------------------------------------------ |
| [rwa-vault](../rwa-vault)                         | Yield aggregator vault             | Calls this adapter via IAdapter interface              |
| [adapter-blend](../adapter-blend)                 | Blend Protocol adapter             | Sibling adapter — same IAdapter interface              |
| [adapter-rwa-lending](../adapter-rwa-lending)     | RWA Lending adapter                | Sibling adapter — same IAdapter interface              |
| [external_wasms/soroswap](../external_wasms/soroswap) | Soroswap WASMs                 | router + pair imported at compile time; all 3 used in tests |

## License

MIT

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
