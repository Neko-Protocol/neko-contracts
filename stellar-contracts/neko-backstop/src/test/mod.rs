#![cfg(test)]
extern crate std;

use soroban_sdk::{
    Address, Env,
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
};

use crate::{NekoBackstop, NekoBackstopClient};
use crate::types::PoolState;

// ---------------------------------------------------------------------------
// Minimal mock pool — accepts update_pool_state_from_backstop calls
// ---------------------------------------------------------------------------

mod mock_pool {
    use soroban_sdk::{contract, contractimpl, Env};

    #[contract]
    pub struct MockPool;

    #[contractimpl]
    impl MockPool {
        pub fn update_pool_state_from_backstop(_env: Env, _state: u32) {
            // no-op — just accept the call so backstop tests can run standalone
        }
    }
}

fn create_token<'a>(env: &'a Env, admin: &Address) -> (TokenClient<'a>, Address) {
    let token_address = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let asset_client = StellarAssetClient::new(env, &token_address);
    asset_client.mint(admin, &1_000_000_000_000);
    (TokenClient::new(env, &token_address), token_address)
}

fn create_backstop<'a>(
    env: &'a Env,
    admin: &Address,
    pool: &Address,
    token: &Address,
    threshold: i128,
) -> NekoBackstopClient<'a> {
    let contract_id = env.register(NekoBackstop, ());
    let client = NekoBackstopClient::new(env, &contract_id);
    client.initialize(admin, pool, token, &threshold);
    client
}

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (_, token_address) = create_token(&env, &admin);

    let client = create_backstop(&env, &admin, &pool, &token_address, 1_000_000_000);

    assert_eq!(client.get_total(), 0);
    assert_eq!(client.get_pool_state(), PoolState::OnIce); // below threshold
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (_, token_address) = create_token(&env, &admin);

    let contract_id = env.register(NekoBackstop, ());
    let client = NekoBackstopClient::new(&env, &contract_id);
    client.initialize(&admin, &pool, &token_address, &1_000_000_000);
    client.initialize(&admin, &pool, &token_address, &1_000_000_000);
}

#[test]
fn test_deposit_and_total() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token_address) = create_token(&env, &admin);

    let client = create_backstop(&env, &admin, &pool, &token_address, 0);
    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &500_000_000);

    client.deposit(&depositor, &500_000_000);

    assert_eq!(client.get_total(), 500_000_000);
    let deposit = client.get_deposit(&depositor);
    assert_eq!(deposit.amount, 500_000_000);
    assert_eq!(deposit.queued_amount, 0);
}

#[test]
fn test_deposit_reaches_threshold_activates_pool() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token_address) = create_token(&env, &admin);

    let threshold = 1_000_000_000i128;
    let client = create_backstop(&env, &admin, &pool, &token_address, threshold);
    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &threshold);

    client.deposit(&depositor, &threshold);

    // At exactly threshold with no queue → Active
    assert_eq!(client.get_pool_state(), PoolState::Active);
}

#[test]
fn test_initiate_withdrawal_queues() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token_address) = create_token(&env, &admin);

    let client = create_backstop(&env, &admin, &pool, &token_address, 0);
    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &100_000_000);

    client.deposit(&depositor, &100_000_000);
    client.initiate_withdrawal(&depositor, &100_000_000);

    let deposit = client.get_deposit(&depositor);
    assert_eq!(deposit.queued_amount, 100_000_000);
    assert!(deposit.queued_at.is_some());
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")] // WithdrawalQueueNotExpired
fn test_withdraw_before_queue_expires() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token_address) = create_token(&env, &admin);

    let client = create_backstop(&env, &admin, &pool, &token_address, 0);
    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &100_000_000);

    client.deposit(&depositor, &100_000_000);
    client.initiate_withdrawal(&depositor, &100_000_000);

    // Try to withdraw immediately — should fail
    client.withdraw(&depositor, &100_000_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #21)")] // WithdrawalQueueActive
fn test_double_queue_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token_address) = create_token(&env, &admin);

    let client = create_backstop(&env, &admin, &pool, &token_address, 0);
    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &200_000_000);

    client.deposit(&depositor, &200_000_000);
    client.initiate_withdrawal(&depositor, &100_000_000);
    // Second queue entry should fail
    client.initiate_withdrawal(&depositor, &50_000_000);
}
