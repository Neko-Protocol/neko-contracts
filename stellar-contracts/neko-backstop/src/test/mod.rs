#![cfg(test)]
extern crate std;

use soroban_sdk::{
    Address, Env,
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
};

use crate::{NekoBackstop, NekoBackstopClient, Q4W};
use crate::types::{PoolState, Q4W_LOCK_SECONDS};

// ---------------------------------------------------------------------------
// Minimal mock pool — accepts update_pool_state_from_backstop calls
// ---------------------------------------------------------------------------

mod mock_pool {
    use soroban_sdk::{contract, contractimpl, Env};

    #[contract]
    pub struct MockPool;

    #[contractimpl]
    impl MockPool {
        pub fn update_pool_state_from_backstop(_env: Env, _state: u32) {}
    }
}

fn create_token<'a>(env: &'a Env, admin: &Address) -> (TokenClient<'a>, Address) {
    let token_address = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(env, &token_address).mint(admin, &1_000_000_000_000);
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (_, token) = create_token(&env, &admin);

    let client = create_backstop(&env, &admin, &pool, &token, 1_000_000_000);

    assert_eq!(client.get_total(), 0);
    // Below threshold → OnIce
    assert_eq!(client.get_pool_state(), PoolState::OnIce);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (_, token) = create_token(&env, &admin);

    let contract_id = env.register(NekoBackstop, ());
    let client = NekoBackstopClient::new(&env, &contract_id);
    client.initialize(&admin, &pool, &token, &1_000_000_000);
    client.initialize(&admin, &pool, &token, &1_000_000_000);
}

#[test]
fn test_deposit_updates_total_and_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &500_000_000);
    client.deposit(&depositor, &500_000_000);

    assert_eq!(client.get_total(), 500_000_000);
    let bal = client.get_user_balance(&depositor);
    assert_eq!(bal.amount, 500_000_000);
    assert_eq!(bal.q4w.len(), 0);
}

#[test]
fn test_deposit_above_threshold_activates_pool() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let threshold = 1_000_000_000i128;
    let client = create_backstop(&env, &admin, &pool, &token, threshold);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &threshold);
    client.deposit(&depositor, &threshold);

    assert_eq!(client.get_pool_state(), PoolState::Active);
}

#[test]
fn test_queue_withdrawal_creates_q4w_entry() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &100_000_000);
    client.deposit(&depositor, &100_000_000);

    let now = env.ledger().timestamp();
    client.queue_withdrawal(&depositor, &60_000_000);

    let bal = client.get_user_balance(&depositor);
    assert_eq!(bal.q4w.len(), 1);
    let entry: Q4W = bal.q4w.get(0).unwrap();
    assert_eq!(entry.amount, 60_000_000);
    assert_eq!(entry.exp, now + Q4W_LOCK_SECONDS);
}

#[test]
fn test_multiple_q4w_entries_allowed() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &300_000_000);
    client.deposit(&depositor, &300_000_000);

    client.queue_withdrawal(&depositor, &100_000_000);
    client.queue_withdrawal(&depositor, &100_000_000);
    client.queue_withdrawal(&depositor, &100_000_000);

    let bal = client.get_user_balance(&depositor);
    assert_eq!(bal.q4w.len(), 3);
}

#[test]
fn test_dequeue_withdrawal_removes_last_entry() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &200_000_000);
    client.deposit(&depositor, &200_000_000);

    client.queue_withdrawal(&depositor, &80_000_000);
    client.queue_withdrawal(&depositor, &50_000_000);
    assert_eq!(client.get_user_balance(&depositor).q4w.len(), 2);

    // Dequeue removes the newest entry (50_000_000)
    client.dequeue_withdrawal(&depositor);
    let bal = client.get_user_balance(&depositor);
    assert_eq!(bal.q4w.len(), 1);
    assert_eq!(bal.q4w.get(0).unwrap().amount, 80_000_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")] // WithdrawalQueueNotExpired
fn test_withdraw_before_expiry_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &100_000_000);
    client.deposit(&depositor, &100_000_000);
    client.queue_withdrawal(&depositor, &100_000_000);

    // Immediately try to withdraw — lock not expired
    client.withdraw(&depositor, &100_000_000);
}

#[test]
fn test_withdraw_after_expiry_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &100_000_000);
    client.deposit(&depositor, &100_000_000);
    client.queue_withdrawal(&depositor, &100_000_000);

    // Advance ledger past expiry
    env.ledger().with_mut(|li| {
        li.timestamp += Q4W_LOCK_SECONDS + 1;
    });

    client.withdraw(&depositor, &100_000_000);

    assert_eq!(client.get_total(), 0);
    let bal = client.get_user_balance(&depositor);
    assert_eq!(bal.amount, 0);
    assert_eq!(bal.q4w.len(), 0);
}

#[test]
fn test_partial_withdraw_from_q4w_entry() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &100_000_000);
    client.deposit(&depositor, &100_000_000);
    client.queue_withdrawal(&depositor, &100_000_000);

    env.ledger().with_mut(|li| {
        li.timestamp += Q4W_LOCK_SECONDS + 1;
    });

    // Withdraw only half
    client.withdraw(&depositor, &40_000_000);

    let bal = client.get_user_balance(&depositor);
    // Entry still present with reduced amount
    assert_eq!(bal.q4w.len(), 1);
    assert_eq!(bal.q4w.get(0).unwrap().amount, 60_000_000);
    assert_eq!(client.get_total(), 60_000_000);
}

#[test]
fn test_large_queue_triggers_on_ice() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let pool = env.register(mock_pool::MockPool, ());
    let (token_client, token) = create_token(&env, &admin);
    // Threshold 0 so deposit alone would be Active
    let client = create_backstop(&env, &admin, &pool, &token, 0);

    let depositor = Address::generate(&env);
    token_client.transfer(&admin, &depositor, &100_000_000);
    client.deposit(&depositor, &100_000_000);
    assert_eq!(client.get_pool_state(), PoolState::Active);

    // Queue 30% → OnIce (≥25%)
    client.queue_withdrawal(&depositor, &30_000_000);
    assert_eq!(client.get_pool_state(), PoolState::OnIce);

    // Queue 50% total → Frozen (≥50%)
    client.queue_withdrawal(&depositor, &20_000_000);
    assert_eq!(client.get_pool_state(), PoolState::Frozen);
}
