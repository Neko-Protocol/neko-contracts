use soroban_sdk::{Address, Env, contractevent};

use crate::types::PoolState;

#[contractevent]
pub struct BackstopDepositEvent {
    pub depositor: Address,
    pub amount: i128,
    pub total: i128,
}

#[contractevent]
pub struct BackstopQueueEvent {
    pub depositor: Address,
    pub amount: i128,
    /// Expiration timestamp for queue_withdrawal; 0 for dequeue_withdrawal.
    pub exp: u64,
}

#[contractevent]
pub struct BackstopWithdrawEvent {
    pub depositor: Address,
    pub amount: i128,
    pub total: i128,
}

#[contractevent]
pub struct PoolStateUpdatedEvent {
    pub new_state: PoolState,
}

#[contractevent]
pub struct BadDebtCoveredEvent {
    pub amount: i128,
    pub new_total: i128,
}

pub struct Events;

impl Events {
    pub fn deposit(env: &Env, depositor: &Address, amount: i128, total: i128) {
        BackstopDepositEvent {
            depositor: depositor.clone(),
            amount,
            total,
        }
        .publish(env);
    }

    pub fn withdrawal_queued(env: &Env, depositor: &Address, amount: i128, exp: u64) {
        BackstopQueueEvent {
            depositor: depositor.clone(),
            amount,
            exp,
        }
        .publish(env);
    }

    pub fn withdrawal_dequeued(env: &Env, depositor: &Address, amount: i128) {
        BackstopQueueEvent {
            depositor: depositor.clone(),
            amount,
            exp: 0,
        }
        .publish(env);
    }

    pub fn withdraw(env: &Env, depositor: &Address, amount: i128, total: i128) {
        BackstopWithdrawEvent {
            depositor: depositor.clone(),
            amount,
            total,
        }
        .publish(env);
    }

    pub fn pool_state_updated(env: &Env, new_state: PoolState) {
        PoolStateUpdatedEvent { new_state }.publish(env);
    }

    pub fn bad_debt_covered(env: &Env, amount: i128, new_total: i128) {
        BadDebtCoveredEvent { amount, new_total }.publish(env);
    }
}
