use soroban_sdk::{panic_with_error, Env, Map, Vec};

use crate::common::error::Error;
use crate::rwa::types::{RWAAssetType, RWAMetadata};
use crate::{Asset, Symbol, contracttype};

use super::types::{DEFAULT_MAX_STALENESS, STORAGE};

#[contracttype]
#[derive(Clone, Debug)]
pub struct RWAOracleStorage {
    // Price data stream (SEP-40 compatible)
    pub assets: Vec<Asset>,
    pub base: Asset,
    pub decimals: u32,
    pub resolution: u32,
    pub last_timestamp: u64,
    // RWA metadata
    pub rwa_metadata: Map<Symbol, RWAMetadata>,
    // Asset type mapping
    pub asset_types: Map<Asset, RWAAssetType>,
    // Maximum acceptable age for price data (seconds)
    pub max_staleness: u64,
}

impl RWAOracleStorage {
    pub fn new(env: &Env, assets: Vec<Asset>, base: Asset, decimals: u32, resolution: u32) -> Self {
        Self {
            assets,
            base,
            decimals,
            resolution,
            last_timestamp: 0,
            rwa_metadata: Map::new(env),
            asset_types: Map::new(env),
            max_staleness: DEFAULT_MAX_STALENESS,
        }
    }

    pub fn get(env: &Env) -> Self {
        env.storage()
            .instance()
            .get(&STORAGE)
            .unwrap_or_else(|| panic_with_error!(env, Error::StorageNotInitialized))
    }

    /// Try to get storage, returns None if not initialized
    pub fn try_get(env: &Env) -> Option<Self> {
        env.storage().instance().get(&STORAGE)
    }

    pub fn set(env: &Env, storage: &Self) {
        env.storage().instance().set(&STORAGE, storage);
    }
}
