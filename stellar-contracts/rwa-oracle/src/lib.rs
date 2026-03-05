#![no_std]

use soroban_sdk::{Address, Symbol, contracttype};

pub mod admin;
pub mod common;
pub mod contract;
pub mod rwa;
pub mod sep40;

// Re-exports
pub use common::error::Error;
pub use contract::{RWAOracle, RWAOracleClient};
pub use rwa::types::{RWAAssetType, RWAMetadata, TokenizationInfo, ValuationMethod};

/// Quoted asset definition (SEP-40 compatible)
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum Asset {
    /// Can be a Stellar Classic or Soroban asset
    Stellar(Address),
    /// For any external tokens/assets/symbols
    Other(Symbol),
}

/// Price record definition (SEP-40 compatible)
#[contracttype]
#[derive(Debug, Clone)]
pub struct PriceData {
    pub price: i128,    // asset price at given point in time
    pub timestamp: u64, // recording timestamp
}

#[cfg(test)]
mod test;

/// Minimal test contract with no constructor - used to test StorageNotInitialized.
/// Uses same storage layout as RWAOracle so RWAOracleStorage::get() can be exercised.
#[cfg(test)]
pub mod test_contract {
    use soroban_sdk::{contract, contractimpl, Env, Vec};

    use crate::common::storage::RWAOracleStorage;
    use crate::Asset;

    #[contract]
    pub struct RWAOracleStorageTest;

    #[contractimpl]
    impl RWAOracleStorageTest {
        pub fn get_assets(env: &Env) -> Vec<Asset> {
            RWAOracleStorage::get(env).assets.clone()
        }
    }
}
