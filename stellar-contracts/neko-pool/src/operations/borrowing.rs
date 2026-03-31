use soroban_sdk::{Address, Env, Symbol, assert_with_error, token::TokenClient};

use crate::admin::Admin;
use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::Storage;
use crate::common::types::{self, PoolState, SCALAR_7, SCALAR_12};
use crate::operations::collateral::Collateral;
use crate::operations::interest::Interest;
use crate::operations::oracles::Oracles;

/// Borrowing functions for dTokens (single asset per borrower)
/// Token rates use 12 decimals (SCALAR_12)
pub struct Borrowing;

impl Borrowing {
    /// Borrow crypto asset from the pool (single asset per borrower)
    pub fn borrow(
        env: &Env,
        borrower: &Address,
        asset: &Symbol,
        amount: i128,
    ) -> Result<i128, Error> {
        borrower.require_auth();

        assert_with_error!(env, amount > 0, Error::NotPositive);

        // Check pool state
        let pool_state = Admin::get_pool_state(env);
        if matches!(pool_state, PoolState::OnIce | PoolState::Frozen) {
            return Err(Error::PoolOnIce);
        }

        // Check reserve is enabled and capture l_factor
        let l_factor = if let Some(params) = Storage::get_interest_rate_params(env, asset) {
            if !params.enabled {
                return Err(Error::ReserveDisabled);
            }
            params.l_factor as i128
        } else {
            SCALAR_7
        };

        // Accrue interest before borrow — reuse returned data to avoid repeated storage reads
        let mut reserve = Interest::accrue_interest(env, asset)?;

        // Get or create CDP
        let mut cdp =
            Storage::get_cdp(env, borrower).unwrap_or_else(|| crate::common::types::CDP {
                collateral: soroban_sdk::Map::new(env),
                debt_asset: None,
                d_tokens: 0,
                created_at: env.ledger().timestamp(),
                last_update: env.ledger().timestamp(),
            });

        // Check if borrower already has debt in a different asset
        if let Some(debt_asset) = &cdp.debt_asset
            && debt_asset != asset
        {
            return Err(Error::DebtAssetAlreadySet);
        }

        // Calculate borrow limit (already accounts for effective current debt via l_factor)
        let borrow_limit = Self::calculate_borrow_limit(env, borrower)?;

        // Fetch asset price for new debt value calculation
        let (asset_price, price_decimals) = Oracles::get_price_for_lending_asset(env, asset)?;

        // Calculate new debt value in effective terms (applying l_factor)
        let new_debt_value =
            Oracles::calculate_usd_value(env, amount, asset_price, 0, price_decimals)?;
        let effective_new_debt = new_debt_value
            .checked_mul(SCALAR_7)
            .ok_or(Error::ArithmeticError)?
            .checked_div(l_factor)
            .ok_or(Error::ArithmeticError)?;

        if effective_new_debt > borrow_limit {
            return Err(Error::InsufficientBorrowLimit);
        }

        // Check pool has enough balance
        let pool_balance = Storage::get_pool_balance(env, asset);
        if pool_balance < amount {
            return Err(Error::InsufficientPoolBalance);
        }

        // Get dTokenRate from cached reserve data (no extra storage read)
        let d_token_rate = reserve.d_rate;

        // Calculate origination fee
        let origination_fee_rate = Storage::get_origination_fee_rate(env) as i128;
        let origination_fee = amount
            .checked_mul(origination_fee_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_7)
            .ok_or(Error::ArithmeticError)?;

        // D-tokens are minted for amount + fee: borrower owes more than they receive
        let borrow_plus_fee = amount
            .checked_add(origination_fee)
            .ok_or(Error::ArithmeticError)?;
        let d_tokens = types::rounding::to_d_token_up(borrow_plus_fee, d_token_rate)?;

        // Track origination fee as treasury credit — update cached reserve and write once
        if origination_fee > 0 {
            reserve.treasury_credit = reserve
                .treasury_credit
                .checked_add(origination_fee)
                .ok_or(Error::ArithmeticError)?;
            Storage::set_reserve_data(env, asset, &reserve);
        }

        // Update CDP
        cdp.debt_asset = Some(asset.clone());
        cdp.d_tokens += d_tokens;
        cdp.last_update = env.ledger().timestamp();
        Storage::set_cdp(env, borrower, &cdp);

        // Update dToken balance
        let current_balance = Storage::get_d_token_balance(env, borrower, asset);
        Storage::set_d_token_balance(env, borrower, asset, current_balance + d_tokens);

        // Update dToken supply
        let current_supply = Storage::get_d_token_supply(env, asset);
        Storage::set_d_token_supply(env, asset, current_supply + d_tokens);

        // Pool balance decreases only by `amount` (fee remains in pool as treasury credit)
        Storage::set_pool_balance(env, asset, pool_balance - amount);

        // Verify utilization is below 100% after borrow
        // This ensures the pool maintains enough liquidity
        let utilization = Interest::calculate_utilization(env, asset)?;
        if utilization >= SCALAR_7 {
            return Err(Error::InvalidUtilRate);
        }

        // Transfer asset from pool to borrower
        let token_address =
            Storage::get_token_contract(env, asset).ok_or(Error::TokenContractNotSet)?;
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(&env.current_contract_address(), borrower, &amount);

        // Emit event
        Events::borrow(env, borrower, asset, amount, d_tokens);

        Ok(d_tokens)
    }

