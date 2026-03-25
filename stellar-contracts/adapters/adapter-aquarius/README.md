<h1 align="center">Adapter: Aquarius AMM</h1>

<p align="center">
  <strong>Part of the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> DeFi ecosystem on Stellar</strong>
</p>

A bridge adapter connecting the **rwa-vault** yield aggregator to an **Aquarius AMM** liquidity pool on Stellar Soroban. This contract implements the `IAdapter` interface consumed by rwa-vault, enabling single-asset entry into an Aquarius pool: the adapter automatically swaps half the deposit into the pair token, provides liquidity, holds LP tokens as its position, and unwinds back to the deposit token on withdrawal. On top of trading fees, LP providers earn **AQUA token rewards** claimable via `a_harvest()`.

## Neko Protocol Integration

```
┌──────────────────────────────────────────────────────────────────┐
│                        Neko Protocol                             │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│   ┌───────────────────────────────────────────────────┐          │
│   │                    RWA Vault                       │          │
│   │            (rwa-vault, NAV accounting)             │          │
│   └─────────────────────┬──────────────────────────────┘          │
│                         │ IAdapter calls                           │
│                         │ a_deposit(amount, from)                  │
│                         │ a_withdraw(amount, to)                   │
│                         │ a_balance(from)                          │
│                         │ a_harvest(to)                            │
│                         ▼                                         │
│   ┌───────────────────────────────────────────────────┐          │
│   │              adapter-aquarius                      │          │
│   │                                                    │          │
│   │  deposit:                                          │          │
│   │    estimate_swap → swap half deposit → pair_token  │          │
│   │    estimate_deposit → add_liquidity → LP held      │          │
│   │                                                    │          │
│   │  withdraw:                                         │          │
│   │    remove_liquidity → (deposit_token, pair_token)  │          │
│   │    estimate_swap → swap pair_token → deposit_token │          │
│   │    transfer to vault                               │          │
│   │                                                    │          │
│   │  harvest:                                          │          │
│   │    pool.claim() → AQUA → vault                     │          │
│   └─────────────────────┬──────────────────────────────┘          │
│                         │ pool.swap / pool.deposit                 │
│                         │ pool.withdraw / pool.claim               │
│                         ▼                                         │
│   ┌───────────────────────────────────────────────────┐          │
│   │             Aquarius Constant-Product Pool         │          │
│   │   deposit_token / pair_token reserves              │          │
│   │   LP tokens represent the pool share               │          │
│   │   Trading fees accrue into reserves → grow NAV     │          │
│   │   AQUA rewards emitted per block → claimable       │          │
│   └───────────────────────────────────────────────────┘          │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

- **a_deposit**: Swaps half of deposit_token to pair_token, adds liquidity, holds LP tokens
- **a_withdraw**: Removes proportional liquidity, swaps pair_token back to deposit_token, sends to vault
- **a_balance**: Returns the adapter's LP position value in deposit_token units (via AMM spot price)
- **a_get_apy**: Returns 0 — AMM yield (trading fees) is reflected in LP token value growth
- **a_harvest**: Claims AQUA rewards via `pool.claim()` and forwards them to vault

## Key Differences from adapter-soroswap

| Aspect | adapter-soroswap | adapter-aquarius |
|---|---|---|
| Pool interaction | Via router (`add_liquidity`, `remove_liquidity`) | Direct pool calls (`pool.deposit`, `pool.withdraw`) |
| Swap routing | `router.swap_exact_tokens_for_tokens(path)` | `pool.swap(in_idx, out_idx, amount, min)` |
| Pool identifier | `pair: Address` | `pool: Address` + indices resolved from `pool.get_tokens()` |
| LP share burn | `pair.transfer(adapter, pair, lp)` | `share_token.burn(adapter, lp)` |
| Slippage | None (MVP) | `estimate_swap` + `estimate_deposit` with configurable `max_slippage_bps` |
| Rewards | None (`a_harvest → 0`) | AQUA via `pool.claim()` |
| Token ordering | Path-based | Index-based, auto-resolved on initialize |

## Architecture

### Project Structure

```
src/
├── lib.rs                 # Crate root: contractimport! for pool WASM
├── contract.rs            # #[contract] AquariusAdapter — IAdapter + admin functions
├── admin/
│   └── mod.rs             # initialize (resolves indices + share_token), update_slippage
├── aquarius_pool/
│   └── mod.rs             # deposit, withdraw, position_value, claim — pool API wrappers
├── common/
│   ├── mod.rs
│   ├── error.rs           # Error enum (9 variants)
│   ├── events.rs          # Event types: Deposited, Withdrawn, Harvested
│   ├── storage.rs         # AdapterStorage load/save with TTL bump on every read
│   └── types.rs           # AdapterStorage struct
└── test/
    └── mod.rs
