extern crate std;

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, EnvTestConfig, Ledger, LedgerInfo},
    token::StellarAssetClient,
    token::TokenClient,
    Address, Env, Symbol,
};

use neko_backstop::{NekoBackstop, NekoBackstopClient};
use neko_pool::{
    neko_oracle::{self, Asset},
    AssetType, InterestRateParams, LendingContract, LendingContractClient,
};

pub const SCALAR_7: i128 = 10_000_000;
pub const SCALAR_12: i128 = 1_000_000_000_000;

// Prices in 7 decimals
pub const PRICE_USDC: i128 = 1_0000000; // $1.00
pub const PRICE_XLM: i128 = 1_000_000; // $0.10
pub const PRICE_BOND: i128 = 100_0000000; // $100.00

pub const INITIAL_USER_BALANCE: i128 = 100_000 * SCALAR_7;
pub const INITIAL_LIQUIDITY: i128 = 50_000 * SCALAR_7;
pub const BACKSTOP_THRESHOLD: i128 = 10_000 * SCALAR_7;

pub mod assertions {
    pub fn assert_approx_eq_abs(a: i128, b: i128, delta: i128) {
        assert!(
            (a - b).abs() <= delta,
            "assert_approx_eq_abs failed: |{a} - {b}| = {} > {delta}",
            (a - b).abs()
        );
    }
}

pub struct NekoFixture<'a> {
    pub env: Env,
    pub admin: Address,
    pub alice: Address,
    pub bob: Address,
    pub whale: Address,

    // Token clients
    pub usdc: TokenClient<'a>,
    pub xlm: TokenClient<'a>,
    pub bond: TokenClient<'a>,
    pub backstop_token: TokenClient<'a>,

    // Oracle
    pub oracle: neko_oracle::Client<'a>,

    // Protocol contracts
    pub pool: LendingContractClient<'a>,
    pub backstop: NekoBackstopClient<'a>,

    // Asset symbols (used for pool queries)
    pub sym_usdc: Symbol,
    pub sym_xlm: Symbol,
    pub sym_bond: Symbol,
}

impl<'a> NekoFixture<'a> {
    pub fn create() -> NekoFixture<'a> {
        let env = Env::new_with_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        env.ledger().set(LedgerInfo {
            timestamp: 1_740_000_000,
            protocol_version: 25,
            sequence_number: 1_000,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 500_000,
            min_persistent_entry_ttl: 500_000,
            max_entry_ttl: 9_999_999,
        });

        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let whale = Address::generate(&env);
        let treasury = Address::generate(&env);

        // ── Tokens ───────────────────────────────────────────────────────────
        let usdc_addr = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let xlm_addr = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let bond_addr = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let backstop_token_addr = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();

        let usdc = TokenClient::new(&env, &usdc_addr);
        let xlm = TokenClient::new(&env, &xlm_addr);
        let bond = TokenClient::new(&env, &bond_addr);
        let backstop_token = TokenClient::new(&env, &backstop_token_addr);

        let sym_usdc = symbol_short!("USDC");
        let sym_xlm = symbol_short!("XLM");
        let sym_bond = symbol_short!("BOND");

        // ── Oracle ───────────────────────────────────────────────────────────
        // One oracle instance used for both neko_oracle + reflector_oracle.
        // All assets are Crypto → only reflector_oracle is queried at runtime.
        let oracle_assets = soroban_sdk::vec![
            &env,
            Asset::Other(sym_usdc.clone()),
            Asset::Other(sym_xlm.clone()),
            Asset::Other(sym_bond.clone()),
        ];
        let oracle_addr = env.register(
            neko_oracle::WASM,
            (
                admin.clone(),
                oracle_assets,
                Asset::Other(Symbol::new(&env, "USD")),
                7u32,  // decimals
                1u32,  // resolution (1 second – minimal for tests)
            ),
        );
        let oracle = neko_oracle::Client::new(&env, &oracle_addr);

