use soroban_sdk::{contracttype, Address};

/// Adapter configuration stored in instance storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdapterStorage {
    /// rwa-vault contract — authorized caller for a_deposit / a_withdraw / a_harvest
    pub vault: Address,
    /// Aquarius pool contract (called directly for swap, deposit, withdraw, claim)
    pub pool: Address,
    /// Deposit token — single-asset entry point (e.g. CETES)
    pub deposit_token: Address,
    /// Index of deposit_token in the pool's token list (0 or 1)
    pub deposit_token_idx: u32,
    /// Pair token (e.g. USDC)
    pub pair_token: Address,
    /// Index of pair_token in the pool's token list (0 or 1)
    pub pair_token_idx: u32,
    /// LP share token minted by the pool
    pub share_token: Address,
    /// AQUA reward token claimable via pool.claim()
    pub aqua_token: Address,
    /// Maximum acceptable slippage in basis points (e.g. 50 = 0.5%). Max 1000 (10%).
    pub max_slippage_bps: u32,
    /// Admin address
    pub admin: Address,
}
