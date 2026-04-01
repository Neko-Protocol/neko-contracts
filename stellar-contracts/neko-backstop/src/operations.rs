use soroban_sdk::{Address, Env, IntoVal, Symbol, Val, Vec, assert_with_error, token::TokenClient};

use crate::error::Error;
use crate::events::Events;
use crate::storage::Storage;
use crate::types::{MAX_Q4W_SIZE, Q4W, Q4W_LOCK_SECONDS, PoolState, UserBalance};

pub struct Backstop;

impl Backstop {
    // =========================================================================
    // Deposit
    // =========================================================================

    pub fn deposit(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();
        assert_with_error!(env, amount > 0, Error::NotPositive);

        let token_address = Storage::get_backstop_token(env).ok_or(Error::TokenContractNotSet)?;
        TokenClient::new(env, &token_address).transfer(
            depositor,
            &env.current_contract_address(),
            &amount,
        );

        let mut balance = Storage::get_user_balance(env, depositor);
        balance.amount = balance
            .amount
            .checked_add(amount)
            .ok_or(Error::ArithmeticError)?;
        Storage::set_user_balance(env, depositor, &balance);

        let new_total = Storage::get_backstop_total(env)
            .checked_add(amount)
            .ok_or(Error::ArithmeticError)?;
        Storage::set_backstop_total(env, new_total);

        Events::deposit(env, depositor, amount, new_total);
        Self::push_pool_state(env);
        Ok(())
    }

    // =========================================================================
    // Queue for Withdrawal
    // =========================================================================

    /// Enter the withdrawal queue for `amount` tokens.
    ///
    /// Multiple entries are allowed up to MAX_Q4W_SIZE. Each entry carries its
    /// own expiration (`now + Q4W_LOCK_SECONDS`), so queuing again before the
    /// first entry expires is valid — each is tracked independently.
    pub fn queue_withdrawal(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();
        assert_with_error!(env, amount > 0, Error::NotPositive);

        let mut balance = Storage::get_user_balance(env, depositor);

        // Check sufficient non-queued balance.
        // Total queued = sum of existing q4w entries + new amount.
        let already_queued: i128 = balance.q4w.iter().map(|e| e.amount).sum();
        let available = balance
            .amount
            .checked_sub(already_queued)
            .ok_or(Error::ArithmeticError)?;
        if amount > available {
            return Err(Error::InsufficientBackstopDeposit);
        }

        if balance.q4w.len() >= MAX_Q4W_SIZE {
            return Err(Error::WithdrawalQueueFull);
        }

        let exp = env
            .ledger()
            .timestamp()
            .checked_add(Q4W_LOCK_SECONDS)
            .ok_or(Error::ArithmeticError)?;
        balance.q4w.push_back(Q4W { amount, exp });
        Storage::set_user_balance(env, depositor, &balance);

        let new_queued = Storage::get_backstop_queued_total(env)
            .checked_add(amount)
            .ok_or(Error::ArithmeticError)?;
        Storage::set_backstop_queued_total(env, new_queued);

        Events::withdrawal_queued(env, depositor, amount, exp);
        Self::push_pool_state(env);
        Ok(())
    }

    /// Cancel the most recently queued withdrawal (tail of the Vec).
    pub fn dequeue_withdrawal(env: &Env, depositor: &Address) -> Result<(), Error> {
        depositor.require_auth();

        let mut balance = Storage::get_user_balance(env, depositor);
        if balance.q4w.is_empty() {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        // Pop the most recent (last) entry.
        let removed = balance.q4w.pop_back().ok_or(Error::WithdrawalQueueNotExpired)?;
        Storage::set_user_balance(env, depositor, &balance);

        let queued_total = Storage::get_backstop_queued_total(env);
        Storage::set_backstop_queued_total(
            env,
            queued_total.saturating_sub(removed.amount),
        );

        Events::withdrawal_dequeued(env, depositor, removed.amount);
        Self::push_pool_state(env);
        Ok(())
    }

    // =========================================================================
    // Withdraw
    // =========================================================================

    /// Withdraw `amount` from the oldest expired Q4W entry.
    ///
    /// A single call can only draw from one Q4W entry. If the head entry is
    /// expired and `amount <= entry.amount`, it succeeds. The entry is removed
    /// if fully consumed, or its amount is reduced for a partial withdrawal.
    pub fn withdraw(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();
        assert_with_error!(env, amount > 0, Error::NotPositive);

        let mut balance = Storage::get_user_balance(env, depositor);
        if balance.q4w.is_empty() {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        // Consume from the oldest (front) entry.
        let head = balance.q4w.get(0).ok_or(Error::WithdrawalQueueNotExpired)?;
        if env.ledger().timestamp() < head.exp {
            return Err(Error::WithdrawalQueueNotExpired);
        }
        if amount > head.amount {
            return Err(Error::InsufficientBackstopDeposit);
        }

        // Update or remove the head entry.
        let remaining = head.amount - amount;
        if remaining == 0 {
            balance.q4w.pop_front();
        } else {
            balance.q4w.set(0, Q4W { amount: remaining, exp: head.exp });
        }

        balance.amount = balance
            .amount
            .checked_sub(amount)
            .ok_or(Error::ArithmeticError)?;
        Storage::set_user_balance(env, depositor, &balance);

        let new_total = Storage::get_backstop_total(env) - amount;
        Storage::set_backstop_total(env, new_total);

        let queued_total = Storage::get_backstop_queued_total(env);
        Storage::set_backstop_queued_total(env, queued_total.saturating_sub(amount));

        let token_address = Storage::get_backstop_token(env).ok_or(Error::TokenContractNotSet)?;
        TokenClient::new(env, &token_address).transfer(
            &env.current_contract_address(),
            depositor,
            &amount,
        );

        Events::withdraw(env, depositor, amount, new_total);
        Self::push_pool_state(env);
        Ok(())
    }

    // =========================================================================
    // Pool-facing
    // =========================================================================

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
        Storage::set_backstop_total(env, total - amount);

        Events::bad_debt_covered(env, amount, total - amount);
        Self::push_pool_state(env);
        Ok(())
    }

    // =========================================================================
    // View helpers
    // =========================================================================

    pub fn get_user_balance(env: &Env, depositor: &Address) -> UserBalance {
        Storage::get_user_balance(env, depositor)
    }

    pub fn get_total(env: &Env) -> i128 {
        Storage::get_backstop_total(env)
    }

    pub fn compute_pool_state(env: &Env) -> PoolState {
        let backstop_total = Storage::get_backstop_total(env);
        let backstop_threshold = Storage::get_backstop_threshold(env);
        let queued_total = Storage::get_backstop_queued_total(env);

        let queued_pct = if backstop_total > 0 {
            (queued_total * 10_000) / backstop_total
        } else {
            0
        };

        if queued_pct >= 5_000 {
            PoolState::Frozen
        } else if queued_pct >= 2_500 || backstop_total < backstop_threshold {
            PoolState::OnIce
        } else {
            PoolState::Active
        }
    }

    // =========================================================================
    // Cross-contract — push pool state after every state-changing operation
    // =========================================================================

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
        args.push_back(env.current_contract_address().into_val(env));
        args.push_back(state_u32.into_val(env));
        let _: () = env.invoke_contract(&pool, &func, args);

        Events::pool_state_updated(env, new_state);
    }
}
