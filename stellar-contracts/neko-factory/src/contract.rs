use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, Bytes, BytesN, Env, IntoVal, Vec,
};

use crate::pool_wasm::PoolInitConfig;

use crate::error::Error;
use crate::events;
use crate::storage;

/// Deploys and tracks [`neko-pool`](https://github.com/Neko-Protocol) lending pool instances.
#[contract]
pub struct NekoFactory;

#[contractimpl]
impl NekoFactory {
    /// One-time setup: factory `admin` and the uploaded **neko-pool** WASM hash (`deploy_v2` / tests).
    pub fn __constructor(env: Env, admin: Address, pool_wasm_hash: BytesN<32>) {
        if storage::is_initialized(&env) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        admin.require_auth();
        storage::set_admin(&env, &admin);
        storage::set_pool_wasm_hash(&env, &pool_wasm_hash);
        let empty = Vec::<Address>::new(&env);
        env.storage().instance().set(&storage::DataKey::PoolList, &empty);
    }

    /// Update the WASM hash used for new deployments (e.g. after an audited upgrade). Factory admin only.
    pub fn set_pool_wasm_hash(env: Env, pool_wasm_hash: BytesN<32>) {
        storage::get_admin(&env).require_auth();
        storage::set_pool_wasm_hash(&env, &pool_wasm_hash);
    }

    /// Deploy a new pool contract and register it. The **pool admin** must authorize.
    ///
    /// `salt` is mixed with `pool_admin` (keccak) so two users cannot collide on the same salt.
    pub fn deploy_pool(
        env: Env,
        pool_admin: Address,
        salt: BytesN<32>,
        treasury: Address,
        neko_oracle: Address,
        reflector_oracle: Address,
        backstop_take_rate: u32,
        reserve_factor: u32,
        origination_fee_rate: u32,
        liquidation_fee_rate: u32,
    ) -> Address {
        if !storage::is_initialized(&env) {
            panic_with_error!(&env, Error::NotInitialized);
        }
        pool_admin.require_auth();
        storage::extend_instance_ttl(&env);
        storage::validate_pool_deploy_args(
            &env,
            backstop_take_rate,
            reserve_factor,
            origination_fee_rate,
            liquidation_fee_rate,
        );

        let wasm_hash = storage::get_pool_wasm_hash(&env);

        // keccak(salt ‖ pool_admin) so two pool admins never collide on the same salt.
        let mut as_u8s: [u8; 56] = [0; 56];
        pool_admin.to_string().copy_into_slice(&mut as_u8s);
        let mut salt_as_bytes: Bytes = salt.into_val(&env);
        salt_as_bytes.extend_from_array(&as_u8s);
        let new_salt: BytesN<32> = env.crypto().keccak256(&salt_as_bytes).to_bytes();

        let config = PoolInitConfig {
            admin: pool_admin.clone(),
            treasury,
            neko_oracle,
            reflector_oracle,
            backstop_take_rate,
            reserve_factor,
            origination_fee_rate,
            liquidation_fee_rate,
        };

        let factory_addr = env.current_contract_address();
        let pool_address = env
            .deployer()
            .with_address(factory_addr, new_salt)
            .deploy_v2(wasm_hash, (config,));

        storage::set_deployed(&env, &pool_address);
        storage::push_pool(&env, &pool_address);
        events::deploy(&env, pool_address.clone());
        pool_address
    }

    pub fn is_pool(env: Env, pool_id: Address) -> bool {
        storage::extend_instance_ttl(&env);
        storage::is_deployed(&env, &pool_id)
    }

    pub fn get_pools(env: Env) -> Vec<Address> {
        storage::get_pools(&env)
    }

    pub fn pool_wasm_hash(env: Env) -> BytesN<32> {
        storage::get_pool_wasm_hash(&env)
    }

    pub fn factory_admin(env: Env) -> Address {
        storage::get_admin(&env)
    }
}
