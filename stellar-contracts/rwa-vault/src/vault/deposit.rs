use soroban_sdk::{Address, Env, token::TokenClient};

use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::Storage;
use crate::common::types::VaultStatus;
use crate::vault::nav::Nav;
use crate::vault::shares::Shares;

pub struct Deposit;

impl Deposit {
    /// Deposit `amount` of deposit_token and receive vTokens in return.
    ///
    /// Flow:
    /// 1. Check vault is Active
    /// 2. Transfer deposit_token from user to vault
    /// 3. Calculate shares proportional to current NAV
    /// 4. Update liquid_reserve and total_shares
    /// 5. Mint vTokens to user
    pub fn execute(env: &Env, from: &Address, amount: i128) -> Result<i128, Error> {
        from.require_auth();

        if amount <= 0 {
            return Err(Error::ZeroAmount);
        }

        let mut storage = Storage::load(env);

        if storage.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        // Transfer deposit_token from user to vault
        let token = TokenClient::new(env, &storage.deposit_token);
        token.transfer(from, &env.current_contract_address(), &amount);

        // Calculate NAV *before* adding this deposit
        let nav = Nav::calculate(env, &storage)?;

        // Calculate shares to mint
        let shares = Nav::to_shares(amount, nav, storage.total_shares)?;

        if shares == 0 {
            return Err(Error::ZeroAmount);
        }

        // Update state
        storage.liquid_reserve = storage
            .liquid_reserve
            .checked_add(amount)
            .ok_or(Error::ArithmeticError)?;

        storage.total_shares = storage
            .total_shares
            .checked_add(shares)
            .ok_or(Error::ArithmeticError)?;

        Storage::save(env, &storage);

        // Mint vTokens to user
        Shares::mint(env, from, shares);

        Events::deposit(env, from, amount, shares);

        Ok(shares)
    }
}