    /// Repay debt by burning dTokens
    pub fn repay(
        env: &Env,
        borrower: &Address,
        asset: &Symbol,
        d_tokens: i128,
    ) -> Result<i128, Error> {
        borrower.require_auth();

        assert_with_error!(env, d_tokens > 0, Error::NotPositive);

        // Accrue interest before repay — reuse returned data to avoid a second storage read
        let reserve = Interest::accrue_interest(env, asset)?;

        // Get CDP
        let mut cdp = Storage::get_cdp(env, borrower).ok_or(Error::DebtAssetNotSet)?;

        // Check debt asset matches
        if cdp.debt_asset.as_ref() != Some(asset) {
            return Err(Error::DebtAssetNotSet);
        }

        // Check borrower has enough dTokens
        let borrower_balance = Storage::get_d_token_balance(env, borrower, asset);
        if borrower_balance < d_tokens {
            return Err(Error::InsufficientDTokenBalance);
        }

        // Check that we're not trying to burn more dTokens than the user has in CDP
        let cur_d_tokens = cdp.d_tokens;
        let d_tokens_to_burn = if d_tokens > cur_d_tokens {
            // If trying to burn more than debt, only burn what's owed
            cur_d_tokens
        } else {
            d_tokens
        };

        // Get dTokenRate from cached reserve data (no extra storage read)
        let d_token_rate = reserve.d_rate;

        // Calculate amount to repay: dTokens × dTokenRate / SCALAR_12
        let amount = d_tokens_to_burn
            .checked_mul(d_token_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_12)
            .ok_or(Error::ArithmeticError)?;

        // Update CDP
        cdp.d_tokens -= d_tokens_to_burn;
        if cdp.d_tokens == 0 {
            cdp.debt_asset = None;
        }
        cdp.last_update = env.ledger().timestamp();
        Storage::set_cdp(env, borrower, &cdp);

        // Update dToken balance
        Storage::set_d_token_balance(env, borrower, asset, borrower_balance - d_tokens_to_burn);

        // Update dToken supply
        let current_supply = Storage::get_d_token_supply(env, asset);
        Storage::set_d_token_supply(env, asset, current_supply - d_tokens_to_burn);

        // Update pool balance
        let pool_balance = Storage::get_pool_balance(env, asset);
        Storage::set_pool_balance(env, asset, pool_balance + amount);

        // Transfer asset from borrower to pool
        let token_address =
            Storage::get_token_contract(env, asset).ok_or(Error::TokenContractNotSet)?;
        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(borrower, env.current_contract_address(), &amount);

        // Emit event
        Events::repay(env, borrower, asset, amount, d_tokens_to_burn);

        Ok(amount)
    }

