use soroban_sdk::{Address, Env, Symbol, panic_with_error, token::TokenClient};

use crate::common::error::Error;
use crate::common::events::Events;
use crate::common::storage::Storage;
use crate::common::types::{AssetType, InterestRateParams, PoolState, SCALAR_7};

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
        backstop_threshold: i128,
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
        Storage::set_backstop_threshold(env, backstop_threshold);
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

    /// Set interest rate parameters for an asset
    pub fn set_interest_rate_params(env: &Env, asset: &Symbol, params: &InterestRateParams) {
        Self::require_admin(env);

        // Validate parameters (7 decimals)
        // target_util should be <= 95% (9_500_000)
        if params.target_util > 9_500_000 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }

        // max_util should be > target_util and <= 100%
        if params.max_util <= params.target_util || params.max_util > SCALAR_7 as u32 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }

        Storage::set_interest_rate_params(env, asset, params);
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

    /// Set backstop threshold
    pub fn set_backstop_threshold(env: &Env, threshold: i128) {
        Self::require_admin(env);
        Storage::set_backstop_threshold(env, threshold);
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

    /// Set backstop token contract address
    pub fn set_backstop_token(env: &Env, token_address: &Address) {
        Self::require_admin(env);
        Storage::set_backstop_token(env, token_address);
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
