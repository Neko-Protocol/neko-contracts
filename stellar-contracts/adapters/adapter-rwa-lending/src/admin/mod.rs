use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use crate::common::error::Error;
use crate::common::storage::Storage;
use crate::common::types::AdapterStorage;

pub struct Admin;

impl Admin {
    pub fn initialize(
        env: &Env,
        admin: &Address,
        vault: &Address,
        lending_pool: &Address,
        deposit_token: &Address,
        rwa_asset: Symbol,
    ) {
        if Storage::is_initialized(env) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }

        Storage::save(
            env,
            &AdapterStorage {
                vault: vault.clone(),
                lending_pool: lending_pool.clone(),
                deposit_token: deposit_token.clone(),
                rwa_asset,
                admin: admin.clone(),
            },
        );
    }

}
