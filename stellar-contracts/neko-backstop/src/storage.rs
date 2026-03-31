use soroban_sdk::{Address, Env, panic_with_error};

use crate::error::Error;
use crate::types::{
    BackstopDeposit, DataKey, INSTANCE_BUMP, INSTANCE_TTL, USER_BUMP, USER_TTL,
};

pub struct Storage;

impl Storage {
    // =========================================================================
    // TTL helpers
    // =========================================================================

    pub fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL, INSTANCE_BUMP);
    }

    fn extend_user_ttl(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, USER_TTL, USER_BUMP);
    }

    // =========================================================================
    // Initialization
    // =========================================================================

    pub fn is_initialized(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Admin)
    }

    // =========================================================================
    // Admin
    // =========================================================================

    pub fn get_admin(env: &Env) -> Address {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    pub fn set_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Pool contract reference
    // =========================================================================

    pub fn get_pool_contract(env: &Env) -> Address {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::PoolContract)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    pub fn set_pool_contract(env: &Env, pool: &Address) {
        env.storage().instance().set(&DataKey::PoolContract, pool);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Backstop token
    // =========================================================================

    pub fn get_backstop_token(env: &Env) -> Option<Address> {
        Self::extend_instance_ttl(env);
        env.storage().instance().get(&DataKey::BackstopToken)
    }

    pub fn set_backstop_token(env: &Env, token: &Address) {
        env.storage().instance().set(&DataKey::BackstopToken, token);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Backstop threshold
    // =========================================================================

    pub fn get_backstop_threshold(env: &Env) -> i128 {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::BackstopThreshold)
            .unwrap_or(0)
    }

    pub fn set_backstop_threshold(env: &Env, threshold: i128) {
        env.storage()
            .instance()
            .set(&DataKey::BackstopThreshold, &threshold);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Per-depositor deposit
    // =========================================================================

    pub fn get_backstop_deposit(env: &Env, depositor: &Address) -> Option<BackstopDeposit> {
        let key = DataKey::BackstopDeposit(depositor.clone());
        let result = env.storage().persistent().get(&key);
        if result.is_some() {
            Self::extend_user_ttl(env, &key);
        }
        result
    }

    pub fn set_backstop_deposit(env: &Env, depositor: &Address, deposit: &BackstopDeposit) {
        let key = DataKey::BackstopDeposit(depositor.clone());
        env.storage().persistent().set(&key, deposit);
        Self::extend_user_ttl(env, &key);
    }

    // =========================================================================
    // Global counters
    // =========================================================================

    pub fn get_backstop_total(env: &Env) -> i128 {
        let key = DataKey::BackstopTotal;
        let result: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if result != 0 {
            Self::extend_user_ttl(env, &key);
        }
        result
    }

    pub fn set_backstop_total(env: &Env, total: i128) {
        let key = DataKey::BackstopTotal;
        env.storage().persistent().set(&key, &total);
        Self::extend_user_ttl(env, &key);
    }

    pub fn get_backstop_queued_total(env: &Env) -> i128 {
        let key = DataKey::BackstopQueuedTotal;
        let result: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if result != 0 {
            Self::extend_user_ttl(env, &key);
        }
        result
    }

    pub fn set_backstop_queued_total(env: &Env, total: i128) {
        let key = DataKey::BackstopQueuedTotal;
        env.storage().persistent().set(&key, &total);
        Self::extend_user_ttl(env, &key);
    }
}