```

### AdapterStorage (instance storage)

```rust
pub struct AdapterStorage {
    pub vault:             Address,  // Authorized rwa-vault — only caller for a_deposit/withdraw/harvest
    pub pool:              Address,  // Aquarius pool contract (direct calls)
    pub deposit_token:     Address,  // Single-asset entry point (e.g. CETES)
    pub deposit_token_idx: u32,      // Index of deposit_token in the pool (0 or 1), auto-resolved
    pub pair_token:        Address,  // Pair counterpart (e.g. USDC)
    pub pair_token_idx:    u32,      // Index of pair_token in the pool (0 or 1), auto-resolved
    pub share_token:       Address,  // LP share token, auto-resolved via pool.share_id()
    pub aqua_token:        Address,  // AQUA reward token
    pub max_slippage_bps:  u32,      // Slippage tolerance (e.g. 50 = 0.5%). Max 1000 (10%)
    pub admin:             Address,
}
```

## Core Concepts

### Deposit Flow

```
vault → deposit_token.transfer(vault, adapter, amount)   [vault self-auth]
vault → adapter.a_deposit(amount, vault)
  adapter → storage.vault.require_auth()                  [vault-only guard]

  // Step 1: swap half to pair_token
  adapter → estimated_pair = pool.estimate_swap(deposit_idx, pair_idx, swap_amount)
  adapter → min_pair_out = estimated_pair × (1 - slippage)
  adapter → authorize([deposit_token.transfer(adapter, pool, swap_amount)])
  adapter → pair_received = pool.swap(deposit_idx, pair_idx, swap_amount, min_pair_out)

  // Step 2: compute optimal pair amount to deposit
  adapter → reserves = pool.get_reserves()
  adapter → pair_to_add = min(remaining × reserve_pair / reserve_deposit, pair_received)

  // Step 3: add liquidity
  adapter → desired = [remaining, pair_to_add]  (ordered by pool token index)
  adapter → min_shares = pool.estimate_deposit(desired) × (1 - slippage)
  adapter → authorize([deposit_token.transfer, pair_token.transfer])
  adapter → pool.deposit(adapter, desired, min_shares)
    pool: transfers tokens from adapter, refunds any excess, mints LP shares to adapter

  adapter → returns position_value(adapter) in deposit_token units
