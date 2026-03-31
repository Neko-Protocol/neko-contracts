use soroban_sdk::{Address, contractevent};

use crate::types::PoolState;

#[contractevent]
pub struct BackstopDepositEvent {
    pub depositor: Address,
    pub amount: i128,
    pub total: i128,
}

#[contractevent]
pub struct BackstopWithdrawalQueuedEvent {
    pub depositor: Address,
    pub amount: i128,
    pub queued_at: u64,
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
    pub fn deposit(env: &soroban_sdk::Env, depositor: &Address, amount: i128, total: i128) {
        BackstopDepositEvent {
            depositor: depositor.clone(),
            amount,
            total,
        }
        .publish(env);
    }

    pub fn withdrawal_queued(env: &soroban_sdk::Env, depositor: &Address, amount: i128, queued_at: u64) {
        BackstopWithdrawalQueuedEvent {
            depositor: depositor.clone(),
            amount,
            queued_at,
        }
        .publish(env);
    }

    pub fn withdraw(env: &soroban_sdk::Env, depositor: &Address, amount: i128, total: i128) {
        BackstopWithdrawEvent {
            depositor: depositor.clone(),
            amount,
            total,
        }
        .publish(env);
    }

    pub fn pool_state_updated(env: &soroban_sdk::Env, new_state: PoolState) {
        PoolStateUpdatedEvent { new_state }.publish(env);
    }

    pub fn bad_debt_covered(env: &soroban_sdk::Env, amount: i128, new_total: i128) {
        BadDebtCoveredEvent { amount, new_total }.publish(env);
    }
}
