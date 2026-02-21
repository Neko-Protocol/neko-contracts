use soroban_sdk::{Env, token::TokenClient};

use crate::adapters::AdapterClient;
use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::Storage;
use crate::common::types::BPS;
use crate::vault::nav::Nav;

pub struct Rebalancer;

impl Rebalancer {
    /// Rebalance allocations between protocols.
    ///
    /// For each protocol:
    /// - If current_bps > target_bps + threshold → withdraw excess to liquid_reserve
    /// - If current_bps < target_bps - threshold → deploy from liquid_reserve to protocol
    ///
    /// Only manager or admin can call this.
    pub fn execute(env: &Env) -> Result<(), Error> {
        let mut storage = Storage::load(env);
        let vault_addr = env.current_contract_address();

        let nav = Nav::calculate(env, &storage)?;
        if nav == 0 {
            return Ok(());
        }

        let token = TokenClient::new(env, &storage.deposit_token);
        let threshold = storage.config.rebalance_threshold_bps;

        for protocol_id in storage.protocol_ids.iter() {
            if let Some(alloc) = storage.protocol_allocations.get(protocol_id.clone()) {
                if !alloc.is_active {
                    continue;
                }

                let adapter = AdapterClient::new(env, &alloc.adapter);
                let current_balance = adapter.a_balance(&vault_addr);

                // Calculate current allocation in BPS
                let current_bps = (current_balance
                    .checked_mul(BPS)
                    .ok_or(Error::ArithmeticError)?
                    / nav) as u32;

                let target_bps = alloc.target_bps;

                // Skip if within threshold
                let diff = if current_bps > target_bps {
                    current_bps - target_bps
                } else {
                    target_bps - current_bps
                };

                if diff < threshold {
                    continue;
                }

                if current_bps > target_bps {
                    // Withdraw excess back to liquid_reserve
                    let excess_amount = ((current_bps - target_bps) as i128)
                        .checked_mul(nav)
                        .ok_or(Error::ArithmeticError)?
                        / BPS;

                    let actual = adapter.a_withdraw(&excess_amount, &vault_addr);
                    storage.liquid_reserve = storage
                        .liquid_reserve
                        .checked_add(actual)
                        .ok_or(Error::ArithmeticError)?;
                } else {
                    // Deploy more from liquid_reserve to adapter
                    let needed_amount = ((target_bps - current_bps) as i128)
                        .checked_mul(nav)
                        .ok_or(Error::ArithmeticError)?
                        / BPS;

                    let to_deploy = needed_amount.min(storage.liquid_reserve);
                    if to_deploy == 0 {
                        continue;
                    }

                    // Transfer tokens from vault to adapter
                    token.transfer(&vault_addr, &alloc.adapter, &to_deploy);
                    storage.liquid_reserve = storage
                        .liquid_reserve
                        .checked_sub(to_deploy)
                        .ok_or(Error::ArithmeticError)?;

                    // Tell adapter to deposit (adapter already has the tokens)
                    adapter.a_deposit(&to_deploy, &vault_addr);
                }
            }
        }

        Storage::save(env, &storage);
        Events::rebalanced(env, nav);

        Ok(())
    }
}