```

### Why `pair_to_add` must be pre-computed

Aquarius's `pool.deposit()` takes the full `desired_amounts`, transfers them, computes the optimal ratio internally, and refunds any excess. `authorize_as_current_contract` entries are verified with **exact argument matching** — the pre-authorized `transfer(adapter, pool, amount)` must match the value the pool actually uses. By computing `pair_to_add = min(pair_optimal, pair_received)` ourselves before the call and passing it as `desired_b`, we ensure:

1. The pre-auth value matches exactly what the pool will transfer.
2. Minimal excess pair_token remains in the adapter after the operation.

Any residual excess from rounding can be recovered via `sweep()`.

### Withdraw Flow

```
vault → adapter.a_withdraw(amount, vault)
  adapter → storage.vault.require_auth()

  // Compute LP tokens to burn proportionally
  adapter → lp_to_burn = lp_balance × (amount / total_value)

  // Compute min_amounts from reserves with slippage
  adapter → expected_deposit = reserve_deposit × lp_to_burn / total_lp
  adapter → expected_pair    = reserve_pair    × lp_to_burn / total_lp
  adapter → min_amounts = [expected_deposit, expected_pair] × (1 - slippage)

  // Remove liquidity — pool burns LP via share_token.burn(adapter, lp_to_burn)
  adapter → authorize([share_token.burn(adapter, lp_to_burn)])
  adapter → (deposit_out, pair_out) = pool.withdraw(adapter, lp_to_burn, min_amounts)

  // Swap pair_out back to deposit_token
  if pair_out > 0:
    adapter → min_deposit = pool.estimate_swap(pair_idx, deposit_idx, pair_out) × (1 - slippage)
    adapter → authorize([pair_token.transfer(adapter, pool, pair_out)])
    adapter → swapped = pool.swap(pair_idx, deposit_idx, pair_out, min_deposit)

  adapter → deposit_token.transfer(adapter, vault, deposit_out + swapped)
```

### Why `share_token.burn` not `transfer`

Soroswap burns LP shares via a `transfer(adapter → pair)`. Aquarius uses a different mechanism:

```
pool.withdraw(user, share_amount, min_amounts)
  → burn_shares(env, &user, share_amount)
    → share_token.burn(&user, share_amount as i128)  // requires user.require_auth()
```

Since `burn` is called from `burn_shares` inside the pool (not directly by the adapter), `adapter.require_auth()` inside `burn` is not automatically satisfied. The adapter must pre-authorize this exact sub-invocation via `authorize_as_current_contract` with `fn_name = "burn"` and `args = [adapter_addr, lp_to_burn]`.

### Balance Calculation

Position value in deposit_token units, using u128 intermediates to prevent i128 overflow on large pools:

```
lp_balance     = share_token.balance(adapter)
total_lp       = pool.get_total_shares()
reserve_deposit = reserves[deposit_token_idx]
reserve_pair    = reserves[pair_token_idx]

share_deposit  = reserve_deposit × lp_balance / total_lp
share_pair     = reserve_pair    × lp_balance / total_lp
pair_in_deposit = share_pair × reserve_deposit / reserve_pair  (AMM spot price)

position_value = share_deposit + pair_in_deposit
```

As traders pay fees into the pool, reserves grow — the same LP balance becomes worth more deposit_token over time.

### Harvest Flow (AQUA rewards)

Aquarius emits AQUA tokens to LP providers each block. `pool.claim()` is **permissionless** on-chain — no `user.require_auth()` is enforced by the pool. The adapter guards it at the contract level (vault-only) to keep reward accounting consistent with the vault's `harvest_all()` flow.

```
vault → adapter.a_harvest(vault)
  adapter → storage.vault.require_auth()
  adapter → aqua_amount = pool.claim(adapter)   // pool transfers AQUA to adapter
  if aqua_amount > 0:
    adapter → aqua_token.transfer(adapter, vault, aqua_amount)
  adapter → returns aqua_amount
```

The vault accumulates AQUA in `liquid_reserve`. Conversion to deposit_token is left to the vault manager (manual swap or future harvester extension).

## Slippage

All operations that interact with the pool enforce slippage protection using Aquarius's own estimation functions:

| Operation | Estimate function | Applied to |
|---|---|---|
| Swap (deposit step 1) | `pool.estimate_swap(deposit_idx, pair_idx, swap_amount)` | `min_pair_out` in `pool.swap` |
| Add liquidity | `pool.estimate_deposit(desired_amounts)` | `min_shares` in `pool.deposit` |
| Remove liquidity | `reserve × lp_to_burn / total_lp` | `min_amounts` in `pool.withdraw` |
| Swap (withdraw step 2) | `pool.estimate_swap(pair_idx, deposit_idx, pair_out)` | `min_out` in `pool.swap` |

`max_slippage_bps` is stored in `AdapterStorage` and configurable by admin. Default recommendation: **50 bps (0.5%)**. Hard cap: **1000 bps (10%)**.

## Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    vault: Address,          // rwa-vault address
    pool: Address,           // Aquarius pool contract address
    deposit_token: Address,  // single-asset entry point (e.g. CETES)
    pair_token: Address,     // pair counterpart (e.g. USDC)
    aqua_token: Address,     // AQUA reward token
    max_slippage_bps: u32,   // slippage tolerance, e.g. 50 = 0.5%
)
```

