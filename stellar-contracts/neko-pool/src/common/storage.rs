use soroban_sdk::{Address, Env, Map, Symbol, panic_with_error};

use crate::common::error::Error;
use crate::common::types::{
    AssetType, AuctionData, CDP, DataKey, AUCTION_BUMP, AUCTION_TTL,
    INSTANCE_BUMP, INSTANCE_TTL, InterestRateParams, PoolState, PROPOSAL_BUMP, PROPOSAL_TTL,
    QUEUED_CONFIG_BUMP, QUEUED_CONFIG_TTL, QueuedReserveConfig, ReserveData, SHARED_BUMP,
    SHARED_TTL, USER_BUMP, USER_TTL, UserAssetKey,
};

/// Storage operations for the lending pool.
///
/// Storage layout:
/// - Instance  (INSTANCE_TTL): fixed-size scalar config — Admin, PoolState, oracle addresses,
///   fee rates. No Maps; Maps were moved to per-entry persistent entries to avoid unbounded growth.
/// - Persistent SHARED (SHARED_TTL): per-asset config (CollateralFactor, TokenContract, AssetType…),
///   per-asset state (PoolBalance, ReserveData, InterestRateParams), and per-auction state.
/// - Persistent USER   (USER_TTL): per-user data — BTokenBalance, DTokenBalance, Cdp.
pub struct Storage;

impl Storage {
    // =========================================================================
    // TTL helpers
    // =========================================================================

