use soroban_sdk::{Address, Env, Map, Symbol, Vec, panic_with_error, token::TokenClient};

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

        // Initialize pool storage with default values
        let storage = crate::common::storage::PoolStorage {
            pool_state: PoolState::OnIce, // Pools start on ice
            pool_balances: Map::new(env),

            // Reserve data
            reserve_data: Map::new(env),

            // User balances
            b_token_balances: Map::new(env),
            d_token_balances: Map::new(env),
            collateral: Map::new(env),

            // Interest rate parameters
            interest_rate_params: Map::new(env),

            // Auctions (unified structure)
            auction_data: Map::new(env),

            // Backstop
            backstop_deposits: Map::new(env),
            backstop_total: 0,
            backstop_threshold,
            backstop_take_rate,
            withdrawal_queue: Vec::new(env),
            backstop_token: None,

            // Treasury & Fees
            treasury: treasury.clone(),
            reserve_factor,
            origination_fee_rate,
            liquidation_fee_rate,

            // Oracles
            neko_oracle: neko_oracle.clone(),
            reflector_oracle: reflector_oracle.clone(),

            // Admin
            admin: admin.clone(),
            collateral_factors: Map::new(env),
            token_contracts: Map::new(env),

            // Asset type routing
            asset_types: Map::new(env),
            collateral_asset_types: Map::new(env),
            collateral_symbols: Map::new(env),
        };

        Storage::set(env, &storage);
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

        let mut storage = Storage::get(env);
        storage.collateral_factors.set(token.clone(), factor);
        Storage::set(env, &storage);

        Storage::set_collateral_asset_type(env, token, asset_type);
        Storage::set_collateral_symbol(env, token, symbol);
    }

    /// Get collateral factor for a token (7 decimals)
    pub fn get_collateral_factor(env: &Env, token: &Address) -> u32 {
        let storage = Storage::get(env);
        storage
            .collateral_factors
            .get(token.clone())
            .unwrap_or(7_500_000) // Default: 75% (7 decimals)
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

        let mut storage = Storage::get(env);
        storage
            .interest_rate_params
            .set(asset.clone(), params.clone());
        Storage::set(env, &storage);
    }

    /// Set pool state
    pub fn set_pool_state(env: &Env, state: PoolState) {
        Self::require_admin(env);

        let mut storage = Storage::get(env);
        storage.pool_state = state;
        Storage::set(env, &storage);
    }

    /// Get pool state
    pub fn get_pool_state(env: &Env) -> PoolState {
        let storage = Storage::get(env);
        storage.pool_state
    }

    /// Set backstop threshold
    pub fn set_backstop_threshold(env: &Env, threshold: i128) {
        Self::require_admin(env);

        let mut storage = Storage::get(env);
        storage.backstop_threshold = threshold;
        Storage::set(env, &storage);
    }

    /// Set backstop take rate (7 decimals)
    /// Example: 500_000 = 5%
    pub fn set_backstop_take_rate(env: &Env, take_rate: u32) {
        Self::require_admin(env);

        if take_rate > SCALAR_7 as u32 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }

        let mut storage = Storage::get(env);
        storage.backstop_take_rate = take_rate;
        Storage::set(env, &storage);
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
        let mut storage = Storage::get(env);
        storage.backstop_token = Some(token_address.clone());
        Storage::set(env, &storage);
    }

    /// Set the treasury address
    pub fn set_treasury(env: &Env, treasury: &Address) {
        Self::require_admin(env);
        let mut storage = Storage::get(env);
        storage.treasury = treasury.clone();
        Storage::set(env, &storage);
    }

    /// Get the treasury address
    pub fn get_treasury(env: &Env) -> Address {
        let storage = Storage::get(env);
        storage.treasury
    }

    /// Set reserve factor (7 decimals). Must not exceed SCALAR_7 - backstop_take_rate.
    pub fn set_reserve_factor(env: &Env, reserve_factor: u32) {
        Self::require_admin(env);
        let mut storage = Storage::get(env);
        if (reserve_factor as i128 + storage.backstop_take_rate as i128) > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        storage.reserve_factor = reserve_factor;
        Storage::set(env, &storage);
    }

    /// Set origination fee rate (7 decimals).
    pub fn set_origination_fee_rate(env: &Env, origination_fee_rate: u32) {
        Self::require_admin(env);
        if origination_fee_rate as i128 > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        let mut storage = Storage::get(env);
        storage.origination_fee_rate = origination_fee_rate;
        Storage::set(env, &storage);
    }

    /// Set liquidation fee rate (7 decimals).
    pub fn set_liquidation_fee_rate(env: &Env, liquidation_fee_rate: u32) {
        Self::require_admin(env);
        if liquidation_fee_rate as i128 > SCALAR_7 {
            panic_with_error!(env, Error::InvalidInterestRateParams);
        }
        let mut storage = Storage::get(env);
        storage.liquidation_fee_rate = liquidation_fee_rate;
        Storage::set(env, &storage);
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

        let storage = Storage::get(env);
        let treasury = storage.treasury.clone();
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