On initialize, the adapter queries the pool to automatically resolve:
- `deposit_token_idx` and `pair_token_idx` via `pool.get_tokens()`
- `share_token` via `pool.share_id()`

The pool must already exist and contain both tokens.

## Key Functions

### IAdapter Interface

```rust
// Deposit deposit_token (swap half → add liquidity → hold LP shares).
// Pre-condition: vault has already transferred `amount` deposit_token to this adapter.
// Returns the adapter's position value in deposit_token units after deposit.
let balance: i128 = adapter.a_deposit(&amount, &vault_address);

// Withdraw deposit_token from the pool and transfer to vault.
// Burns LP shares proportional to amount / total_balance.
// Returns the actual deposit_token amount sent to vault.
let received: i128 = adapter.a_withdraw(&amount, &vault_address);

// Returns the adapter's LP position value in deposit_token units.
let balance: i128 = adapter.a_balance(&vault_address);

// Returns 0 — AMM yield is reflected in LP token value growth.
let apy: u32 = adapter.a_get_apy();

// Claims AQUA rewards from the pool and forwards to vault.
// Returns AQUA amount (0 if no rewards have accrued).
let harvested: i128 = adapter.a_harvest(&vault_address);
```

### Admin Functions

```rust
// Update slippage tolerance. Admin-only. Max 1000 bps (10%).
adapter.update_slippage(&admin, &new_slippage_bps);

// Recover tokens stuck in the adapter (e.g. excess pair_token from deposits).
// Admin-only.
adapter.sweep(&admin, &token_address, &destination, &amount);
```

### View Functions

```rust
let vault:              Address = adapter.get_vault();
let pool:               Address = adapter.get_pool();
let deposit_token:      Address = adapter.get_deposit_token();
let pair_token:         Address = adapter.get_pair_token();
let share_token:        Address = adapter.get_share_token();
let aqua_token:         Address = adapter.get_aqua_token();
let deposit_token_idx:  u32     = adapter.get_deposit_token_idx();
let pair_token_idx:     u32     = adapter.get_pair_token_idx();
let max_slippage_bps:   u32     = adapter.get_max_slippage_bps();
```

## Aquarius WASM Import

```rust
pub mod aquarius_pool_contract {
    soroban_sdk::contractimport!(
        file = "../../wasms/external_wasms/aquarius/soroban_liquidity_pool_contract.wasm"
    );
    pub type PoolClient<'a> = Client<'a>;
}
```

Path is relative to `Cargo.toml`. The WASM must be present at `wasms/external_wasms/aquarius/`.

## Error Codes

| Code | Name                  | Description                                           |
|------|-----------------------|-------------------------------------------------------|
| 1    | `AlreadyInitialized`  | Contract already initialized                          |
| 2    | `NotInitialized`      | Contract not initialized                              |
| 3    | `NotVault`            | Caller is not the registered vault                    |
| 4    | `NotAdmin`            | Caller is not the admin                               |
| 5    | `ZeroAmount`          | Amount must be positive                               |
| 6    | `ArithmeticError`     | Overflow or division by zero                          |
| 7    | `InsufficientBalance` | Adapter has no LP tokens to withdraw                  |
| 8    | `TokenNotInPool`      | deposit_token or pair_token not found in pool         |
| 9    | `SlippageOutOfBounds` | max_slippage_bps exceeds the 1000 bps (10%) hard cap  |