    /// Calculate borrow limit for a borrower
    pub fn calculate_borrow_limit(env: &Env, borrower: &Address) -> Result<i128, Error> {
        // Get all collateral
        let all_collateral = Collateral::get_all_collateral(env, borrower);

        let mut total_collateral_value = 0i128;

        // Fetch oracle decimals once before the loop to avoid one cross-contract call per collateral item
        let neko_oracle_decimals = Oracles::get_neko_oracle_decimals(env);
        let reflector_oracle_decimals = Oracles::get_reflector_oracle_decimals(env);

        // Iterate through all collateral
        let keys = all_collateral.keys();
        for neko_token in keys {
            let collateral_amount = all_collateral.get(neko_token.clone()).unwrap_or(0);
            if collateral_amount == 0 {
                continue;
            }

            // Route to correct oracle, reusing pre-fetched decimals
            let (rwa_price, price_decimals) = Oracles::get_price_for_collateral_cached(
                env,
                &neko_token,
                neko_oracle_decimals,
                reflector_oracle_decimals,
            )?;

            // Calculate collateral value in USD (_asset_decimals unused in calculate_usd_value)
            let collateral_value = Oracles::calculate_usd_value(
                env,
                collateral_amount,
                rwa_price,
                0,
                price_decimals,
            )?;

            // Get collateral factor (7 decimals)
            let collateral_factor = Admin::get_collateral_factor(env, &neko_token);

            // Add to total: CollateralValue × CollateralFactor / SCALAR_7
            let factored_value = collateral_value
                .checked_mul(collateral_factor as i128)
                .ok_or(Error::ArithmeticError)?
                .checked_div(SCALAR_7)
                .ok_or(Error::ArithmeticError)?;

            total_collateral_value = total_collateral_value
                .checked_add(factored_value)
                .ok_or(Error::ArithmeticError)?;
        }

        // Get current debt
        let cdp = Storage::get_cdp(env, borrower);
        let (current_debt_value, l_factor) = if let Some(cdp) = cdp {
            if let Some(debt_asset) = &cdp.debt_asset {
                if cdp.d_tokens > 0 {
                    let d_token_rate = Storage::get_d_token_rate(env, debt_asset);
                    let debt_amount = cdp
                        .d_tokens
                        .checked_mul(d_token_rate)
                        .ok_or(Error::ArithmeticError)?
                        .checked_div(SCALAR_12)
                        .ok_or(Error::ArithmeticError)?;

                    // Route to correct oracle based on debt asset type
                    let (debt_price, price_decimals) =
                        Oracles::get_price_for_lending_asset(env, debt_asset)?;

                    // Calculate debt value in USD (_asset_decimals unused in calculate_usd_value)
                    let debt_usd = Oracles::calculate_usd_value(
                        env,
                        debt_amount,
                        debt_price,
                        0,
                        price_decimals,
                    )?;

                    let lf = Storage::get_interest_rate_params(env, debt_asset)
                        .map(|p| p.l_factor as i128)
                        .unwrap_or(SCALAR_7);

                    (debt_usd, lf)
                } else {
                    (0, SCALAR_7)
                }
            } else {
                (0, SCALAR_7)
            }
        } else {
            (0, SCALAR_7)
        };

        // Apply l_factor: effective_debt = debt_usd * SCALAR_7 / l_factor
        // Lower l_factor → larger effective_debt → stricter borrow limit
        let effective_debt = current_debt_value
            .checked_mul(SCALAR_7)
            .ok_or(Error::ArithmeticError)?
            .checked_div(l_factor)
            .ok_or(Error::ArithmeticError)?;

        // Borrow Limit = TotalCollateralValue - EffectiveDebt
        let borrow_limit = total_collateral_value
            .checked_sub(effective_debt)
            .ok_or(Error::ArithmeticError)?;

        Ok(borrow_limit.max(0))
    }

    /// Get dToken balance for a borrower
    pub fn get_d_token_balance(env: &Env, borrower: &Address, asset: &Symbol) -> i128 {
        Storage::get_d_token_balance(env, borrower, asset)
    }

    /// Get dTokenRate for an asset (12 decimals)
    pub fn get_d_token_rate(env: &Env, asset: &Symbol) -> i128 {
        Storage::get_d_token_rate(env, asset)
    }
}