        let now = env.ledger().timestamp();
        oracle.set_asset_price(&Asset::Other(sym_usdc.clone()), &PRICE_USDC, &now);
        oracle.set_asset_price(&Asset::Other(sym_xlm.clone()), &PRICE_XLM, &now);
        oracle.set_asset_price(&Asset::Other(sym_bond.clone()), &PRICE_BOND, &now);

        // ── Pool ─────────────────────────────────────────────────────────────
        let pool_addr = env.register(LendingContract, ());
        let pool = LendingContractClient::new(&env, &pool_addr);

        pool.initialize(
            &admin,
            &treasury,
            &oracle_addr, // neko_oracle (RWA)
            &oracle_addr, // reflector_oracle (Crypto) – same instance for tests
            &500_000u32,  // backstop_take_rate: 5%
            &1_000_000u32, // reserve_factor: 10%
            &40_000u32,   // origination_fee_rate: 0.4%
            &100_000u32,  // liquidation_fee_rate: 1%
        );

        // Pool starts OnIce → reserve params are applied immediately (no timelock)
        let default_params = default_interest_params();

        pool.set_token_contract(&sym_usdc, &usdc_addr, &AssetType::Crypto);
        pool.queue_set_reserve_params(&sym_usdc, &default_params);
        pool.apply_queued_reserve_params(&sym_usdc);

        pool.set_token_contract(&sym_xlm, &xlm_addr, &AssetType::Crypto);
        pool.queue_set_reserve_params(&sym_xlm, &default_params);
        pool.apply_queued_reserve_params(&sym_xlm);

        // BOND as collateral with AssetType::Crypto → oracle lookup by symbol
        pool.set_collateral_factor(&bond_addr, &7_000_000u32, &AssetType::Crypto, &sym_bond);
        pool.set_backstop_token(&backstop_token_addr);

        // ── Backstop ─────────────────────────────────────────────────────────
        let backstop_addr = env.register(NekoBackstop, ());
        let backstop = NekoBackstopClient::new(&env, &backstop_addr);
        backstop.initialize(&admin, &pool_addr, &backstop_token_addr, &BACKSTOP_THRESHOLD);
        pool.set_backstop_contract(&backstop_addr);

        // Whale deposits above threshold → pool becomes Active
        let whale_deposit = BACKSTOP_THRESHOLD + 5_000 * SCALAR_7;
        StellarAssetClient::new(&env, &backstop_token_addr).mint(&whale, &whale_deposit);
        backstop.deposit(&whale, &whale_deposit);

        // ── Initial liquidity (whale deposits into the lending pool) ─────────
        StellarAssetClient::new(&env, &usdc_addr).mint(&whale, &INITIAL_LIQUIDITY);
        StellarAssetClient::new(&env, &xlm_addr).mint(&whale, &INITIAL_LIQUIDITY);
        pool.deposit(&whale, &sym_usdc, &INITIAL_LIQUIDITY);
        pool.deposit(&whale, &sym_xlm, &INITIAL_LIQUIDITY);

        // ── Mint test user tokens ─────────────────────────────────────────────
        for user in [&alice, &bob] {
            StellarAssetClient::new(&env, &usdc_addr).mint(user, &INITIAL_USER_BALANCE);
            StellarAssetClient::new(&env, &xlm_addr).mint(user, &INITIAL_USER_BALANCE);
            StellarAssetClient::new(&env, &bond_addr).mint(user, &INITIAL_USER_BALANCE);
        }

        env.cost_estimate().budget().reset_unlimited();

