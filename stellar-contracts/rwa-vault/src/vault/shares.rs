use soroban_sdk::{Address, Env};

use crate::common::storage::{AllowanceStorage, BalanceStorage};

/// Internal share (vToken) mint/burn/transfer logic.
/// Does NOT emit SEP-41 events — call site is responsible.
pub struct Shares;

impl Shares {
    pub fn mint(env: &Env, to: &Address, amount: i128) {
        BalanceStorage::add(env, to, amount);
    }

    pub fn burn(env: &Env, from: &Address, amount: i128) {
        BalanceStorage::subtract(env, from, amount);
    }

    pub fn transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
        BalanceStorage::subtract(env, from, amount);
        BalanceStorage::add(env, to, amount);
    }

    pub fn transfer_from(
        env: &Env,
        spender: &Address,
        from: &Address,
        to: &Address,
        amount: i128,
    ) {
        AllowanceStorage::subtract(env, from, spender, amount);
        BalanceStorage::subtract(env, from, amount);
        BalanceStorage::add(env, to, amount);
    }
}
