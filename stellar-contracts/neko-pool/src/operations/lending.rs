use soroban_sdk::{Address, Env, Symbol, assert_with_error, token::TokenClient};

use crate::admin::Admin;
use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::reserve_cache::ReserveCache;
use crate::common::storage::Storage;
use crate::common::types::{self, PoolState, SCALAR_7};
use crate::operations::interest::Interest;

/// Lending functions for bTokens
pub struct Lending;

impl Lending {
    /// Deposit crypto asset to the pool and receive bTokens
    pub fn deposit(
        env: &Env,
        lender: &Address,
        asset: &Symbol,
        amount: i128,
    ) -> Result<i128, Error> {
        lender.require_auth();

        assert_with_error!(env, amount > 0, Error::NotPositive);

        // Check pool state
        let pool_state = Admin::get_pool_state(env);
        if matches!(pool_state, PoolState::Frozen) {
            return Err(Error::PoolFrozen);
        }

        // Check reserve is enabled and supply cap
        if let Some(params) = Storage::get_interest_rate_params(env, asset) {
            if !params.enabled {
                return Err(Error::ReserveDisabled);
            }
            if params.supply_cap > 0 {
                let current_balance = Storage::get_pool_balance(env, asset);
                if current_balance + amount > params.supply_cap {
                    return Err(Error::SupplyCapExceeded);
                }
            }
        }

        let mut cache = ReserveCache::new(env);
        let mut reserve = Interest::accrue_interest(env, asset, &mut cache)?;
        let b_token_rate = reserve.b_rate;

        // Calculate bTokens with rounding down
        // This favors the protocol by minting fewer bTokens
        let b_tokens = types::rounding::to_b_token_down(env, amount, b_token_rate)?;

        // Transfer asset from lender to pool
        let token_address =
            Storage::get_token_contract(env, asset).ok_or(Error::TokenContractNotSet)?;
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(lender, env.current_contract_address(), &amount);

        // Update pool balance
        let current_balance = Storage::get_pool_balance(env, asset);
        Storage::set_pool_balance(env, asset, current_balance + amount);

        reserve.b_supply = reserve
            .b_supply
            .checked_add(b_tokens)
            .ok_or(Error::ArithmeticError)?;
        cache.set_reserve(env, asset, &reserve);

        // Update lender's bToken balance
        let current_balance = Storage::get_b_token_balance(env, lender, asset);
        Storage::set_b_token_balance(env, lender, asset, current_balance + b_tokens);

        // Emit event
        Events::deposit(env, lender, asset, amount, b_tokens);

        cache.flush(env);
        Ok(b_tokens)
    }

    /// Withdraw crypto asset from the pool by burning bTokens
    pub fn withdraw(
        env: &Env,
        lender: &Address,
        asset: &Symbol,
        b_tokens: i128,
    ) -> Result<i128, Error> {
        lender.require_auth();

        assert_with_error!(env, b_tokens > 0, Error::NotPositive);

        // Check pool state
        let pool_state = Admin::get_pool_state(env);
        if matches!(pool_state, PoolState::Frozen) {
            return Err(Error::PoolFrozen);
        }

        let mut cache = ReserveCache::new(env);
        let mut reserve = Interest::accrue_interest(env, asset, &mut cache)?;

        // Get current lender balance and adjust if user tries to withdraw more than they have
        let lender_balance = Storage::get_b_token_balance(env, lender, asset);
        let b_tokens_to_burn = if b_tokens > lender_balance {
            lender_balance
        } else {
            b_tokens
        };

        if b_tokens_to_burn == 0 {
            return Err(Error::InsufficientBTokenBalance);
        }

        let b_token_rate = reserve.b_rate;

        // Underlying out: floor favors protocol (user receives slightly less for burning bTokens)
        let amount = types::rounding::to_underlying_from_b_token(env, b_tokens_to_burn, b_token_rate)?;

        // Check pool has enough balance
        let pool_balance = Storage::get_pool_balance(env, asset);
        if pool_balance < amount {
            return Err(Error::InsufficientPoolBalance);
        }

        reserve.b_supply = reserve
            .b_supply
            .checked_sub(b_tokens_to_burn)
            .ok_or(Error::ArithmeticError)?;
        cache.set_reserve(env, asset, &reserve);

        // Update lender's bToken balance
        Storage::set_b_token_balance(env, lender, asset, lender_balance - b_tokens_to_burn);

        // Update pool balance
        Storage::set_pool_balance(env, asset, pool_balance - amount);

        // Verify utilization is below 100% AFTER updating supply (7 decimals)
        let utilization = Interest::calculate_utilization(env, asset, &mut cache)?;
        if utilization >= SCALAR_7 {
            return Err(Error::InvalidUtilRate);
        }

        // Transfer asset from pool to lender
        let token_address =
            Storage::get_token_contract(env, asset).ok_or(Error::TokenContractNotSet)?;
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(&env.current_contract_address(), lender, &amount);

        // Emit event (use b_tokens_to_burn, not the original b_tokens)
        Events::withdraw(env, lender, asset, amount, b_tokens_to_burn);

        cache.flush(env);
        Ok(amount)
    }

    /// Get bToken balance for a lender
    pub fn get_b_token_balance(env: &Env, lender: &Address, asset: &Symbol) -> i128 {
        Storage::get_b_token_balance(env, lender, asset)
    }

    /// Get bTokenRate for an asset
    pub fn get_b_token_rate(env: &Env, asset: &Symbol) -> i128 {
        Storage::get_b_token_rate(env, asset)
    }

    /// Get total bToken supply for an asset
    pub fn get_b_token_supply(env: &Env, asset: &Symbol) -> i128 {
        Storage::get_b_token_supply(env, asset)
    }
}