        NekoFixture {
            env,
            admin,
            alice,
            bob,
            whale,
            usdc,
            xlm,
            bond,
            backstop_token,
            oracle,
            pool,
            backstop,
            sym_usdc,
            sym_xlm,
            sym_bond,
        }
    }

    /// Advance time by `seconds`. Also refreshes oracle prices so staleness check passes.
    /// Oracle prices are only updated when time actually advances (oracle requires strictly
    /// increasing timestamps).
    pub fn jump(&self, seconds: u64) {
        if seconds == 0 {
            return;
        }
        let new_ts = self.env.ledger().timestamp().saturating_add(seconds);
        let new_seq = self
            .env
            .ledger()
            .sequence()
            .saturating_add((seconds / 5) as u32);
        self.env.ledger().set(LedgerInfo {
            timestamp: new_ts,
            protocol_version: 25,
            sequence_number: new_seq,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 999_999,
            min_persistent_entry_ttl: 999_999,
            max_entry_ttl: 9_999_999,
        });
        self.oracle.set_asset_price(
            &Asset::Other(self.sym_usdc.clone()),
            &PRICE_USDC,
            &new_ts,
        );
        self.oracle
            .set_asset_price(&Asset::Other(self.sym_xlm.clone()), &PRICE_XLM, &new_ts);
        self.oracle.set_asset_price(
            &Asset::Other(self.sym_bond.clone()),
            &PRICE_BOND,
            &new_ts,
        );
    }

    // ── Invariant assertions ─────────────────────────────────────────────────

    /// Core solvency: liquid pool + outstanding loans must cover all bToken claims.
    /// pool_bal * SCALAR_12 + d_supply * d_rate >= b_supply * b_rate
    ///
    /// This accounts for the fact that some deposit value is currently lent out.
    pub fn assert_pool_solvency(&self) {
        for sym in [&self.sym_usdc, &self.sym_xlm] {
            let pool_bal = self.pool.get_pool_balance(sym);
            let b_supply = self.pool.get_b_token_supply(sym);
            let b_rate = self.pool.get_b_token_rate(sym);
            let d_supply = self.pool.get_d_token_supply(sym);
            let d_rate = self.pool.get_d_token_rate(sym);
            let total_assets = pool_bal * SCALAR_12 + d_supply * d_rate;
            let total_liabilities = b_supply * b_rate;
            assert!(
                total_assets >= total_liabilities,
                "solvency violated for {:?}: pool_bal={} d_supply={} d_rate={} b_supply={} b_rate={}",
                sym,
                pool_bal,
                d_supply,
                d_rate,
                b_supply,
                b_rate,
            );
        }
    }

    /// Utilization must never exceed 100%: liabilities (debt) <= deposit claims in underlying.
    pub fn assert_utilization(&self) {
        for sym in [&self.sym_usdc, &self.sym_xlm] {
            let pool_bal = self.pool.get_pool_balance(sym);
            let b_supply = self.pool.get_b_token_supply(sym);
            let b_rate = self.pool.get_b_token_rate(sym);
            let d_supply = self.pool.get_d_token_supply(sym);
            let d_rate = self.pool.get_d_token_rate(sym);
            assert!(
                pool_bal >= 0,
                "negative pool balance for {sym:?}: {pool_bal}"
            );
            assert!(b_supply >= 0, "negative b_supply for {sym:?}: {b_supply}");
            assert!(d_supply >= 0, "negative d_supply for {sym:?}: {d_supply}");

            let total_supply = b_supply.saturating_mul(b_rate).saturating_div(SCALAR_12);
            let total_liabilities = d_supply.saturating_mul(d_rate).saturating_div(SCALAR_12);
            assert!(
                total_liabilities <= total_supply,
                "utilization > 100% for {sym:?}: liabilities={total_liabilities} supply={total_supply}"
            );
        }
    }

    /// Backstop total must equal sum of all tracked user active amounts
    /// (active = deposited - queued, i.e. UserBalance.amount).
    pub fn assert_backstop_consistency(&self) {
        let whale_active = self.backstop.get_user_balance(&self.whale).amount;
        let alice_active = self.backstop.get_user_balance(&self.alice).amount;
        let bob_active = self.backstop.get_user_balance(&self.bob).amount;
        let expected_total = whale_active + alice_active + bob_active;
        let actual_total = self.backstop.get_total();
        assert_eq!(
            actual_total, expected_total,
            "backstop total mismatch: actual={actual_total} expected={expected_total}"
        );
    }

    /// All invariants combined.
    pub fn assert_invariants(&self) {
        self.assert_pool_solvency();
        self.assert_utilization();
        self.assert_backstop_consistency();
    }
}

