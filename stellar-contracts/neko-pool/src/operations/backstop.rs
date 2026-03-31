use soroban_sdk::{Address, Env, assert_with_error, token::TokenClient};

use crate::common::error::Error;
use crate::common::storage::Storage;
use crate::common::types::{
    BACKSTOP_WITHDRAWAL_QUEUE_SECONDS, MAX_BACKSTOP_QUEUE_SIZE, PoolState,
};

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

        // Update backstop deposit
        let mut deposit =
            Storage::get_backstop_deposit(env, depositor).unwrap_or(
                crate::common::types::BackstopDeposit {
                    amount: 0,
                    deposited_at: env.ledger().timestamp(),
                    in_withdrawal_queue: false,
                    queued_at: None,
                },
            );

        deposit.amount += amount;
        deposit.deposited_at = env.ledger().timestamp();
        // Do not touch in_withdrawal_queue or queued_at — an active queue entry
        // must survive a top-up deposit so the user can still withdraw after the
        // lock period expires.

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

        if deposit.in_withdrawal_queue {
            return Err(Error::WithdrawalQueueActive);
        }

        let mut queue = Storage::get_withdrawal_queue(env);

        if queue.len() >= MAX_BACKSTOP_QUEUE_SIZE {
            return Err(Error::WithdrawalQueueFull);
        }

        // Add to withdrawal queue
        let withdrawal_request = crate::common::types::WithdrawalRequest {
            address: depositor.clone(),
            amount,
            queued_at: env.ledger().timestamp(),
        };

        queue.push_back(withdrawal_request);
        deposit.in_withdrawal_queue = true;
        deposit.queued_at = Some(env.ledger().timestamp());

        Storage::set_backstop_deposit(env, depositor, &deposit);
        Storage::set_withdrawal_queue(env, &queue);

        // Update pool state
        Self::update_pool_state(env)?;

        Ok(())
    }

    /// Withdraw from backstop (after queue period)
    pub fn withdraw(env: &Env, depositor: &Address, amount: i128) -> Result<(), Error> {
        depositor.require_auth();

        assert_with_error!(env, amount > 0, Error::NotPositive);

        let mut deposit = Storage::get_backstop_deposit(env, depositor)
            .ok_or(Error::InsufficientBackstopDeposit)?;

        if !deposit.in_withdrawal_queue {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        let queued_at = deposit.queued_at.ok_or(Error::WithdrawalQueueNotExpired)?;
        let current_time = env.ledger().timestamp();

        if current_time < queued_at + BACKSTOP_WITHDRAWAL_QUEUE_SECONDS {
            return Err(Error::WithdrawalQueueNotExpired);
        }

        if deposit.amount < amount {
            return Err(Error::InsufficientBackstopDeposit);
        }

        // Update deposit
        let saved_queued_at = deposit.queued_at;
        deposit.amount -= amount;
        deposit.in_withdrawal_queue = false;
        deposit.queued_at = None;

        // Remove the matching entry from the global withdrawal queue so that
        // update_pool_state() does not count stale entries.
        let old_queue = Storage::get_withdrawal_queue(env);
        let mut updated_queue = soroban_sdk::Vec::new(env);
        for req in old_queue.iter() {
            if !(req.address == *depositor && Some(req.queued_at) == saved_queued_at) {
                updated_queue.push_back(req);
            }
        }

        // Get token address before updating storage
        let token_address = Storage::get_backstop_token(env).ok_or(Error::TokenContractNotSet)?;

        Storage::set_backstop_deposit(env, depositor, &deposit);
        Storage::set_backstop_total(env, Storage::get_backstop_total(env) - amount);
        Storage::set_withdrawal_queue(env, &updated_queue);

        // Transfer tokens from contract to depositor
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(&env.current_contract_address(), depositor, &amount);

        // Update pool state
        Self::update_pool_state(env)?;

        Ok(())
    }

    /// Update pool state based on backstop status
    fn update_pool_state(env: &Env) -> Result<(), Error> {
        let queue = Storage::get_withdrawal_queue(env);
        let backstop_total = Storage::get_backstop_total(env);
        let backstop_threshold = Storage::get_backstop_threshold(env);

        // Calculate queued withdrawals percentage
        let queued_withdrawals: i128 = queue.iter().map(|req| req.amount).sum();

        let queued_percentage = if backstop_total > 0 {
            (queued_withdrawals * 10_000) / backstop_total
        } else {
            0
        };

        let new_state = if queued_percentage >= 5000 {
            // 50% or more in withdrawal queue
            PoolState::Frozen
        } else if queued_percentage >= 2500 || backstop_total < backstop_threshold {
            // 25% or more in queue, or below threshold
            PoolState::OnIce
        } else {
            // Healthy
            PoolState::Active
        };

        // Only update if state changed (direct storage call — no admin auth needed for internal state updates)
        let current_state = Storage::get_pool_state(env);
        if current_state != new_state {
            Storage::set_pool_state(env, new_state);
        }

        Ok(())
    }

    /// Get backstop deposit for a depositor
    pub fn get_deposit(env: &Env, depositor: &Address) -> crate::common::types::BackstopDeposit {
        Storage::get_backstop_deposit(env, depositor).unwrap_or(
            crate::common::types::BackstopDeposit {
                amount: 0,
                deposited_at: 0,
                in_withdrawal_queue: false,
                queued_at: None,
            },
        )
    }

    /// Get total backstop deposits
    pub fn get_total(env: &Env) -> i128 {
        Storage::get_backstop_total(env)
    }
}
