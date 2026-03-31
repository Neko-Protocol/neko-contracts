use soroban_sdk::{Address, Env, Symbol, panic_with_error, token::TokenClient};

use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::Storage;
use crate::common::types::{
    AssetType, CONFIG_DELAY_SECONDS, InterestRateParams, PoolState, QueuedReserveConfig, SCALAR_7,
};

// Pool state ordinals pushed by the backstop contract.
const POOL_STATE_ACTIVE: u32 = 0;
const POOL_STATE_ON_ICE: u32 = 1;

/// Administrative functions for the lending pool
pub struct Admin;

impl Admin {
    /// Initialize the lending pool
    pub fn initialize(
        env: &Env,
        admin: &Address,
        treasury: &Address,
        neko_oracle: &Address,
        reflector_oracle: &Address,
        backstop_take_rate: u32,
        reserve_factor: u32,
        origination_fee_rate: u32,
        liquidation_fee_rate: u32,
    ) {
        if Storage::is_initialized(env) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }

        // Validate fee rates: reserve_factor + backstop_take_rate <= SCALAR_7 (can't exceed 100%)
        if (reserve_factor as i128 + backstop_take_rate as i128) > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        if origination_fee_rate as i128 > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        if liquidation_fee_rate as i128 > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }

        Storage::set_admin(env, admin);
        Storage::set_pool_state(env, PoolState::OnIce);
        Storage::set_neko_oracle(env, neko_oracle);
        Storage::set_reflector_oracle(env, reflector_oracle);
        Storage::set_backstop_take_rate(env, backstop_take_rate);
        Storage::set_treasury(env, treasury);
        Storage::set_reserve_factor(env, reserve_factor);
        Storage::set_origination_fee_rate(env, origination_fee_rate);
        Storage::set_liquidation_fee_rate(env, liquidation_fee_rate);
    }

    /// Get the admin address
    pub fn get_admin(env: &Env) -> Address {
        Storage::get_admin(env)
    }

    /// Require admin authorization
    pub fn require_admin(env: &Env) {
        let admin = Self::get_admin(env);
        admin.require_auth();
    }

    /// Propose a new admin (step 1 of two-step transfer).
    /// Current admin calls this — stores the proposal in temporary storage (7-day TTL).
    /// If the proposed address never calls accept_admin(), the proposal expires automatically.
    pub fn propose_admin(env: &Env, proposed: &Address) {
        Self::require_admin(env);
        Storage::set_proposed_admin(env, proposed);
        Events::admin_proposed(env, proposed);
    }

    /// Accept a pending admin proposal (step 2 of two-step transfer).
    /// The proposed address calls this to finalize the transfer.
    pub fn accept_admin(env: &Env) {
        let proposed = Storage::get_proposed_admin(env)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotAuthorized));
        proposed.require_auth();
        Storage::del_proposed_admin(env);
        Storage::replace_admin(env, &proposed);
        Events::admin_changed(env, &proposed);
    }

    /// Set collateral factor for a token (7 decimals)
    /// asset_type: Rwa uses RWA oracle, Crypto uses Reflector oracle
    /// symbol: required for Crypto collateral (e.g. symbol_short!("USDC"))
    /// Example: 7_500_000 = 75%
    pub fn set_collateral_factor(
        env: &Env,
        token: &Address,
        factor: u32,
        asset_type: AssetType,
        symbol: Symbol,
    ) {
        Self::require_admin(env);

        // Validate factor is within [0, SCALAR_7] (0% to 100%)
        if factor > SCALAR_7 as u32 {
            panic_with_error!(env, Error::InvalidCollateralFactor);
        }

        Storage::set_collateral_factor(env, token, factor);
        Storage::set_collateral_asset_type(env, token, asset_type);
        Storage::set_collateral_symbol(env, token, symbol);
    }

    /// Get collateral factor for a token (7 decimals)
    pub fn get_collateral_factor(env: &Env, token: &Address) -> u32 {
        Storage::get_collateral_factor(env, token).unwrap_or(7_500_000) // Default: 75% (7 decimals)
    }

    /// Queue a change to reserve interest rate parameters (step 1 of two-step config change).
    ///
    /// If the pool is OnIce (initial setup), the change is immediately applicable (unlock_time = now).
    /// Otherwise, a 7-day timelock is enforced so users can react before risk params change.
    pub fn queue_set_reserve_params(env: &Env, asset: &Symbol, params: &InterestRateParams) {
        Self::require_admin(env);

        // Cannot queue if there is already a pending queue for this asset
        if Storage::get_queued_reserve_config(env, asset).is_some() {
            panic_with_error!(env, Error::ConfigAlreadyQueued);
        }

        // Validate parameters
        if params.target_util > 9_500_000 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        if params.max_util <= params.target_util || params.max_util > SCALAR_7 as u32 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }

        // If pool is OnIce (initial setup), apply immediately; otherwise enforce timelock
        let pool_state = Storage::get_pool_state(env);
        let unlock_time = if matches!(pool_state, PoolState::OnIce) {
            env.ledger().timestamp()
        } else {
            env.ledger().timestamp() + CONFIG_DELAY_SECONDS
        };

        Storage::set_queued_reserve_config(
            env,
            asset,
            &QueuedReserveConfig {
                new_params: params.clone(),
                unlock_time,
            },
        );

        Events::reserve_config_queued(env, asset, unlock_time);
    }

    /// Apply a queued reserve param change (step 2 of two-step config change).
    /// Panics if no change is queued or if the timelock has not expired.
    pub fn apply_queued_reserve_params(env: &Env, asset: &Symbol) {
        Self::require_admin(env);

        let queued = Storage::get_queued_reserve_config(env, asset)
            .unwrap_or_else(|| panic_with_error!(env, Error::ConfigQueueNotFound));

        if queued.unlock_time > env.ledger().timestamp() {
            panic_with_error!(env, Error::ConfigNotUnlocked);
        }

        Storage::del_queued_reserve_config(env, asset);
        Storage::set_interest_rate_params(env, asset, &queued.new_params);
        Events::reserve_config_applied(env, asset);
    }

    /// Cancel a queued reserve param change before it is applied.
    pub fn cancel_queued_reserve_params(env: &Env, asset: &Symbol) {
        Self::require_admin(env);
        Storage::del_queued_reserve_config(env, asset);
        Events::reserve_config_cancelled(env, asset);
    }

    /// Set pool state
    pub fn set_pool_state(env: &Env, state: PoolState) {
        Self::require_admin(env);
        Storage::set_pool_state(env, state);
    }

    /// Get pool state
    pub fn get_pool_state(env: &Env) -> PoolState {
        Storage::get_pool_state(env)
    }

    /// Set the neko-backstop contract address. Admin-only.
    /// Must be called after deploying both pool and backstop.
    pub fn set_backstop_contract(env: &Env, backstop: &Address) {
        Self::require_admin(env);
        Storage::set_backstop_contract(env, backstop);
    }

    /// Update pool state as pushed by the registered backstop contract.
    ///
    /// Called automatically by neko-backstop on every deposit/withdraw/queue change.
    /// Only the registered backstop address is allowed to call this.
    /// State ordinals: 0 = Active, 1 = OnIce, 2+ = Frozen.
    pub fn update_pool_state_from_backstop(env: &Env, caller: &Address, state: u32) {
        let backstop = Storage::get_backstop_contract(env)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotAuthorized));
        if caller != &backstop {
            panic_with_error!(env, Error::NotAuthorized);
        }
        caller.require_auth();

        let new_state = if state == POOL_STATE_ACTIVE {
            PoolState::Active
        } else if state == POOL_STATE_ON_ICE {
            PoolState::OnIce
        } else {
            PoolState::Frozen
        };

        Storage::set_pool_state(env, new_state);
    }

    /// Set backstop token address — needed by interest auctions to accept bidder payments. Admin-only.
    pub fn set_backstop_token(env: &Env, token_address: &Address) {
        Self::require_admin(env);
        Storage::set_backstop_token(env, token_address);
    }

    /// Set backstop take rate (7 decimals)
    /// Example: 500_000 = 5%
    pub fn set_backstop_take_rate(env: &Env, take_rate: u32) {
        Self::require_admin(env);

        if take_rate > SCALAR_7 as u32 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }

        Storage::set_backstop_take_rate(env, take_rate);
    }

    /// Set token contract address for an asset symbol
    /// asset_type: Rwa uses RWA oracle, Crypto uses Reflector oracle
    pub fn set_token_contract(
        env: &Env,
        asset: &Symbol,
        token_address: &Address,
        asset_type: AssetType,
    ) {
        Self::require_admin(env);
        Storage::set_token_contract(env, asset, token_address);
        Storage::set_asset_type(env, asset, asset_type);
    }

    /// Set the treasury address
    pub fn set_treasury(env: &Env, treasury: &Address) {
        Self::require_admin(env);
        Storage::set_treasury(env, treasury);
    }

    /// Get the treasury address
    pub fn get_treasury(env: &Env) -> Address {
        Storage::get_treasury(env)
    }

    /// Set reserve factor (7 decimals). Must not exceed SCALAR_7 - backstop_take_rate.
    pub fn set_reserve_factor(env: &Env, reserve_factor: u32) {
        Self::require_admin(env);
        let backstop_take_rate = Storage::get_backstop_take_rate(env);
        if (reserve_factor as i128 + backstop_take_rate as i128) > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        Storage::set_reserve_factor(env, reserve_factor);
    }

    /// Set origination fee rate (7 decimals).
    pub fn set_origination_fee_rate(env: &Env, origination_fee_rate: u32) {
        Self::require_admin(env);
        if origination_fee_rate as i128 > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        Storage::set_origination_fee_rate(env, origination_fee_rate);
    }

    /// Set liquidation fee rate (7 decimals).
    pub fn set_liquidation_fee_rate(env: &Env, liquidation_fee_rate: u32) {
        Self::require_admin(env);
        if liquidation_fee_rate as i128 > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        Storage::set_liquidation_fee_rate(env, liquidation_fee_rate);
    }

    /// Collect accumulated treasury fees for an asset and transfer to treasury address.
    /// Only admin can call this.
    pub fn collect_treasury_fees(env: &Env, asset: &Symbol) -> Result<i128, Error> {
        Self::require_admin(env);

        let mut reserve = Storage::get_reserve_data(env, asset);
        let amount = reserve.treasury_credit;
        if amount == 0 {
            return Err(Error::NoTreasuryFeesToCollect);
        }

        let treasury = Storage::get_treasury(env);
        let token_address =
            Storage::get_token_contract(env, asset).ok_or(Error::TokenContractNotSet)?;

        let token_client = TokenClient::new(env, &token_address);
        token_client.transfer(&env.current_contract_address(), &treasury, &amount);

        reserve.treasury_credit = 0;
        Storage::set_reserve_data(env, asset, &reserve);

        Events::treasury_fees_collected(env, asset, amount, &treasury);

        Ok(amount)
    }

    /// Get accumulated treasury fees for an asset (not yet collected).
    pub fn get_treasury_credit(env: &Env, asset: &Symbol) -> i128 {
        Storage::get_reserve_data(env, asset).treasury_credit
    }

    /// Upgrade the contract to a new WASM hash
    /// Only the admin can call this function
    pub fn upgrade(env: &Env, new_wasm_hash: &soroban_sdk::BytesN<32>) {
        Self::require_admin(env);
        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
    }
}
