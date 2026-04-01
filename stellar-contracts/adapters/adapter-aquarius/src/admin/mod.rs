use soroban_sdk::{panic_with_error, Address, Env};

use crate::aquarius_pool_contract;
use crate::common::error::Error;
use crate::common::storage::Storage;
use crate::common::types::AdapterStorage;

/// Maximum slippage configurable by admin: 10%
pub const MAX_SLIPPAGE_BPS: u32 = 1_000;

pub struct Admin;

impl Admin {
    /// Initialize the adapter.
    ///
    /// Resolves deposit_token_idx, pair_token_idx, and share_token automatically
    /// by querying the Aquarius pool — no manual index configuration needed.
    ///
    /// `max_slippage_bps`: slippage tolerance applied to all swaps and LP operations
    /// (e.g. 50 = 0.5%). Must be ≤ 1000 (10%).
    pub fn initialize(
        env: &Env,
        admin: &Address,
        vault: &Address,
        pool: &Address,
        deposit_token: &Address,
        pair_token: &Address,
        aqua_token: &Address,
        max_slippage_bps: u32,
    ) {
        if Storage::is_initialized(env) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }
        if max_slippage_bps > MAX_SLIPPAGE_BPS {
            panic_with_error!(env, Error::SlippageOutOfBounds);
        }

        let pool_client = aquarius_pool_contract::PoolClient::new(env, pool);

        // Resolve token indices from pool
        let tokens = pool_client.get_tokens();
        let deposit_token_idx = tokens
            .iter()
            .position(|t| t == *deposit_token)
            .unwrap_or_else(|| panic_with_error!(env, Error::TokenNotInPool)) as u32;
        let pair_token_idx = tokens
            .iter()
            .position(|t| t == *pair_token)
            .unwrap_or_else(|| panic_with_error!(env, Error::TokenNotInPool)) as u32;

        // Resolve LP share token from pool
        let share_token = pool_client.share_id();

        Storage::save(
            env,
            &AdapterStorage {
                vault:             vault.clone(),
                pool:              pool.clone(),
                deposit_token:     deposit_token.clone(),
                deposit_token_idx,
                pair_token:        pair_token.clone(),
                pair_token_idx,
                share_token,
                aqua_token:        aqua_token.clone(),
                max_slippage_bps,
                admin:             admin.clone(),
            },
        );
    }

    /// Update the slippage tolerance. Admin-only.
    pub fn update_slippage(env: &Env, caller: &Address, new_slippage_bps: u32) {
        let mut storage = Storage::load(env);
        if *caller != storage.admin {
            panic_with_error!(env, Error::NotAdmin);
        }
        if new_slippage_bps > MAX_SLIPPAGE_BPS {
            panic_with_error!(env, Error::SlippageOutOfBounds);
        }
        storage.max_slippage_bps = new_slippage_bps;
        Storage::save(env, &storage);
    }
}
