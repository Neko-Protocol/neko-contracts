use soroban_sdk::{
    Address, BytesN, Env, Vec,
    contracttype,
    panic_with_error,
    unwrap::UnwrapOptimized,
};

use crate::error::Error;

/// 7 decimals fixed-point scale (same as neko-pool fee checks).
const SCALAR_7: i128 = 10_000_000;

const ONE_DAY_LEDGERS: u32 = 17_280;
const LEDGER_THRESHOLD_INSTANCE: u32 = ONE_DAY_LEDGERS * 30;
const LEDGER_BUMP_INSTANCE: u32 = LEDGER_THRESHOLD_INSTANCE + ONE_DAY_LEDGERS;
const LEDGER_THRESHOLD_USER: u32 = ONE_DAY_LEDGERS * 100;
const LEDGER_BUMP_USER: u32 = LEDGER_THRESHOLD_USER + 20 * ONE_DAY_LEDGERS;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    PoolWasmHash,
    /// Ordered list of pools deployed through this factory (for `get_pools`).
    PoolList,
    Deployed(Address),
}

pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD_INSTANCE, LEDGER_BUMP_INSTANCE);
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
    extend_instance_ttl(env);
}

pub fn get_admin(env: &Env) -> Address {
    extend_instance_ttl(env);
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_optimized()
}

pub fn set_pool_wasm_hash(env: &Env, hash: &BytesN<32>) {
    env.storage().instance().set(&DataKey::PoolWasmHash, hash);
    extend_instance_ttl(env);
}

pub fn get_pool_wasm_hash(env: &Env) -> BytesN<32> {
    extend_instance_ttl(env);
    env.storage()
        .instance()
        .get(&DataKey::PoolWasmHash)
        .unwrap_optimized()
}

pub fn push_pool(env: &Env, pool: &Address) {
    let mut list: Vec<Address> = env
        .storage()
        .instance()
        .get(&DataKey::PoolList)
        .unwrap_or_else(|| Vec::new(env));
    list.push_back(pool.clone());
    env.storage().instance().set(&DataKey::PoolList, &list);
    extend_instance_ttl(env);
}

pub fn get_pools(env: &Env) -> Vec<Address> {
    extend_instance_ttl(env);
    env.storage()
        .instance()
        .get(&DataKey::PoolList)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_deployed(env: &Env, pool: &Address) {
    let key = DataKey::Deployed(pool.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD_USER, LEDGER_BUMP_USER);
}

pub fn is_deployed(env: &Env, pool: &Address) -> bool {
    let key = DataKey::Deployed(pool.clone());
    if let Some(v) = env.storage().persistent().get::<DataKey, bool>(&key) {
        env.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD_USER, LEDGER_BUMP_USER);
        v
    } else {
        false
    }
}

/// Validate deploy args (mirrors neko-pool `Admin::initialize` checks).
pub fn validate_pool_deploy_args(
    env: &Env,
    backstop_take_rate: u32,
    reserve_factor: u32,
    origination_fee_rate: u32,
    liquidation_fee_rate: u32,
) {
    if (reserve_factor as i128 + backstop_take_rate as i128) > SCALAR_7 {
        panic_with_error!(env, Error::InvalidPoolDeployArgs);
    }
    if origination_fee_rate as i128 > SCALAR_7 {
        panic_with_error!(env, Error::InvalidPoolDeployArgs);
    }
    if liquidation_fee_rate as i128 > SCALAR_7 {
        panic_with_error!(env, Error::InvalidPoolDeployArgs);
    }
}
