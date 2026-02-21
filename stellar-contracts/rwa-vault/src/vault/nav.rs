use soroban_sdk::Env;

use crate::adapters::AdapterClient;
use crate::common::error::Error;
use crate::common::types::{SCALAR_7, VaultStorage};

pub struct Nav;

impl Nav {
    /// Calculate total NAV = liquid_reserve + Σ adapter.a_balance(vault)
    /// Result is in deposit_token units.
    pub fn calculate(env: &Env, storage: &VaultStorage) -> Result<i128, Error> {
        let mut nav = storage.liquid_reserve;
        let vault_addr = env.current_contract_address();

        for protocol_id in storage.protocol_ids.iter() {
            if let Some(alloc) = storage.protocol_allocations.get(protocol_id) {
                if alloc.is_active {
                    let adapter = AdapterClient::new(env, &alloc.adapter);
                    let balance = adapter.a_balance(&vault_addr);
                    nav = nav.checked_add(balance).ok_or(Error::ArithmeticError)?;
                }
            }
        }

        Ok(nav)
    }

    /// Share price = NAV * SCALAR_7 / total_shares
    /// Returns SCALAR_7 (= 1.0) when no shares exist yet.
    pub fn share_price(nav: i128, total_shares: i128) -> Result<i128, Error> {
        if total_shares == 0 {
            return Ok(SCALAR_7);
        }
        nav.checked_mul(SCALAR_7)
            .ok_or(Error::ArithmeticError)?
            .checked_div(total_shares)
            .ok_or(Error::ArithmeticError)
    }

    /// Convert deposit_token amount to shares.
    /// First deposit is 1:1. Subsequent deposits are proportional to NAV.
    pub fn to_shares(amount: i128, nav: i128, total_shares: i128) -> Result<i128, Error> {
        if total_shares == 0 || nav == 0 {
            // First deposit: 1:1
            return Ok(amount);
        }
        amount
            .checked_mul(total_shares)
            .ok_or(Error::ArithmeticError)?
            .checked_div(nav)
            .ok_or(Error::ArithmeticError)
    }

    /// Convert shares to deposit_token amount.
    pub fn from_shares(shares: i128, nav: i128, total_shares: i128) -> Result<i128, Error> {
        if total_shares == 0 {
            return Ok(0);
        }
        shares
            .checked_mul(nav)
            .ok_or(Error::ArithmeticError)?
            .checked_div(total_shares)
            .ok_or(Error::ArithmeticError)
    }
}
