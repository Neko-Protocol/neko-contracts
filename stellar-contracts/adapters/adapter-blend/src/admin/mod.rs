use soroban_sdk::{panic_with_error, vec, Address, Env};

use crate::blend;
use crate::common::error::Error;
use crate::common::storage::Storage;
use crate::common::types::AdapterStorage;

pub struct Admin;

impl Admin {
    pub fn initialize(
        env: &Env,
        admin: &Address,
        vault: &Address,
        blend_pool: &Address,
        deposit_token: &Address,
        blend_token: &Address,
    ) {
        if Storage::is_initialized(env) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }

        // Query the Blend pool to get the reserve_id for deposit_token
        let pool = blend::PoolClient::new(env, blend_pool);
        let reserve = pool.get_reserve(deposit_token);
        let reserve_id = reserve.config.index;

        // bToken claim ID = reserve_index * 2 + 1
        let claim_id = reserve_id * 2 + 1;
        let claim_ids = vec![env, claim_id];

        Storage::save(
            env,
            &AdapterStorage {
                vault: vault.clone(),
                blend_pool: blend_pool.clone(),
                deposit_token: deposit_token.clone(),
                blend_token: blend_token.clone(),
                reserve_id,
                claim_ids,
                admin: admin.clone(),
            },
        );
    }

}
