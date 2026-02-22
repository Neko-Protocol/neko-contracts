use soroban_sdk::{contracttype, Address, Symbol, Vec};

/// 12 decimal precision — matches Blend's b_rate scale
pub const SCALAR_12: i128 = 1_000_000_000_000;

pub const ONE_DAY_LEDGERS: u32 = 17280;
pub const INSTANCE_TTL: u32 = ONE_DAY_LEDGERS * 30;
pub const INSTANCE_BUMP: u32 = ONE_DAY_LEDGERS * 31;

pub use soroban_sdk::symbol_short;
pub const STORAGE_KEY: Symbol = symbol_short!("ASTORAGE");

/// Blend request types used in pool.submit()
pub const REQUEST_SUPPLY: u32 = 0;
pub const REQUEST_WITHDRAW: u32 = 1;

/// Adapter configuration stored in instance storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdapterStorage {
    /// rwa-vault contract — only caller allowed for a_deposit / a_withdraw
    pub vault: Address,
    /// Blend pool contract address
    pub blend_pool: Address,
    /// RWA / underlying token (e.g. CETES, USDC)
    pub deposit_token: Address,
    /// BLND reward token emitted by the pool
    pub blend_token: Address,
    /// Blend reserve index for deposit_token (from pool.get_reserve(token).config.index)
    pub reserve_id: u32,
    /// Claim IDs for BLND emissions: [reserve_id * 2 + 1] (bToken emissions only)
    pub claim_ids: Vec<u32>,
    /// Admin address
    pub admin: Address,
}
