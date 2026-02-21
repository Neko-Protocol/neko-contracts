use soroban_sdk::{Env, Symbol, Vec};

use crate::adapters::AdapterClient;
use crate::common::types::{RiskTier, VaultStorage};

pub struct Optimizer;

impl Optimizer {
    /// Rank active protocols by risk-adjusted APY (descending).
    /// risk_factor: Low=10000 (100%), Medium=8500 (85%), High=7000 (70%)
    /// Returns Vec<(protocol_id, adjusted_apy_bps)> sorted highest-first.
    pub fn rank_by_apy(env: &Env, storage: &VaultStorage) -> Vec<(Symbol, u32)> {
        let vault_addr = env.current_contract_address();
        let mut results: Vec<(Symbol, u32)> = Vec::new(env);

        for protocol_id in storage.protocol_ids.iter() {
            if let Some(alloc) = storage.protocol_allocations.get(protocol_id.clone()) {
                if !alloc.is_active {
                    continue;
                }
                let adapter = AdapterClient::new(env, &alloc.adapter);
                // Suppress unused-variable warning — vault_addr used for consistency
                let _ = &vault_addr;
                let raw_apy = adapter.a_get_apy();

                let risk_factor: u64 = match alloc.risk_tier {
                    RiskTier::Low => 10_000,
                    RiskTier::Medium => 8_500,
                    RiskTier::High => 7_000,
                };

                let adjusted_apy = (raw_apy as u64 * risk_factor / 10_000) as u32;
                results.push_back((protocol_id.clone(), adjusted_apy));
            }
        }

        // Bubble sort descending by adjusted_apy (n is small, max 10 protocols)
        let n = results.len();
        for i in 0..n {
            for j in 0..(n - 1 - i) {
                let a = results.get(j).unwrap();
                let b = results.get(j + 1).unwrap();
                if a.1 < b.1 {
                    results.set(j, b);
                    results.set(j + 1, a);
                }
            }
        }

        results
    }

    /// Calculate weighted average APY of all active protocols.
    /// Returns APY in basis points.
    pub fn weighted_apy(env: &Env, storage: &VaultStorage) -> u32 {
        let vault_addr = env.current_contract_address();
        let mut total_balance = 0i128;
        let mut weighted_sum = 0i128;

        for protocol_id in storage.protocol_ids.iter() {
            if let Some(alloc) = storage.protocol_allocations.get(protocol_id) {
                if !alloc.is_active {
                    continue;
                }
                let adapter = AdapterClient::new(env, &alloc.adapter);
                let balance = adapter.a_balance(&vault_addr);
                let apy = adapter.a_get_apy() as i128;
                weighted_sum += balance * apy;
                total_balance += balance;
            }
        }

        if total_balance == 0 {
            return 0;
        }

        (weighted_sum / total_balance) as u32
    }
}
