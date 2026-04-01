use soroban_sdk::{contracttype, Env};

use crate::common::types::AdapterStorage;

#[contracttype]
enum DataKey {
    Storage,
}

pub struct Storage;

impl Storage {
    pub fn is_initialized(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Storage)
    }

    pub fn save(env: &Env, data: &AdapterStorage) {
        env.storage().instance().set(&DataKey::Storage, data);
        env.storage().instance().extend_ttl(
            518_400, // 30 days
            518_400,
        );
    }

    pub fn load(env: &Env) -> AdapterStorage {
        let data = env.storage()
            .instance()
            .get(&DataKey::Storage)
            .unwrap_or_else(|| panic!());
        // Bump TTL on every read so the contract stays alive as long as it is used.
        env.storage().instance().extend_ttl(518_400, 518_400);
        data
    }
}
