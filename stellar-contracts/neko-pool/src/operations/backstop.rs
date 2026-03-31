use soroban_sdk::{Address, Env, assert_with_error, token::TokenClient};

use crate::common::error::Error;
use crate::common::storage::Storage;
use crate::common::types::{BACKSTOP_WITHDRAWAL_QUEUE_SECONDS, BackstopDeposit, PoolState};

/// Backstop Module for first-loss capital
pub struct Backstop;

impl Backstop {
    /// Deposit to backstop
    pub fn deposit(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();

        assert_with_error!(env, amount > 0, Error::NotPositive);

        // Transfer tokens from depositor to contract
        let token_address = Storage::get_backstop_token(env).ok_or(Error::TokenContractNotSet)?;
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(depositor, &env.current_contract_address(), &amount);

        // Update backstop deposit — preserve queued_amount and queued_at so an
        // active queue entry survives a top-up deposit.
        let mut deposit = Storage::get_backstop_deposit(env, depositor).unwrap_or(BackstopDeposit {
            amount: 0,
            deposited_at: env.ledger().timestamp(),
            queued_amount: 0,
            queued_at: None,
        });

        deposit.amount += amount;
        deposit.deposited_at = env.ledger().timestamp();

        Storage::set_backstop_deposit(env, depositor, &deposit);
        Storage::set_backstop_total(env, Storage::get_backstop_total(env) + amount);

        // Update pool state based on backstop
        Self::update_pool_state(env)?;

        Ok(())
    }

    /// Initiate withdrawal from backstop (enters queue)
    pub fn initiate_withdrawal(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();

        assert_with_error!(env, amount > 0, Error::NotPositive);

        let mut deposit = Storage::get_backstop_deposit(env, depositor)
            .ok_or(Error::InsufficientBackstopDeposit)?;

        if deposit.amount < amount {
            return Err(Error::InsufficientBackstopDeposit);
        }

        // Only one pending withdrawal per depositor at a time
        if deposit.queued_amount > 0 {
            return Err(Error::WithdrawalQueueActive);
        }

        // Record queue entry on the deposit; bump global counter
        deposit.queued_amount = amount;
        deposit.queued_at = Some(env.ledger().timestamp());

        Storage::set_backstop_deposit(env, depositor, &deposit);

        let queued_total = Storage::get_backstop_queued_total(env);
        Storage::set_backstop_queued_total(
            env,
            queued_total.checked_add(amount).ok_or(Error::ArithmeticError)?,
        );

        // Update pool state
        Self::update_pool_state(env)?;

        Ok(())
    }

    /// Withdraw from backstop (after queue lock period)
    pub fn withdraw(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();

        assert_with_error!(env, amount > 0, Error::NotPositive);

        let mut deposit = Storage::get_backstop_deposit(env, depositor)
            .ok_or(Error::InsufficientBackstopDeposit)?;

        // Must have an active queue entry
        if deposit.queued_amount == 0 {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        let queued_at = deposit.queued_at.ok_or(Error::WithdrawalQueueNotExpired)?;
        let current_time = env.ledger().timestamp();

        if current_time < queued_at + BACKSTOP_WITHDRAWAL_QUEUE_SECONDS {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        // Cannot withdraw more than queued or held
        if amount > deposit.queued_amount || amount > deposit.amount {
            return Err(Error::InsufficientBackstopDeposit);
        }

        // Deduct from global queued counter (clear entire queued_amount slot)
        let queued_total = Storage::get_backstop_queued_total(env);
        Storage::set_backstop_queued_total(
            env,
            queued_total.saturating_sub(deposit.queued_amount),
        );

        // Update deposit — clear queue slot regardless of partial vs full withdraw
        deposit.amount -= amount;
        deposit.queued_amount = 0;
        deposit.queued_at = None;

        let token_address = Storage::get_backstop_token(env).ok_or(Error::TokenContractNotSet)?;

        Storage::set_backstop_deposit(env, depositor, &deposit);
        Storage::set_backstop_total(env, Storage::get_backstop_total(env) - amount);

        // Transfer tokens from contract to depositor
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(&env.current_contract_address(), depositor, &amount);

        // Update pool state
        Self::update_pool_state(env)?;

        Ok(())
    }

    /// Update pool state based on backstop health (O(1) — uses global counter)
    fn update_pool_state(env: &Env) -> Result<(), Error> {
        let backstop_total = Storage::get_backstop_total(env);
        let backstop_threshold = Storage::get_backstop_threshold(env);
        let queued_total = Storage::get_backstop_queued_total(env);

        let queued_percentage = if backstop_total > 0 {
            (queued_total * 10_000) / backstop_total
        } else {
            0
        };

        let new_state = if queued_percentage >= 5000 {
            // 50% or more queued for withdrawal
            PoolState::Frozen
        } else if queued_percentage >= 2500 || backstop_total < backstop_threshold {
            // 25%+ queued, or below minimum threshold
            PoolState::OnIce
        } else {
            PoolState::Active
        };

        let current_state = Storage::get_pool_state(env);
        if current_state != new_state {
            Storage::set_pool_state(env, new_state);
        }

        Ok(())
    }

    /// Get backstop deposit for a depositor
    pub fn get_deposit(env: &Env, depositor: &Address) -> BackstopDeposit {
        Storage::get_backstop_deposit(env, depositor).unwrap_or(BackstopDeposit {
            amount: 0,
            deposited_at: 0,
            queued_amount: 0,
            queued_at: None,
        })
    }

    /// Get total backstop deposits
    pub fn get_total(env: &Env) -> i128 {
        Storage::get_backstop_total(env)
    }
}
