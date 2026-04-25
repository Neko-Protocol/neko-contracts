#![cfg(test)]

use neko_pool::{LendingContractClient, PoolState};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, Bytes, BytesN, Env,
};

use crate::{NekoFactory, NekoFactoryClient};

fn pool_wasm_bytes() -> &'static [u8] {
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../target/wasm32v1-none/release/neko_pool.wasm"
    ))
}

#[test]
fn test_factory_deploy_and_registry() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();

    let factory_admin = Address::generate(&e);
    let wasm_hash = e
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(&e, pool_wasm_bytes()));

    let factory_id = e.register(
        NekoFactory,
        (factory_admin.clone(), wasm_hash.clone()),
    );
    let factory = NekoFactoryClient::new(&e, &factory_id);

    let pool_admin = Address::generate(&e);
    let treasury = Address::generate(&e);
    let oracle = Address::generate(&e);
    let salt = BytesN::random(&e);

    let pool_addr = factory.deploy_pool(
        &pool_admin,
        &salt,
        &treasury,
        &oracle,
        &oracle,
        &500_000u32,
        &1_000_000u32,
        &40_000u32,
        &100_000u32,
    );

    assert!(factory.is_pool(&pool_addr));
    assert_eq!(factory.get_pools().len(), 1);
    assert_eq!(factory.pool_wasm_hash(), wasm_hash);
    assert_eq!(factory.factory_admin(), factory_admin);

    let pool = LendingContractClient::new(&e, &pool_addr);
    assert_eq!(pool.get_pool_state(), PoolState::OnIce);
}

#[test]
fn test_deploy_same_salt_different_pool_admin_differs() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();

    let factory_admin = Address::generate(&e);
    let wasm_hash = e
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(&e, pool_wasm_bytes()));
    let factory_id = e.register(NekoFactory, (factory_admin, wasm_hash));
    let factory = NekoFactoryClient::new(&e, &factory_id);

    let salt = BytesN::random(&e);
    let oracle = Address::generate(&e);
    let a = Address::generate(&e);
    let b = Address::generate(&e);
    let t = Address::generate(&e);

    let p1 = factory.deploy_pool(
        &a,
        &salt,
        &t,
        &oracle,
        &oracle,
        &500_000u32,
        &1_000_000u32,
        &40_000u32,
        &100_000u32,
    );
    let p2 = factory.deploy_pool(
        &b,
        &salt,
        &t,
        &oracle,
        &oracle,
        &500_000u32,
        &1_000_000u32,
        &40_000u32,
        &100_000u32,
    );
    assert_ne!(p1, p2);
}

#[test]
#[should_panic(expected = "Error(Contract, #1300)")]
fn test_deploy_invalid_fee_sum() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();

    let factory_admin = Address::generate(&e);
    let wasm_hash = e
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(&e, pool_wasm_bytes()));
    let factory_id = e.register(NekoFactory, (factory_admin, wasm_hash));
    let factory = NekoFactoryClient::new(&e, &factory_id);

    let pool_admin = Address::generate(&e);
    let t = Address::generate(&e);
    let oracle = Address::generate(&e);
    factory.deploy_pool(
        &pool_admin,
        &BytesN::random(&e),
        &t,
        &oracle,
        &oracle,
        &9_000_000u32,
        &9_000_000u32,
        &40_000u32,
        &100_000u32,
    );
}
