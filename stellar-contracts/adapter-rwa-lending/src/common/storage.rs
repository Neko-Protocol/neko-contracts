use soroban_sdk::{panic_with_error, Env};

use crate::common::error::Error;
use crate::common::types::{AdapterStorage, INSTANCE_BUMP, INSTANCE_TTL, STORAGE_KEY};

pub struct Storage;

impl Storage {
    pub fn load(env: &Env) -> AdapterStorage {
        env.storage()
            .instance()
            .get(&STORAGE_KEY)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    pub fn save(env: &Env, storage: &AdapterStorage) {
        env.storage().instance().set(&STORAGE_KEY, storage);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL, INSTANCE_BUMP);
    }

    pub fn is_initialized(env: &Env) -> bool {
        env.storage().instance().has(&STORAGE_KEY)
    }
}
