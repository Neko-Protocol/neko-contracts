use soroban_sdk::{Address, Env, contract, contractimpl, panic_with_error};

use crate::error::Error;
use crate::operations::Backstop;
use crate::storage::Storage;
use crate::types::{BackstopDeposit, PoolState};

/// Backstop contract — holds first-loss capital for the Neko lending pool.
///
/// Deployment order:
///   1. Deploy neko-pool (initialize without backstop)
///   2. Deploy neko-backstop (initialize with pool address)
///   3. Call pool.set_backstop_contract(backstop_address)
///
/// After step 3, backstop automatically pushes pool state on every deposit/withdraw.
#[contract]
pub struct NekoBackstop;

#[contractimpl]
impl NekoBackstop {
    /// Initialize the backstop contract.
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

    /// Update the backstop threshold. Admin-only.
    pub fn set_threshold(env: Env, threshold: i128) {
        let admin = Storage::get_admin(&env);
        admin.require_auth();
        Storage::set_backstop_threshold(&env, threshold);
    }

    /// Update the backstop token address. Admin-only.
    pub fn set_backstop_token(env: Env, token: Address) {
        let admin = Storage::get_admin(&env);
        admin.require_auth();
        Storage::set_backstop_token(&env, &token);
    }

    // ========== Depositor Functions ==========

    /// Deposit backstop tokens.
    pub fn deposit(env: Env, depositor: Address, amount: i128) -> Result<(), Error> {
        Backstop::deposit(&env, &depositor, amount)
    }

    /// Enter the 17-day withdrawal queue.
    pub fn initiate_withdrawal(env: Env, depositor: Address, amount: i128) -> Result<(), Error> {
        Backstop::initiate_withdrawal(&env, &depositor, amount)
    }

    /// Withdraw after queue period has elapsed.
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

    /// Get deposit info for a depositor.
    pub fn get_deposit(env: Env, depositor: Address) -> BackstopDeposit {
        Backstop::get_deposit(&env, &depositor)
    }

    /// Get total backstop balance.
    pub fn get_total(env: Env) -> i128 {
        Backstop::get_total(&env)
    }

    /// Compute the pool state derived from current backstop metrics.
    pub fn get_pool_state(env: Env) -> PoolState {
        Backstop::compute_pool_state(&env)
    }

    /// Get the registered pool contract address.
    pub fn get_pool_contract(env: Env) -> Address {
        Storage::get_pool_contract(&env)
    }

    /// Get the backstop token address.
    pub fn get_backstop_token(env: Env) -> Option<Address> {
        Storage::get_backstop_token(&env)
    }
}
