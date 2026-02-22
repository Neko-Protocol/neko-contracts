use soroban_sdk::{contracttype, Address, Symbol};

pub const SCALAR_12: i128 = 1_000_000_000_000;

pub const ONE_DAY_LEDGERS: u32 = 17280;
pub const INSTANCE_TTL: u32 = ONE_DAY_LEDGERS * 30;
pub const INSTANCE_BUMP: u32 = ONE_DAY_LEDGERS * 31;

pub use soroban_sdk::symbol_short;
pub const STORAGE_KEY: Symbol = symbol_short!("ASTORAGE");

/// Adapter configuration (instance storage)
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdapterStorage {
    /// Vault contract allowed to call this adapter
    pub vault: Address,
    /// rwa-lending pool contract address
    pub lending_pool: Address,
    /// RWA token (deposit token) address
    pub deposit_token: Address,
    /// Asset symbol used in rwa-lending (e.g. symbol_short!("CETES"))
    pub rwa_asset: Symbol,
    /// Admin address
    pub admin: Address,
}
