use soroban_sdk::{Address, Env, IntoVal, Symbol, Val, Vec, assert_with_error, token::TokenClient};

use crate::error::Error;
use crate::events::Events;
use crate::storage::Storage;
use crate::types::{BACKSTOP_WITHDRAWAL_QUEUE_SECONDS, BackstopDeposit, PoolState};

/// Backstop operations — first-loss capital reserve for the Neko lending pool.
pub struct Backstop;

impl Backstop {
    /// Deposit backstop tokens.
    pub fn deposit(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();
        assert_with_error!(env, amount > 0, Error::NotPositive);

        let token_address = Storage::get_backstop_token(env).ok_or(Error::TokenContractNotSet)?;
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(depositor, &env.current_contract_address(), &amount);

        // Preserve any existing queue entry on top-up deposits.
        let mut deposit = Storage::get_backstop_deposit(env, depositor).unwrap_or(BackstopDeposit {
            amount: 0,
            deposited_at: env.ledger().timestamp(),
            queued_amount: 0,
            queued_at: None,
        });
        deposit.amount = deposit
            .amount
            .checked_add(amount)
            .ok_or(Error::ArithmeticError)?;
        deposit.deposited_at = env.ledger().timestamp();
        Storage::set_backstop_deposit(env, depositor, &deposit);

        let new_total = Storage::get_backstop_total(env)
            .checked_add(amount)
            .ok_or(Error::ArithmeticError)?;
        Storage::set_backstop_total(env, new_total);

        Events::deposit(env, depositor, amount, new_total);
        Self::push_pool_state(env);
        Ok(())
    }

    /// Initiate a withdrawal — enters the 17-day queue.
    pub fn initiate_withdrawal(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();
        assert_with_error!(env, amount > 0, Error::NotPositive);

        let mut deposit = Storage::get_backstop_deposit(env, depositor)
            .ok_or(Error::InsufficientBackstopDeposit)?;

        if deposit.amount < amount {
            return Err(Error::InsufficientBackstopDeposit);
        }
        if deposit.queued_amount > 0 {
            return Err(Error::WithdrawalQueueActive);
        }

        let now = env.ledger().timestamp();
        deposit.queued_amount = amount;
        deposit.queued_at = Some(now);
        Storage::set_backstop_deposit(env, depositor, &deposit);

        let queued_total = Storage::get_backstop_queued_total(env)
            .checked_add(amount)
            .ok_or(Error::ArithmeticError)?;
        Storage::set_backstop_queued_total(env, queued_total);

        Events::withdrawal_queued(env, depositor, amount, now);
        Self::push_pool_state(env);
        Ok(())
    }

    /// Withdraw from backstop after the queue lock period has elapsed.
    pub fn withdraw(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();
        assert_with_error!(env, amount > 0, Error::NotPositive);

        let mut deposit = Storage::get_backstop_deposit(env, depositor)
            .ok_or(Error::InsufficientBackstopDeposit)?;

        if deposit.queued_amount == 0 {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        let queued_at = deposit.queued_at.ok_or(Error::WithdrawalQueueNotExpired)?;
        if env.ledger().timestamp() < queued_at + BACKSTOP_WITHDRAWAL_QUEUE_SECONDS {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        if amount > deposit.queued_amount || amount > deposit.amount {
            return Err(Error::InsufficientBackstopDeposit);
        }

        // Clear the full queued_amount slot regardless of partial withdraw.
        let queued_total = Storage::get_backstop_queued_total(env);
        Storage::set_backstop_queued_total(
            env,
            queued_total.saturating_sub(deposit.queued_amount),
        );

        deposit.amount -= amount;
        deposit.queued_amount = 0;
        deposit.queued_at = None;
        Storage::set_backstop_deposit(env, depositor, &deposit);

        let new_total = Storage::get_backstop_total(env) - amount;
        Storage::set_backstop_total(env, new_total);

        let token_address = Storage::get_backstop_token(env).ok_or(Error::TokenContractNotSet)?;
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(&env.current_contract_address(), depositor, &amount);

        Events::withdraw(env, depositor, amount, new_total);
        Self::push_pool_state(env);
        Ok(())
    }

    /// Cover bad debt by reducing backstop balance.
    /// Only callable by the registered pool contract.
    pub fn cover_bad_debt(env: &Env, caller: &Address, amount: i128) -> Result<(), Error> {
        let pool = Storage::get_pool_contract(env);
        if caller != &pool {
            return Err(Error::NotAuthorized);
        }
        caller.require_auth();

        let total = Storage::get_backstop_total(env);
        if total < amount {
            return Err(Error::BadDebtNotCovered);
        }

        let new_total = total - amount;
        Storage::set_backstop_total(env, new_total);

        Events::bad_debt_covered(env, amount, new_total);
        Self::push_pool_state(env);
        Ok(())
    }

    // =========================================================================
    // View helpers
    // =========================================================================

    pub fn get_deposit(env: &Env, depositor: &Address) -> BackstopDeposit {
        Storage::get_backstop_deposit(env, depositor).unwrap_or(BackstopDeposit {
            amount: 0,
            deposited_at: 0,
            queued_amount: 0,
            queued_at: None,
        })
    }

    pub fn get_total(env: &Env) -> i128 {
        Storage::get_backstop_total(env)
    }

    /// Compute pool state from current backstop metrics (O(1) — uses global counters).
    pub fn compute_pool_state(env: &Env) -> PoolState {
        let backstop_total = Storage::get_backstop_total(env);
        let backstop_threshold = Storage::get_backstop_threshold(env);
        let queued_total = Storage::get_backstop_queued_total(env);

        let queued_percentage = if backstop_total > 0 {
            (queued_total * 10_000) / backstop_total
        } else {
            0
        };

        if queued_percentage >= 5000 {
            PoolState::Frozen
        } else if queued_percentage >= 2500 || backstop_total < backstop_threshold {
            PoolState::OnIce
        } else {
            PoolState::Active
        }
    }

    // =========================================================================
    // Cross-contract — push pool state after each backstop change
    // =========================================================================

    /// Compute the current pool state and push it to the pool contract.
    ///
    /// Calls `pool.update_pool_state_from_backstop(state: u32)` where:
    ///   0 = Active, 1 = OnIce, 2 = Frozen
    ///
    /// The pool validates that the caller is the registered backstop address,
    /// so no extra auth is required here.
    fn push_pool_state(env: &Env) {
        let new_state = Self::compute_pool_state(env);
        let state_u32: u32 = match new_state {
            PoolState::Active => 0,
            PoolState::OnIce => 1,
            PoolState::Frozen => 2,
        };

        let pool = Storage::get_pool_contract(env);
        let func = Symbol::new(env, "update_pool_state_from_backstop");
        let mut args: Vec<Val> = Vec::new(env);
        args.push_back(state_u32.into_val(env));
        let _: () = env.invoke_contract(&pool, &func, args);

        Events::pool_state_updated(env, new_state);
    }
}
