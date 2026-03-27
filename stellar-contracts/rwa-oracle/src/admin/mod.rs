use soroban_sdk::{Address, BytesN, Env, panic_with_error};

use crate::common::error::Error;
use crate::common::storage::RWAOracleStorage;
use crate::common::types::{
    ADMIN_KEY, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PAUSED_KEY, PENDING_ADMIN_KEY,
};

/// Administrative functions for the oracle contract
pub struct Admin;

impl Admin {
    /// Get the admin address
    pub fn get_admin(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Admin must be set")
    }

    /// Set the admin address (used in constructor)
    pub fn set_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&ADMIN_KEY, admin);
    }

    /// Require admin authorization
    pub fn require_admin(env: &Env) {
        let admin = Self::get_admin(env);
        admin.require_auth();
    }

    /// Upgrade the contract to new wasm
    pub fn upgrade(env: &Env, new_wasm_hash: BytesN<32>) {
        Self::require_admin(env);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    /// Set the maximum acceptable age (in seconds) for price data
    pub fn set_max_staleness(env: &Env, max_seconds: u64) {
        Self::require_not_paused(env);
        Self::require_admin(env);
        let mut state = RWAOracleStorage::get(env);
        state.max_staleness = max_seconds;
        RWAOracleStorage::set(env, &state);
        Self::extend_instance_ttl(env);
    }

    /// Get the pending admin address (if any)
    pub fn get_pending_admin(env: &Env) -> Option<Address> {
        env.storage().instance().get(&PENDING_ADMIN_KEY)
    }

    /// Propose a new admin (two-step transfer, step 1)
    pub fn propose_admin(env: &Env, new_admin: &Address) {
        Self::require_admin(env);
        env.storage().instance().set(&PENDING_ADMIN_KEY, new_admin);
        Self::extend_instance_ttl(env);
    }

    /// Accept admin role (two-step transfer, step 2)
    pub fn accept_admin(env: &Env) {
        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN_KEY)
            .expect("No pending admin");
        pending.require_auth();

        env.storage().instance().set(&ADMIN_KEY, &pending);
        env.storage().instance().remove(&PENDING_ADMIN_KEY);
        Self::extend_instance_ttl(env);
    }

    /// Check whether the contract is currently paused
    pub fn is_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&PAUSED_KEY)
            .unwrap_or(false)
    }

    /// Pause the contract (admin only)
    pub fn pause(env: &Env) {
        Self::require_admin(env);
        env.storage().instance().set(&PAUSED_KEY, &true);
        Self::extend_instance_ttl(env);
    }

    /// Unpause the contract (admin only)
    pub fn unpause(env: &Env) {
        Self::require_admin(env);
        env.storage().instance().set(&PAUSED_KEY, &false);
        Self::extend_instance_ttl(env);
    }

    /// Panic with Paused error if the contract is currently paused
    pub fn require_not_paused(env: &Env) {
        if Self::is_paused(env) {
            panic_with_error!(env, Error::Paused);
        }
    }

    /// Extend instance TTL
    pub fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    }
}
