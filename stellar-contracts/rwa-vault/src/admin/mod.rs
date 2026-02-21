use soroban_sdk::{Address, BytesN, Env, String, Symbol, panic_with_error};

use crate::adapters::AdapterClient;
use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::Storage;
use crate::common::types::{
    BPS, MAX_PROTOCOLS, ProtocolAllocation, RiskTier, SCALAR_7, VaultConfig, VaultStatus,
    VaultStorage,
};
use crate::vault::nav::Nav;

pub struct Admin;

impl Admin {
    // ========== Initialization ==========

    pub fn initialize(
        env: &Env,
        admin: &Address,
        manager: &Address,
        deposit_token: &Address,
        token_name: String,
        token_symbol: String,
        token_decimals: u32,
        config: VaultConfig,
    ) {
        if Storage::is_initialized(env) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }

        let storage = VaultStorage {
            status: VaultStatus::Active,
            deposit_token: deposit_token.clone(),
            token_name,
            token_symbol,
            token_decimals,
            liquid_reserve: 0,
            total_shares: 0,
            high_water_mark: SCALAR_7, // Initial share price = 1.0
            last_fee_accrual: env.ledger().timestamp(),
            protocol_ids: soroban_sdk::Vec::new(env),
            protocol_allocations: soroban_sdk::Map::new(env),
            config,
            admin: admin.clone(),
            manager: manager.clone(),
        };

        Storage::save(env, &storage);
    }

    // ========== Auth helpers ==========

    pub fn require_admin(env: &Env) {
        let storage = Storage::load(env);
        storage.admin.require_auth();
    }

    /// Rebalance / harvest operations require manager auth.
    /// Admin can grant themselves manager role via set_manager.
    pub fn require_manager(env: &Env) {
        let storage = Storage::load(env);
        storage.manager.require_auth();
    }

    // ========== Vault state ==========

    pub fn pause(env: &Env) {
        Self::require_admin(env);
        let mut storage = Storage::load(env);
        storage.status = VaultStatus::Paused;
        Storage::save(env, &storage);
    }

    pub fn unpause(env: &Env) {
        Self::require_admin(env);
        let mut storage = Storage::load(env);
        storage.status = VaultStatus::Active;
        Storage::save(env, &storage);
    }

    pub fn emergency_exit(env: &Env) {
        Self::require_admin(env);
        let mut storage = Storage::load(env);
        let vault_addr = env.current_contract_address();

        // Withdraw everything from all protocols into liquid_reserve
        for protocol_id in storage.protocol_ids.iter() {
            if let Some(alloc) = storage.protocol_allocations.get(protocol_id.clone()) {
                if !alloc.is_active {
                    continue;
                }
                let adapter = AdapterClient::new(env, &alloc.adapter);
                let balance = adapter.a_balance(&vault_addr);
                if balance > 0 {
                    let actual = adapter.a_withdraw(&balance, &vault_addr);
                    storage.liquid_reserve += actual;
                }

                // Mark protocol as inactive
                let mut updated_alloc = alloc.clone();
                updated_alloc.is_active = false;
                storage.protocol_allocations.set(protocol_id, updated_alloc);
            }
        }

        storage.status = VaultStatus::EmergencyExit;
        Storage::save(env, &storage);
    }

    pub fn upgrade(env: &Env, new_wasm_hash: BytesN<32>) {
        Self::require_admin(env);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    // ========== Config ==========

    pub fn set_config(env: &Env, config: VaultConfig) {
        Self::require_admin(env);
        // Basic validation
        if config.min_liquidity_bps > BPS as u32
            || config.max_protocol_bps > BPS as u32
            || config.management_fee_bps > 1000  // max 10%
            || config.performance_fee_bps > 5000
        // max 50%
        {
            panic_with_error!(env, Error::InvalidConfig);
        }
        let mut storage = Storage::load(env);
        storage.config = config;
        Storage::save(env, &storage);
    }

    pub fn set_manager(env: &Env, manager: &Address) {
        Self::require_admin(env);
        let mut storage = Storage::load(env);
        storage.manager = manager.clone();
        Storage::save(env, &storage);
    }

    // ========== Protocol management ==========

    pub fn add_protocol(
        env: &Env,
        id: &Symbol,
        adapter: &Address,
        target_bps: u32,
        risk_tier: RiskTier,
    ) {
        Self::require_admin(env);
        let mut storage = Storage::load(env);

        if storage.protocol_ids.len() >= MAX_PROTOCOLS {
            panic_with_error!(env, Error::MaxProtocolsReached);
        }

        if storage.protocol_allocations.contains_key(id.clone()) {
            panic_with_error!(env, Error::ProtocolAlreadyExists);
        }

        if target_bps > BPS as u32 {
            panic_with_error!(env, Error::InvalidConfig);
        }

        storage.protocol_ids.push_back(id.clone());
        storage.protocol_allocations.set(
            id.clone(),
            ProtocolAllocation {
                adapter: adapter.clone(),
                target_bps,
                is_active: true,
                risk_tier,
            },
        );

        Storage::save(env, &storage);
        Events::protocol_added(env, id, adapter);
    }

    pub fn remove_protocol(env: &Env, id: &Symbol) {
        Self::require_admin(env);
        let mut storage = Storage::load(env);

        if !storage.protocol_allocations.contains_key(id.clone()) {
            panic_with_error!(env, Error::ProtocolNotFound);
        }

        // Remove from vec
        let mut new_ids: soroban_sdk::Vec<Symbol> = soroban_sdk::Vec::new(env);
        for pid in storage.protocol_ids.iter() {
            if &pid != id {
                new_ids.push_back(pid);
            }
        }
        storage.protocol_ids = new_ids;
        storage.protocol_allocations.remove(id.clone());

        Storage::save(env, &storage);
        Events::protocol_removed(env, id);
    }

    pub fn set_protocol_active(env: &Env, id: &Symbol, active: bool) {
        Self::require_admin(env);
        let mut storage = Storage::load(env);

        let mut alloc = storage
            .protocol_allocations
            .get(id.clone())
            .unwrap_or_else(|| panic_with_error!(env, Error::ProtocolNotFound));

        alloc.is_active = active;
        storage.protocol_allocations.set(id.clone(), alloc);
        Storage::save(env, &storage);
    }

    // ========== Views ==========

    pub fn get_nav(env: &Env) -> i128 {
        let storage = Storage::load(env);
        Nav::calculate(env, &storage).unwrap_or(0)
    }

    pub fn get_share_price(env: &Env) -> i128 {
        let storage = Storage::load(env);
        let nav = Nav::calculate(env, &storage).unwrap_or(0);
        Nav::share_price(nav, storage.total_shares).unwrap_or(SCALAR_7)
    }
}
