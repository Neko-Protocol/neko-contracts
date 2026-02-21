use soroban_sdk::{token::TokenClient, Address, Env};

use crate::adapters::AdapterClient;
use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::{BalanceStorage, Storage};
use crate::common::types::VaultStatus;
use crate::vault::nav::Nav;
use crate::vault::shares::Shares;

pub struct Withdraw;

impl Withdraw {
    /// Burn `shares` of vTokens and receive deposit_token in return.
    ///
    /// Flow:
    /// 1. Check vault allows withdrawals (Active or Paused — but not EmergencyExit with 0 reserve)
    /// 2. Verify user has enough shares
    /// 3. Calculate token amount = shares * NAV / total_shares
    /// 4. Fulfill from liquid_reserve first, then pull from protocols if needed
    /// 5. Burn vTokens and transfer tokens to user
    pub fn execute(env: &Env, from: &Address, shares: i128) -> Result<i128, Error> {
        from.require_auth();

        if shares <= 0 {
            return Err(Error::ZeroAmount);
        }

        let mut storage = Storage::load(env);

        // Emergency exit with no liquid_reserve: nothing to withdraw
        if storage.status == VaultStatus::EmergencyExit && storage.liquid_reserve == 0 {
            return Err(Error::InsufficientLiquidity);
        }

        // Check user has enough shares
        let user_shares = BalanceStorage::get(env, from);
        if user_shares < shares {
            return Err(Error::InsufficientShares);
        }

        // Calculate NAV and amount to return
        let nav = Nav::calculate(env, &storage)?;
        let amount = Nav::from_shares(shares, nav, storage.total_shares)?;

        if amount == 0 {
            return Err(Error::ZeroAmount);
        }

        // Burn vTokens
        Shares::burn(env, from, shares);
        storage.total_shares = storage
            .total_shares
            .checked_sub(shares)
            .ok_or(Error::ArithmeticError)?;

        // Calculate how much comes from reserve vs protocols
        let actual_amount = if storage.liquid_reserve >= amount {
            storage.liquid_reserve -= amount;
            amount
        } else {
            let from_reserve = storage.liquid_reserve;
            let still_needed = amount - from_reserve;
            storage.liquid_reserve = 0;

            // Pull deficit from protocols (tokens come directly to vault address)
            let pulled = Self::pull_from_protocols(env, &storage, still_needed)?;

            from_reserve + pulled
        };

        Storage::save(env, &storage);

        // Transfer tokens to user
        let token = TokenClient::new(env, &storage.deposit_token);
        token.transfer(&env.current_contract_address(), from, &actual_amount);

        Events::withdraw(env, from, actual_amount, shares);

        Ok(actual_amount)
    }

    /// Pull `needed` tokens from active protocols (starting from the first).
    /// Adapters transfer tokens directly to the vault address.
    fn pull_from_protocols(
        env: &Env,
        storage: &crate::common::types::VaultStorage,
        needed: i128,
    ) -> Result<i128, Error> {
        let vault_addr = env.current_contract_address();
        let mut remaining = needed;
        let mut pulled = 0i128;

        for protocol_id in storage.protocol_ids.iter() {
            if remaining <= 0 {
                break;
            }
            if let Some(alloc) = storage.protocol_allocations.get(protocol_id) {
                if !alloc.is_active {
                    continue;
                }
                let adapter = AdapterClient::new(env, &alloc.adapter);
                let balance = adapter.a_balance(&vault_addr);
                let to_withdraw = remaining.min(balance);
                if to_withdraw > 0 {
                    let actual = adapter.a_withdraw(&to_withdraw, &vault_addr);
                    pulled = pulled
                        .checked_add(actual)
                        .ok_or(Error::ArithmeticError)?;
                    remaining = remaining
                        .checked_sub(actual)
                        .ok_or(Error::ArithmeticError)?;
                }
            }
        }

        Ok(pulled)
    }
}
