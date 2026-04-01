use soroban_sdk::{Address, Env, contract, contractimpl, panic_with_error};

use crate::admin::Admin;
use crate::error::Error;
use crate::operations::Backstop;
use crate::storage::Storage;
use crate::types::{PoolState, UserBalance};

/// Backstop contract — holds first-loss capital for the Neko lending pool.
///
/// Deployment order:
///   1. Deploy neko-pool (initialize without backstop)
///   2. Deploy neko-backstop (initialize with pool address)
///   3. Call pool.set_backstop_contract(backstop_address)
///
/// After step 3, backstop automatically pushes pool state on every
/// deposit/queue_withdrawal/dequeue_withdrawal/withdraw change.
#[contract]
pub struct NekoBackstop;

#[contractimpl]
impl NekoBackstop {
    pub fn initialize(
        env: Env,
        admin: Address,
        pool: Address,
        backstop_token: Address,
        backstop_threshold: i128,
    ) {
        if Storage::is_initialized(&env) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        Storage::set_admin(&env, &admin);
        Storage::set_pool_contract(&env, &pool);
        Storage::set_backstop_token(&env, &backstop_token);
        Storage::set_backstop_threshold(&env, backstop_threshold);
        Storage::set_backstop_total(&env, 0);
        Storage::set_backstop_queued_total(&env, 0);
    }

    // ========== Admin ==========

    pub fn set_threshold(env: Env, threshold: i128) {
        Storage::get_admin(&env).require_auth();
        Storage::set_backstop_threshold(&env, threshold);
    }

    pub fn set_backstop_token(env: Env, token: Address) {
        Storage::get_admin(&env).require_auth();
        Storage::set_backstop_token(&env, &token);
    }

    /// Step 1 of two-step admin transfer. Current admin only.
    pub fn propose_admin(env: Env, proposed: Address) {
        Admin::propose_admin(&env, &proposed);
    }

    /// Step 2: pending admin accepts (must match `propose_admin`).
    pub fn accept_admin(env: Env) {
        Admin::accept_admin(&env);
    }

    pub fn get_admin(env: Env) -> Address {
        Storage::get_admin(&env)
    }

    // ========== Depositor Functions ==========

    /// Deposit backstop tokens.
    pub fn deposit(env: Env, depositor: Address, amount: i128) -> Result<(), Error> {
        Backstop::deposit(&env, &depositor, amount)
    }

    /// Add `amount` to the withdrawal queue (up to MAX_Q4W_SIZE entries per user).
    pub fn queue_withdrawal(env: Env, depositor: Address, amount: i128) -> Result<(), Error> {
        Backstop::queue_withdrawal(&env, &depositor, amount)
    }

    /// Cancel the most recently queued withdrawal entry.
    pub fn dequeue_withdrawal(env: Env, depositor: Address) -> Result<(), Error> {
        Backstop::dequeue_withdrawal(&env, &depositor)
    }

    /// Withdraw from the oldest expired Q4W entry.
    pub fn withdraw(env: Env, depositor: Address, amount: i128) -> Result<(), Error> {
        Backstop::withdraw(&env, &depositor, amount)
    }

    // ========== Pool-facing Functions ==========

    /// Cover bad debt by reducing backstop balance.
    /// Only callable by the registered pool contract.
    pub fn cover_bad_debt(env: Env, caller: Address, amount: i128) -> Result<(), Error> {
        Backstop::cover_bad_debt(&env, &caller, amount)
    }

    // ========== View Functions ==========

    pub fn get_user_balance(env: Env, depositor: Address) -> UserBalance {
        Backstop::get_user_balance(&env, &depositor)
    }

    pub fn get_total(env: Env) -> i128 {
        Backstop::get_total(&env)
    }

    pub fn get_pool_state(env: Env) -> PoolState {
        Backstop::compute_pool_state(&env)
    }

    pub fn get_pool_contract(env: Env) -> Address {
        Storage::get_pool_contract(&env)
    }

    pub fn get_backstop_token(env: Env) -> Option<Address> {
        Storage::get_backstop_token(&env)
    }
}