## Events

| Event       | Fields                                                                          |
|-------------|---------------------------------------------------------------------------------|
| `Deposited` | `adapter: Address, token: Address, amount_in: i128, lp_value: i128`             |
| `Withdrawn` | `adapter: Address, token: Address, amount_out: i128`                            |
| `Harvested` | `adapter: Address, aqua_token: Address, aqua_harvested: i128`                   |

## Usage Example

### CETES Vault — connecting to a CETES/USDC Aquarius pool

```rust
// 1. Deploy adapter (pool must already exist on Aquarius)
adapter.initialize(
    &admin,
    &vault_contract,      // rwa-vault address
    &aquarius_pool,       // Aquarius CETES/USDC pool address
    &cetes_token,         // deposit_token: single-asset entry (CETES)
    &usdc_token,          // pair_token: counterpart (USDC)
    &aqua_token,          // AQUA reward token
    &50u32,               // max_slippage_bps: 0.5%
);
// deposit_token_idx, pair_token_idx, share_token auto-resolved from pool

// 2. Register adapter in vault
vault.add_protocol(
    &symbol_short!("AQUAR"),
    &adapter_contract,
    &2000u32,             // 20% target allocation
    &RiskTier::Medium,
);

// 3. Vault deposit flow (called internally by vault on rebalance)
//    vault → cetes.transfer(vault, adapter, 100 CETES)
//    vault → adapter.a_deposit(100 CETES, vault)
//      → estimate_swap: 50 CETES → ~5000 USDC (at ~100 USDC/CETES)
//      → swap 50 CETES → USDC (with 0.5% slippage guard)
//      → estimate_deposit → min_shares computed
//      → add_liquidity(50 CETES, ~5000 USDC) → LP tokens held by adapter
//      → returns position value in CETES

// 4. Vault withdraw flow
//    vault → adapter.a_withdraw(50 CETES, vault)
//      → burns proportional LP tokens
//      → remove_liquidity → (~25 CETES, ~2500 USDC)
//      → swap ~2500 USDC → CETES (with slippage guard)
//      → transfers ~50 CETES to vault

// 5. Harvest AQUA rewards
//    vault.harvest_all() → adapter.a_harvest(vault)
//      → pool.claim(adapter) → AQUA transferred to adapter
//      → aqua.transfer(adapter, vault, aqua_amount)
//      vault accumulates AQUA in liquid_reserve

// 6. Recover stuck tokens (if any excess pair_token accumulated)
adapter.sweep(&admin, &usdc_token, &admin_wallet, &stuck_amount);
```

## Building

```bash
cargo build --target wasm32v1-none --release -p adapter-aquarius

# Output: target/wasm32v1-none/release/adapter_aquarius.wasm
```

## Testing

```bash
cargo test -p adapter-aquarius
```

Integration tests require the Aquarius pool WASM at `wasms/external_wasms/aquarius/soroban_liquidity_pool_contract.wasm`.

## Related Contracts

| Contract | Description | Relationship |
|---|---|---|
| [rwa-vault](../../rwa-vault) | Yield aggregator vault | Calls this adapter via IAdapter interface |
| [adapter-soroswap](../adapter-soroswap) | Soroswap AMM adapter | Sibling adapter — same IAdapter, no rewards |
| [adapter-blend](../adapter-blend) | Blend Protocol adapter | Sibling adapter — lending yield + BLND rewards |
| [adapter-rwa-lending](../adapter-rwa-lending) | RWA Lending adapter | Sibling adapter — native rwa-lending protocol |
| [external_wasms/aquarius](../../wasms/external_wasms/aquarius) | Aquarius pool WASM | Imported at compile time via `contractimport!` |

## License

MIT

---

<p align="center">
  Built with ❤️ by the <a href="https://github.com/Neko-Protocol">Neko Protocol</a> team
</p>