pub fn default_interest_params() -> InterestRateParams {
    InterestRateParams {
        target_util: 7_500_000,
        max_util: 9_500_000,
        r_base: 100_000,
        r_one: 500_000,
        r_two: 5_000_000,
        r_three: 15_000_000,
        reactivity: 200,
        l_factor: 10_000_000,
        supply_cap: 0,
        enabled: true,
    }
}

/// Create a fresh fixture with initial lending data — two users have deposited
/// and borrowed at roughly 65% utilization. Used as the starting state for fuzz targets.
pub fn create_fixture_with_data<'a>() -> NekoFixture<'a> {
    let fixture = NekoFixture::create();

    // Alice deposits USDC and borrows XLM (using BOND as collateral)
    let alice_bond_collateral = 10_000 * SCALAR_7;
    fixture
        .pool
        .add_collateral(&fixture.alice, &fixture.bond.address, &alice_bond_collateral);
    fixture
        .pool
        .deposit(&fixture.alice, &fixture.sym_usdc, &(5_000 * SCALAR_7));
    fixture
        .pool
        .borrow(&fixture.alice, &fixture.sym_xlm, &(1_000 * SCALAR_7));

    // Bob deposits XLM and borrows USDC (using BOND as collateral)
    let bob_bond_collateral = 10_000 * SCALAR_7;
    fixture
        .pool
        .add_collateral(&fixture.bob, &fixture.bond.address, &bob_bond_collateral);
    fixture
        .pool
        .deposit(&fixture.bob, &fixture.sym_xlm, &(5_000 * SCALAR_7));
    fixture
        .pool
        .borrow(&fixture.bob, &fixture.sym_usdc, &(500 * SCALAR_7));

    fixture.jump(60 * 60); // 1 hour — let interest start accruing

    fixture.env.cost_estimate().budget().reset_unlimited();
    fixture
}

/// Lending fixture with borrowers, then BOND price crashed so positions are **underwater** (HF below 1.0).
/// Used by `fuzz_liquidation` to exercise `initiate_liquidation` / `fill_auction` paths.
pub fn create_fixture_for_liquidation_fuzz<'a>() -> NekoFixture<'a> {
    let fixture = create_fixture_with_data();
    // SEP-40 requires strictly increasing price timestamps vs last update (same as `jump()`).
    let ts = fixture.env.ledger().timestamp().saturating_add(1);
    // Crash BOND to a tiny positive price so collateral value * CF falls below debt (HF below 1.0).
    fixture.oracle.set_asset_price(
        &Asset::Other(fixture.sym_bond.clone()),
        &1i128,
        &ts,
    );
    fixture.env.cost_estimate().budget().reset_unlimited();
    fixture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liquidation_fuzz_fixture_borrowers_are_liquidatable() {
        let f = create_fixture_for_liquidation_fuzz();
        let hf_alice = f.pool.calculate_health_factor(&f.alice);
        let hf_bob = f.pool.calculate_health_factor(&f.bob);
        assert!(
            hf_alice < 10_000_000,
            "alice HF should be < 1.0 (10M), got {hf_alice}"
        );
        assert!(
            hf_bob < 10_000_000,
            "bob HF should be < 1.0 (10M), got {hf_bob}"
        );
    }

    #[test]
    fn lending_fixture_solvency_after_seed_data() {
        let f = create_fixture_with_data();
        f.assert_pool_solvency();
        f.assert_utilization();
    }
}
