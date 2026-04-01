use soroban_sdk::Env;

use crate::adapters::AdapterClient;
use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::Storage;
use crate::common::types::{BPS, SCALAR_7, SECONDS_PER_YEAR};
use crate::vault::nav::Nav;
use crate::vault::shares::Shares;

pub struct Harvester;

impl Harvester {
    /// Call a_harvest() on all active protocols and accumulate rewards in liquid_reserve.
    /// Also accrues the management fee.
    pub fn harvest_all(env: &Env) -> Result<i128, Error> {
        let mut storage = Storage::load(env);
        let vault_addr = env.current_contract_address();
        let mut total_harvested = 0i128;

        for protocol_id in storage.protocol_ids.iter() {
            if let Some(alloc) = storage.protocol_allocations.get(protocol_id) {
                if !alloc.is_active {
                    continue;
                }
                let adapter = AdapterClient::new(env, &alloc.adapter);
                let harvested = adapter.a_harvest(&vault_addr);
                total_harvested = total_harvested
                    .checked_add(harvested)
                    .ok_or(Error::ArithmeticError)?;
            }
        }

        // Harvested rewards go to liquid_reserve
        storage.liquid_reserve = storage
            .liquid_reserve
            .checked_add(total_harvested)
            .ok_or(Error::ArithmeticError)?;

        // Accrue management fee (mints shares to admin)
        Self::accrue_management_fee(env, &mut storage)?;

        Storage::save(env, &storage);
        Events::harvested(env, total_harvested);

        Ok(total_harvested)
    }

    /// Accrue management fee as newly minted shares to the admin.
    ///
    /// fee_shares = NAV * mgmt_fee_bps * elapsed_seconds / BPS / SECONDS_PER_YEAR
    fn accrue_management_fee(
        env: &Env,
        storage: &mut crate::common::types::VaultStorage,
    ) -> Result<(), Error> {
        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(storage.last_fee_accrual);

        if elapsed == 0 || storage.config.management_fee_bps == 0 {
            return Ok(());
        }

        let nav = Nav::calculate(env, storage)?;
        if nav == 0 {
            storage.last_fee_accrual = now;
            return Ok(());
        }

        // fee = nav * management_fee_bps * elapsed / BPS / SECONDS_PER_YEAR
        // Using u128 intermediate to avoid overflow
        let fee_amount = (nav as u128)
            .checked_mul(storage.config.management_fee_bps as u128)
            .unwrap_or(0)
            .checked_mul(elapsed as u128)
            .unwrap_or(0)
            / BPS as u128
            / SECONDS_PER_YEAR as u128;

        if fee_amount > 0 && storage.total_shares > 0 {
            // Convert fee_amount to shares using current share_price
            let share_price = Nav::share_price(nav, storage.total_shares)?;
            let fee_shares = (fee_amount as i128)
                .checked_mul(SCALAR_7)
                .ok_or(Error::ArithmeticError)?
                .checked_div(share_price)
                .ok_or(Error::ArithmeticError)?;

            if fee_shares > 0 {
                Shares::mint(env, &storage.admin, fee_shares);
                storage.total_shares = storage
                    .total_shares
                    .checked_add(fee_shares)
                    .ok_or(Error::ArithmeticError)?;
            }
        }

        storage.last_fee_accrual = now;
        Ok(())
    }

    /// Apply performance fee when share_price exceeds the high_water_mark.
    /// Mints new shares to admin for the fee amount.
    pub fn apply_performance_fee(env: &Env) -> Result<(), Error> {
        let mut storage = Storage::load(env);

        if storage.config.performance_fee_bps == 0 || storage.total_shares == 0 {
            return Ok(());
        }

        let nav = Nav::calculate(env, &storage)?;
        let share_price = Nav::share_price(nav, storage.total_shares)?;

        if share_price <= storage.high_water_mark {
            return Ok(());
        }

        // gains = (share_price - HWM) * total_shares / SCALAR_7
        let gains = share_price
            .checked_sub(storage.high_water_mark)
            .ok_or(Error::ArithmeticError)?
            .checked_mul(storage.total_shares)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_7)
            .ok_or(Error::ArithmeticError)?;

        // perf_fee = gains * performance_fee_bps / BPS
        let perf_fee = gains
            .checked_mul(storage.config.performance_fee_bps as i128)
            .ok_or(Error::ArithmeticError)?
            .checked_div(BPS)
            .ok_or(Error::ArithmeticError)?;

        if perf_fee > 0 {
            let fee_shares = perf_fee
                .checked_mul(SCALAR_7)
                .ok_or(Error::ArithmeticError)?
                .checked_div(share_price)
                .ok_or(Error::ArithmeticError)?;

            if fee_shares > 0 {
                Shares::mint(env, &storage.admin, fee_shares);
                storage.total_shares = storage
                    .total_shares
                    .checked_add(fee_shares)
                    .ok_or(Error::ArithmeticError)?;
            }
        }

        // Update HWM to current share price (recalculate after fee dilution)
        storage.high_water_mark = share_price;
        Storage::save(env, &storage);

        Ok(())
    }
}
