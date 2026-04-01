# rwa-faucet

Bulk mint contract for RWA tokens. Allows minting multiple tokens to multiple recipients in a single Soroban invocation — useful for testnet airdrops, dev environments, and batch distributions.

## Integration with Neko Protocol

The faucet plugs into the existing stack as follows:

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  rwa-oracle  │────▶│  rwa-token   │◀────│  rwa-faucet  │
│   (SEP-40)   │     │   (SEP-41)   │     │  bulk mint   │
└──────────────┘     └──────┬───────┘     └──────────────┘
                            │
                            ▼
                    ┌───────────────┐
                    │  rwa-lending  │
                    │  rwa-vault    │
                    └───────────────┘
```

### Requirements

1. **Admin role**: The faucet must be the **admin** of each token it mints. The rwa-token admin is set at deployment and cannot be changed later.

2. **Deploy order**:
   - Deploy `rwa-faucet` first.
   - Deploy each `rwa-token` with `admin = faucet contract address`.

3. **Auth for `bulk_mint`**:
   - The faucet calls `require_auth()` on itself, so the caller must authorize the faucet (e.g. faucet admin signs for the faucet).
   - `set_authorized` / `mint`: the token admin (faucet) authorizes via the same auth chain.
   - For rwa-token: `mint` also requires each recipient to authorize → each recipient signs for themselves.

### Deployment Flow

```bash
# 1. Build WASM
cargo build --target wasm32v1-none --release -p rwa-faucet

# 2. Deploy faucet (using stellar-cli or your deploy script)
# faucet_addr = deploy(rwa_faucet.wasm)

# 3. Deploy rwa-token with admin = faucet_addr
# rwa_token.initialize(admin: faucet_addr, asset_contract, pegged_asset, ...)

# 4. Initialize faucet with your deployer as faucet admin
# faucet.initialize(admin: deployer_address)
```

### Usage

**Single recipient, single token:**
```rust
let requests = vec![
    MintRequest { token: nvda_token, to: user_a, amount: 1_000_0000000 }
];
faucet_client.bulk_mint(&requests);
```

**Multiple recipients, multiple tokens:**
```rust
let requests = vec![
    MintRequest { token: nvda_token, to: user_a, amount: 500_0000000 },
    MintRequest { token: nvda_token, to: user_b, amount: 300_0000000 },
    MintRequest { token: tsla_token, to: user_a, amount: 200_0000000 },
];
faucet_client.bulk_mint(&requests);
```

### Token Compatibility

| Token Type | Compatible | Notes |
|------------|------------|-------|
| Stellar Asset Contract | ✅ | Tests use this; issuer = admin |
| rwa-token (SEP-41) | ✅ | Deploy with admin = faucet address |

### Build & Test

```bash
cargo build -p rwa-faucet
cargo test -p rwa-faucet
cargo build --target wasm32v1-none --release -p rwa-faucet
```

Output: `target/wasm32v1-none/release/rwa_faucet.wasm`
