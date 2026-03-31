use soroban_sdk::{Address, Env, Map, Symbol, token::TokenClient};

use crate::common::error::Error;
use crate::common::storage::Storage;
use crate::common::types::{
    AUCTION_DURATION_BLOCKS, AuctionData, AuctionType, MAX_HEALTH_FACTOR, SCALAR_7, SCALAR_12,
};
use crate::operations::collateral::Collateral;
use crate::operations::oracles::Oracles;

/// Liquidation functions using Dutch Auctions (unified with AuctionData)
pub struct Liquidations;

impl Liquidations {
    /// Initiate a liquidation auction for a borrower
    /// Returns the auction ID (u32)
    pub fn initiate_liquidation(
        env: &Env,
        borrower: &Address,
        neko_token: &Address,
        debt_asset: &Symbol,
        liquidation_percent: u32,
    ) -> Result<u32, Error> {
        // Get CDP
        let cdp = Storage::get_cdp(env, borrower).ok_or(Error::CDPNotInsolvent)?;

        // Check if borrower has debt in this asset
        if cdp.debt_asset.as_ref() != Some(debt_asset) {
            return Err(Error::CDPNotInsolvent);
        }

        // Calculate health factor
        let health_factor = Self::calculate_health_factor(env, borrower)?;

        // Check if CDP is insolvent (health factor < 1.0)
        // A CDP can only be liquidated if health factor < 1.0 (10_000_000 in 7 decimals)
        if health_factor >= SCALAR_7 as u32 {
            return Err(Error::CDPNotInsolvent);
        }

        // Get collateral amount
        let collateral_amount = Storage::get_collateral(env, borrower, neko_token);
        if collateral_amount == 0 {
            return Err(Error::InsufficientCollateral);
        }

        // Get debt amount (using SCALAR_12 for dToken rate)
        let d_token_rate = Storage::get_d_token_rate(env, debt_asset);
        let debt_amount = cdp
            .d_tokens
            .checked_mul(d_token_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_12)
            .ok_or(Error::ArithmeticError)?;

        // Calculate liquidation amounts based on liquidation_percent (7 decimals)
        let liquidation_debt = debt_amount
            .checked_mul(liquidation_percent as i128)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_7)
            .ok_or(Error::ArithmeticError)?;

        // Calculate collateral to liquidate using premium formula
        // Premium p = (1 - avg_cf * avg_lf) / 2 + 1
        let collateral_factor = crate::admin::Admin::get_collateral_factor(env, neko_token);
        let avg_cf = collateral_factor as i128;
        let avg_lf = SCALAR_7; // 1.0 (100%)