    pub fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL, INSTANCE_BUMP);
    }

    fn extend_shared_ttl(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, SHARED_TTL, SHARED_BUMP);
    }

    fn extend_user_ttl(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, USER_TTL, USER_BUMP);
    }

    // =========================================================================
    // Initialization check
    // =========================================================================

    pub fn is_initialized(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Admin)
    }

    // =========================================================================
    // Admin
    // =========================================================================

    pub fn get_admin(env: &Env) -> Address {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    pub fn set_admin(env: &Env, admin: &Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, admin);
        Self::extend_instance_ttl(env);
    }

    /// Overwrite the admin directly — only called by accept_admin after two-step verification.
    pub fn replace_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
        Self::extend_instance_ttl(env);
    }

    pub fn get_proposed_admin(env: &Env) -> Option<Address> {
        env.storage().temporary().get(&DataKey::ProposedAdmin)
    }

    pub fn set_proposed_admin(env: &Env, proposed: &Address) {
        env.storage()
            .temporary()
            .set(&DataKey::ProposedAdmin, proposed);
        env.storage()
            .temporary()
            .extend_ttl(&DataKey::ProposedAdmin, PROPOSAL_TTL, PROPOSAL_BUMP);
    }

    pub fn del_proposed_admin(env: &Env) {
        env.storage().temporary().remove(&DataKey::ProposedAdmin);
    }

    // =========================================================================
    // Pool state
    // =========================================================================

    pub fn get_pool_state(env: &Env) -> PoolState {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::PoolState)
            .unwrap_or(PoolState::Active)
    }

    pub fn set_pool_state(env: &Env, state: PoolState) {
        env.storage().instance().set(&DataKey::PoolState, &state);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Oracle addresses
    // =========================================================================

    pub fn get_neko_oracle(env: &Env) -> Address {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::NekoOracle)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    pub fn set_neko_oracle(env: &Env, oracle: &Address) {
        env.storage().instance().set(&DataKey::NekoOracle, oracle);
        Self::extend_instance_ttl(env);
    }

    pub fn get_reflector_oracle(env: &Env) -> Address {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::ReflectorOracle)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    pub fn set_reflector_oracle(env: &Env, oracle: &Address) {
        env.storage()
            .instance()
            .set(&DataKey::ReflectorOracle, oracle);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Backstop contract reference
    // =========================================================================

    pub fn get_backstop_contract(env: &Env) -> Option<Address> {
        Self::extend_instance_ttl(env);
        env.storage().instance().get(&DataKey::BackstopContract)
    }

    pub fn set_backstop_contract(env: &Env, backstop: &Address) {
        env.storage()
            .instance()
            .set(&DataKey::BackstopContract, backstop);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Backstop token reference (used by interest auctions)
    // =========================================================================

    pub fn get_backstop_token(env: &Env) -> Option<Address> {
        Self::extend_instance_ttl(env);
        env.storage().instance().get(&DataKey::BackstopToken)
    }

    pub fn set_backstop_token(env: &Env, token: &Address) {
        env.storage()
            .instance()
            .set(&DataKey::BackstopToken, token);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Backstop take rate
    // =========================================================================

    pub fn get_backstop_take_rate(env: &Env) -> u32 {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::BackstopTakeRate)
            .unwrap_or(0)
    }

    pub fn set_backstop_take_rate(env: &Env, rate: u32) {
        env.storage()
            .instance()
            .set(&DataKey::BackstopTakeRate, &rate);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Fee config
    // =========================================================================

    pub fn get_treasury(env: &Env) -> Address {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::Treasury)
            .unwrap_or_else(|| panic_with_error!(env, Error::TreasuryNotSet))
    }

    pub fn set_treasury(env: &Env, treasury: &Address) {
        env.storage()
            .instance()
            .set(&DataKey::Treasury, treasury);
        Self::extend_instance_ttl(env);
    }

    pub fn get_reserve_factor(env: &Env) -> u32 {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::ReserveFactor)
            .unwrap_or(0)
    }

    pub fn set_reserve_factor(env: &Env, factor: u32) {
        env.storage()
            .instance()
            .set(&DataKey::ReserveFactor, &factor);
        Self::extend_instance_ttl(env);
    }

    pub fn get_origination_fee_rate(env: &Env) -> u32 {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::OriginationFeeRate)
            .unwrap_or(0)
    }

    pub fn set_origination_fee_rate(env: &Env, rate: u32) {
        env.storage()
            .instance()
            .set(&DataKey::OriginationFeeRate, &rate);
        Self::extend_instance_ttl(env);
    }

    pub fn get_liquidation_fee_rate(env: &Env) -> u32 {
        Self::extend_instance_ttl(env);
        env.storage()
            .instance()
            .get(&DataKey::LiquidationFeeRate)
            .unwrap_or(0)
    }

    pub fn set_liquidation_fee_rate(env: &Env, rate: u32) {
        env.storage()
            .instance()
            .set(&DataKey::LiquidationFeeRate, &rate);
        Self::extend_instance_ttl(env);
    }

    // =========================================================================
    // Asset config (persistent per-entry, SHARED_TTL)
    // =========================================================================

    pub fn get_token_contract(env: &Env, asset: &Symbol) -> Option<Address> {
        let key = DataKey::TokenContract(asset.clone());
        let val: Option<Address> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        val
    }

    pub fn set_token_contract(env: &Env, asset: &Symbol, token_address: &Address) {
        let key = DataKey::TokenContract(asset.clone());
        env.storage().persistent().set(&key, token_address);
        Self::extend_shared_ttl(env, &key);
    }

    pub fn get_asset_type(env: &Env, asset: &Symbol) -> AssetType {
        let key = DataKey::AssetType(asset.clone());
        let val: Option<AssetType> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        val.unwrap_or(AssetType::Crypto)
    }

    pub fn set_asset_type(env: &Env, asset: &Symbol, asset_type: AssetType) {
        let key = DataKey::AssetType(asset.clone());
        env.storage().persistent().set(&key, &asset_type);
        Self::extend_shared_ttl(env, &key);
    }

    pub fn get_collateral_asset_type(env: &Env, token: &Address) -> AssetType {
        let key = DataKey::CollateralAssetType(token.clone());
        let val: Option<AssetType> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        val.unwrap_or(AssetType::Rwa)
    }

    pub fn set_collateral_asset_type(env: &Env, token: &Address, asset_type: AssetType) {
        let key = DataKey::CollateralAssetType(token.clone());
        env.storage().persistent().set(&key, &asset_type);
        Self::extend_shared_ttl(env, &key);
    }

    pub fn get_collateral_symbol(env: &Env, token: &Address) -> Option<Symbol> {
        let key = DataKey::CollateralSymbol(token.clone());
        let val: Option<Symbol> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        val
    }

    pub fn set_collateral_symbol(env: &Env, token: &Address, symbol: Symbol) {
        let key = DataKey::CollateralSymbol(token.clone());
        env.storage().persistent().set(&key, &symbol);
        Self::extend_shared_ttl(env, &key);
    }

    pub fn get_collateral_factor(env: &Env, token: &Address) -> Option<u32> {
        let key = DataKey::CollateralFactor(token.clone());
        let val: Option<u32> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        val
    }

    pub fn set_collateral_factor(env: &Env, token: &Address, factor: u32) {
        let key = DataKey::CollateralFactor(token.clone());
        env.storage().persistent().set(&key, &factor);
        Self::extend_shared_ttl(env, &key);
    }

    // =========================================================================
    // Reserve data (persistent, per asset)
    // =========================================================================

    pub fn get_reserve_data(env: &Env, asset: &Symbol) -> ReserveData {
        let key = DataKey::ReserveData(asset.clone());
        let data: Option<ReserveData> = env.storage().persistent().get(&key);
        if data.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        data.unwrap_or_else(|| ReserveData::new(env.ledger().timestamp()))
    }

    pub fn set_reserve_data(env: &Env, asset: &Symbol, data: &ReserveData) {
        let key = DataKey::ReserveData(asset.clone());
        env.storage().persistent().set(&key, data);
        Self::extend_shared_ttl(env, &key);
    }

    // =========================================================================
    // Interest rate params (persistent, per asset)
    // =========================================================================

    pub fn get_interest_rate_params(env: &Env, asset: &Symbol) -> Option<InterestRateParams> {
        let key = DataKey::InterestRateParams(asset.clone());
        let data: Option<InterestRateParams> = env.storage().persistent().get(&key);
        if data.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        data
    }

    pub fn set_interest_rate_params(env: &Env, asset: &Symbol, params: &InterestRateParams) {
        let key = DataKey::InterestRateParams(asset.clone());
        env.storage().persistent().set(&key, params);
        Self::extend_shared_ttl(env, &key);
    }

    pub fn get_queued_reserve_config(
        env: &Env,
        asset: &Symbol,
    ) -> Option<QueuedReserveConfig> {
        env.storage()
            .temporary()
            .get(&DataKey::QueuedReserveConfig(asset.clone()))
    }

    pub fn set_queued_reserve_config(
        env: &Env,
        asset: &Symbol,
        queued: &QueuedReserveConfig,
    ) {
        let key = DataKey::QueuedReserveConfig(asset.clone());
        env.storage().temporary().set(&key, queued);
        env.storage()
            .temporary()
            .extend_ttl(&key, QUEUED_CONFIG_TTL, QUEUED_CONFIG_BUMP);
    }

    pub fn del_queued_reserve_config(env: &Env, asset: &Symbol) {
        env.storage()
            .temporary()
            .remove(&DataKey::QueuedReserveConfig(asset.clone()));
    }

    // Convenience wrappers used by interest accrual
    pub fn get_b_token_rate(env: &Env, asset: &Symbol) -> i128 {
        Self::get_reserve_data(env, asset).b_rate
    }

    pub fn get_b_token_supply(env: &Env, asset: &Symbol) -> i128 {
        Self::get_reserve_data(env, asset).b_supply
    }

    pub fn get_d_token_rate(env: &Env, asset: &Symbol) -> i128 {
        Self::get_reserve_data(env, asset).d_rate
    }

    pub fn get_d_token_supply(env: &Env, asset: &Symbol) -> i128 {
        Self::get_reserve_data(env, asset).d_supply
    }

    // =========================================================================
    // Pool balance (persistent, per asset)
    // =========================================================================

    pub fn get_pool_balance(env: &Env, asset: &Symbol) -> i128 {
        let key = DataKey::PoolBalance(asset.clone());
        let val: Option<i128> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_shared_ttl(env, &key);
        }
        val.unwrap_or(0)
    }

    pub fn set_pool_balance(env: &Env, asset: &Symbol, amount: i128) {
        let key = DataKey::PoolBalance(asset.clone());
        env.storage().persistent().set(&key, &amount);
        Self::extend_shared_ttl(env, &key);
    }

    // =========================================================================
    // bToken balances (persistent, per user per asset)
    // =========================================================================

    pub fn get_b_token_balance(env: &Env, lender: &Address, asset: &Symbol) -> i128 {
        let key = DataKey::BTokenBalance(UserAssetKey {
            user: lender.clone(),
            asset: asset.clone(),
        });
        let val: Option<i128> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_user_ttl(env, &key);
        }
        val.unwrap_or(0)
    }

    pub fn set_b_token_balance(env: &Env, lender: &Address, asset: &Symbol, amount: i128) {
        let key = DataKey::BTokenBalance(UserAssetKey {
            user: lender.clone(),
            asset: asset.clone(),
        });
        env.storage().persistent().set(&key, &amount);
        Self::extend_user_ttl(env, &key);
    }

    // =========================================================================
    // dToken balances (persistent, per user per asset)
    // =========================================================================

    pub fn get_d_token_balance(env: &Env, borrower: &Address, asset: &Symbol) -> i128 {
        let key = DataKey::DTokenBalance(UserAssetKey {
            user: borrower.clone(),
            asset: asset.clone(),
        });
        let val: Option<i128> = env.storage().persistent().get(&key);
        if val.is_some() {
            Self::extend_user_ttl(env, &key);
        }
        val.unwrap_or(0)
    }

    pub fn set_d_token_balance(env: &Env, borrower: &Address, asset: &Symbol, amount: i128) {
        let key = DataKey::DTokenBalance(UserAssetKey {
            user: borrower.clone(),
            asset: asset.clone(),
        });
        env.storage().persistent().set(&key, &amount);
        Self::extend_user_ttl(env, &key);
    }

    // =========================================================================
    // Collateral — CDP.collateral is the single source of truth.
    // =========================================================================

    pub fn get_collateral(env: &Env, borrower: &Address, token: &Address) -> i128 {
        Self::get_cdp(env, borrower)
            .and_then(|cdp| cdp.collateral.get(token.clone()))
            .unwrap_or(0)
    }

    pub fn set_collateral(env: &Env, borrower: &Address, token: &Address, amount: i128) {
        let mut cdp = Self::get_cdp(env, borrower).unwrap_or_else(|| CDP {
            collateral: Map::new(env),
            debt_asset: None,
            d_tokens: 0,
            created_at: env.ledger().timestamp(),
            last_update: env.ledger().timestamp(),
        });
        cdp.collateral.set(token.clone(), amount);
        Self::set_cdp(env, borrower, &cdp);
    }

    // =========================================================================
    // CDP (persistent, per borrower)
    // =========================================================================

    pub fn get_cdp(env: &Env, borrower: &Address) -> Option<CDP> {
        let key = DataKey::Cdp(borrower.clone());
        let cdp: Option<CDP> = env.storage().persistent().get(&key);
        if cdp.is_some() {
            Self::extend_user_ttl(env, &key);
        }
        cdp
    }

    pub fn set_cdp(env: &Env, borrower: &Address, cdp: &CDP) {
        let key = DataKey::Cdp(borrower.clone());
        env.storage().persistent().set(&key, cdp);
        Self::extend_user_ttl(env, &key);
    }

    // =========================================================================
    // Auctions (temporary storage — auto-expires after AUCTION_TTL if not filled)
    // =========================================================================

    pub fn get_auction(env: &Env, id: u32) -> Option<AuctionData> {
        let key = DataKey::Auction(id);
        let val: Option<AuctionData> = env.storage().temporary().get(&key);
        if val.is_some() {
            env.storage()
                .temporary()
                .extend_ttl(&key, AUCTION_TTL, AUCTION_BUMP);
        }
        val
    }

    pub fn set_auction(env: &Env, id: u32, auction: &AuctionData) {
        let key = DataKey::Auction(id);
        env.storage().temporary().set(&key, auction);
        env.storage()
            .temporary()
            .extend_ttl(&key, AUCTION_TTL, AUCTION_BUMP);
    }

    pub fn del_auction(env: &Env, id: u32) {
        env.storage().temporary().remove(&DataKey::Auction(id));
    }
}
