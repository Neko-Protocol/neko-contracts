use soroban_sdk::{panic_with_error, Address, Env};

use crate::common::error::Error;
use crate::common::storage::Storage;
use crate::common::types::AdapterStorage;
use crate::soroswap_router;

pub struct Admin;

impl Admin {
    pub fn initialize(
        env: &Env,
        admin: &Address,
        vault: &Address,
        router: &Address,
        token_a: &Address,
        token_b: &Address,
    ) {
        if Storage::is_initialized(env) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }

        // Resolve the pair address via the router
        let router_client = soroswap_router::RouterClient::new(env, router);
        let pair = router_client.router_pair_for(token_a, token_b);

        Storage::save(
            env,
            &AdapterStorage {
                vault:   vault.clone(),
                router:  router.clone(),
                pair,
                token_a: token_a.clone(),
                token_b: token_b.clone(),
                admin:   admin.clone(),
            },
        );
    }
}