        // Calculate premium: p = (1 - avg_cf * avg_lf) / 2 + 1
        let cf_lf_product = avg_cf
            .checked_mul(avg_lf)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_7)
            .ok_or(Error::ArithmeticError)?;

        let premium = (SCALAR_7
            .checked_sub(cf_lf_product)
            .ok_or(Error::ArithmeticError)?
            .checked_div(2)
            .ok_or(Error::ArithmeticError)?)
        .checked_add(SCALAR_7)
        .ok_or(Error::ArithmeticError)?;

        // Get total collateral value — route to correct oracle based on collateral asset type
        let (rwa_price, rwa_decimals) = Oracles::get_price_for_collateral(env, neko_token)?;
        let total_collateral_value = Oracles::calculate_usd_value(
            env,
            collateral_amount,
            rwa_price,
            rwa_decimals,
            rwa_decimals,
        )?;

        // Get total debt value — route to correct oracle based on debt asset type
        let (debt_price, debt_decimals) = Oracles::get_price_for_lending_asset(env, debt_asset)?;
        let total_debt_value = Oracles::calculate_usd_value(
            env,
            debt_amount,
            debt_price,
            debt_decimals,
            debt_decimals,
        )?;

        // Calculate collateral percentage: C_p = (p * L_p * L_o) / C_o
        let collateral_percent = premium
            .checked_mul(liquidation_percent as i128)
            .ok_or(Error::ArithmeticError)?
            .checked_mul(total_debt_value)
            .ok_or(Error::ArithmeticError)?
            .checked_div(total_collateral_value)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_7)
            .ok_or(Error::ArithmeticError)?;

        // Cap at 100% (SCALAR_7)
        let collateral_percent_capped = collateral_percent.min(SCALAR_7);

        // Calculate collateral amount to liquidate
        let liquidation_collateral = collateral_amount
            .checked_mul(collateral_percent_capped)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_7)
            .ok_or(Error::ArithmeticError)?;

        // Generate auction ID
        let auction_id = Self::generate_auction_id(env);

        // Get token contract address for debt asset
        let debt_token_address =
            Storage::get_token_contract(env, debt_asset).ok_or(Error::TokenContractNotSet)?;

        // Create lot map (collateral - what liquidator receives)
        let mut lot = Map::new(env);
        lot.set(neko_token.clone(), liquidation_collateral);

        // Create bid map (debt - what liquidator pays)
        let mut bid = Map::new(env);
        bid.set(debt_token_address, liquidation_debt);

        // Create AuctionData (unified structure)
        let auction = AuctionData {
            auction_type: AuctionType::UserLiquidation,
            user: borrower.clone(),
            bid,
            lot,
            block: env.ledger().sequence(),
        };

        // Store auction
        Storage::set_auction(env, auction_id, &auction);

        // Emit event
        crate::common::events::Events::liquidation_initiated(
            env,
            borrower,
            neko_token,
            debt_asset,
            liquidation_collateral,
            liquidation_debt,
            auction_id,
        );

        Ok(auction_id)
    }

    /// Fill a liquidation auction
    pub fn fill_auction(env: &Env, auction_id: u32, liquidator: &Address) -> Result<(), Error> {
        liquidator.require_auth();

        let auction = Storage::get_auction(env, auction_id).ok_or(Error::AuctionNotFound)?;

        // Verify it's a user liquidation auction
        if auction.auction_type != AuctionType::UserLiquidation {
            return Err(Error::AuctionNotActive);
        }

        // Calculate blocks elapsed
        let blocks_elapsed = env.ledger().sequence() - auction.block;
        let (lot_modifier, bid_modifier) = Self::calculate_auction_modifiers(blocks_elapsed);

        // Get collateral info from lot map (first entry)
        let lot_keys: soroban_sdk::Vec<Address> = auction.lot.keys();
        if lot_keys.is_empty() {
            return Err(Error::AuctionNotActive);
        }
        let neko_token = lot_keys.get(0).ok_or(Error::AuctionNotActive)?;
        let collateral_amount = auction.lot.get(neko_token.clone()).unwrap_or(0);

        // Get debt info from bid map (first entry)
        let bid_keys: soroban_sdk::Vec<Address> = auction.bid.keys();
        if bid_keys.is_empty() {
            return Err(Error::AuctionNotActive);
        }
        let debt_token_address = bid_keys.get(0).ok_or(Error::AuctionNotActive)?;
        let debt_amount = auction.bid.get(debt_token_address.clone()).unwrap_or(0);

        // Calculate collateral to receive and debt to pay (modifiers use SCALAR_12)
        let collateral_received = collateral_amount
            .checked_mul(lot_modifier)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_12)
            .ok_or(Error::ArithmeticError)?;

        let debt_to_pay = debt_amount
            .checked_mul(bid_modifier)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_12)
            .ok_or(Error::ArithmeticError)?;

        // Transfer debt asset from liquidator to pool
        let token_client = TokenClient::new(env, &debt_token_address);
        token_client.transfer(liquidator, env.current_contract_address(), &debt_to_pay);

        // Calculate liquidation fee (1% of collateral goes to treasury)
        let neko_token_client = TokenClient::new(env, &neko_token);
        let liquidation_fee_rate = Storage::get_liquidation_fee_rate(env) as i128;
        let liq_fee = collateral_received
            .checked_mul(liquidation_fee_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_7)
            .ok_or(Error::ArithmeticError)?;
        let collateral_for_liquidator = collateral_received
            .checked_sub(liq_fee)
            .ok_or(Error::ArithmeticError)?;

        // Transfer collateral minus fee to liquidator
        neko_token_client.transfer(
            &env.current_contract_address(),
            liquidator,
            &collateral_for_liquidator,
        );

        // Transfer liquidation fee directly to treasury
        if liq_fee > 0 {
            let treasury = Storage::get_treasury(env);
            neko_token_client.transfer(&env.current_contract_address(), &treasury, &liq_fee);
            crate::common::events::Events::liquidation_fee(env, &neko_token, liq_fee, &treasury);
        }

        // Update CDP
        let borrower = &auction.user;
        let mut cdp = Storage::get_cdp(env, borrower).ok_or(Error::CDPNotInsolvent)?;

        // Get debt asset symbol from CDP
        let debt_asset = cdp.debt_asset.clone().ok_or(Error::DebtAssetNotSet)?;

        // Calculate dTokens to burn (using SCALAR_12)
        let d_token_rate = Storage::get_d_token_rate(env, &debt_asset);
        let d_tokens_to_burn = debt_to_pay
            .checked_mul(SCALAR_12)
            .ok_or(Error::ArithmeticError)?
            .checked_div(d_token_rate)
            .ok_or(Error::ArithmeticError)?;

        cdp.d_tokens -= d_tokens_to_burn;
        if cdp.d_tokens == 0 {
            cdp.debt_asset = None;
        }
        cdp.last_update = env.ledger().timestamp();
        Storage::set_cdp(env, borrower, &cdp);

        // Update collateral
        let current_collateral = Storage::get_collateral(env, borrower, &neko_token);
        Storage::set_collateral(
            env,
            borrower,
            &neko_token,
            current_collateral - collateral_received,
        );

        // Update dToken balance
        let current_balance = Storage::get_d_token_balance(env, borrower, &debt_asset);
        Storage::set_d_token_balance(
            env,
            borrower,
            &debt_asset,
            current_balance - d_tokens_to_burn,
        );

        // Update pool balance
        let pool_balance = Storage::get_pool_balance(env, &debt_asset);
        Storage::set_pool_balance(env, &debt_asset, pool_balance + debt_to_pay);

        // Verify post-liquidation health factor (7 decimals)
        let post_liq_health_factor = Self::calculate_health_factor(env, borrower)?;
        if (post_liq_health_factor as i128) > MAX_HEALTH_FACTOR {
            return Err(Error::HealthFactorTooHigh);
        }

        // Remove auction (it's been filled)
        Storage::del_auction(env, auction_id);

        // Emit event
        crate::common::events::Events::liquidation_filled(
            env,
            auction_id,
            liquidator,
            collateral_received,
            debt_to_pay,
        );

        Ok(())
    }

    /// Calculate health factor for a borrower
    /// Health Factor = (CollateralValue × CollateralFactor) / DebtValue
    /// Returns health factor in 7 decimals (10_000_000 = 1.0)
    pub fn calculate_health_factor(env: &Env, borrower: &Address) -> Result<u32, Error> {
        // Get CDP
        let cdp = Storage::get_cdp(env, borrower).ok_or(Error::CDPNotInsolvent)?;

        // Calculate total collateral value
        let all_collateral = Collateral::get_all_collateral(env, borrower);
        let mut total_collateral_value = 0i128;

        // Fetch oracle decimals once before the loop to avoid per-item cross-contract calls
        let neko_oracle_decimals = Oracles::get_neko_oracle_decimals(env);
        let reflector_oracle_decimals = Oracles::get_reflector_oracle_decimals(env);

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
            let collateral_factor = crate::admin::Admin::get_collateral_factor(env, &neko_token);

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

        // Calculate total debt value (using SCALAR_12 for dToken rate)
        let total_debt_value = if let Some(debt_asset) = &cdp.debt_asset {
            if cdp.d_tokens > 0 {
                let d_token_rate = Storage::get_d_token_rate(env, debt_asset);
                let debt_amount = cdp
                    .d_tokens
                    .checked_mul(d_token_rate)
                    .ok_or(Error::ArithmeticError)?
                    .checked_div(SCALAR_12)
                    .ok_or(Error::ArithmeticError)?;

                // Route to correct oracle based on debt asset type
                let (debt_price, debt_price_decimals) =
                    Oracles::get_price_for_lending_asset(env, debt_asset)?;

                // Calculate debt value in USD (_asset_decimals unused in calculate_usd_value)
                Oracles::calculate_usd_value(
                    env,
                    debt_amount,
                    debt_price,
                    0,
                    debt_price_decimals,
                )?
            } else {
                0
            }
        } else {
            0
        };

        if total_debt_value == 0 {
            // No debt, health factor is infinite (return max value)
            return Ok(u32::MAX);
        }

        // Apply l_factor: effective_debt = debt_usd * SCALAR_7 / l_factor
        // Lower l_factor → larger effective_debt → stricter health factor
        let l_factor = if let Some(debt_asset) = &cdp.debt_asset {
            Storage::get_interest_rate_params(env, debt_asset)
                .map(|p| p.l_factor as i128)
                .unwrap_or(SCALAR_7)
        } else {
            SCALAR_7
        };
        let effective_debt = total_debt_value
            .checked_mul(SCALAR_7)
            .ok_or(Error::ArithmeticError)?
            .checked_div(l_factor)
            .ok_or(Error::ArithmeticError)?;

        // Health Factor = (CollateralValue × CollateralFactor) / EffectiveDebt
        // With 7 decimals: HF = (total_collateral_value * SCALAR_7) / effective_debt
        let health_factor = total_collateral_value
            .checked_mul(SCALAR_7)
            .ok_or(Error::ArithmeticError)?
            .checked_div(effective_debt)
            .ok_or(Error::ArithmeticError)?;

        // Cap at u32::MAX
        Ok(health_factor.min(u32::MAX as i128) as u32)
    }

    /// Calculate auction modifiers (lot modifier and bid modifier)
    /// Modifiers use SCALAR_12 (12 decimals)
    fn calculate_auction_modifiers(blocks_elapsed: u32) -> (i128, i128) {
        let duration = AUCTION_DURATION_BLOCKS;

        // Lot Modifier: 0 → 1 over AUCTION_DURATION_BLOCKS blocks
        let lot_modifier = if blocks_elapsed <= duration {
            (blocks_elapsed as i128 * SCALAR_12) / duration as i128
        } else {
            SCALAR_12 // 1.0
        };

        // Bid Modifier: 1 → 0 after AUCTION_DURATION_BLOCKS blocks
        let bid_modifier = if blocks_elapsed <= duration {
            SCALAR_12 // 1.0
        } else {
            // Decrease from 1.0 to 0.0 over time
            let decrease = ((blocks_elapsed - duration) as i128 * SCALAR_12) / duration as i128;
            (SCALAR_12 - decrease).max(0)
        };

        (lot_modifier, bid_modifier)
    }

    /// Generate unique auction ID
    fn generate_auction_id(env: &Env) -> u32 {
        let sequence = env.ledger().sequence();
        let timestamp = env.ledger().timestamp() as u32;
        // Add offset to avoid collision with other auction types
        sequence.wrapping_add(timestamp).wrapping_add(2000)
    }
}
