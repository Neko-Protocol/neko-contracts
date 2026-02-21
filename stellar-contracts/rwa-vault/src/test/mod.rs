#![cfg(test)]
extern crate std;

use soroban_sdk::{
    Address, Env, String, contract, contractimpl, symbol_short,
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
};

use crate::common::types::{RiskTier, VaultConfig, VaultStatus};
use crate::{VaultContract, VaultContractClient};

// ============================================================================
// Mock Adapter Contract (in-memory, no real lending)
// ============================================================================

#[contract]
struct MockAdapter;

#[contractimpl]
impl MockAdapter {
    pub fn initialize(env: Env, deposit_token: Address, _vault: Address) {
        env.storage()
            .instance()
            .set(&symbol_short!("TOKEN"), &deposit_token);
        env.storage().instance().set(&symbol_short!("BAL"), &0i128);
    }

    pub fn a_deposit(env: Env, amount: i128, _from: Address) -> i128 {
        // In tests, vault transfers tokens to adapter before calling this.
        // We just track the virtual balance.
        let bal: i128 = env
            .storage()
            .instance()
            .get(&symbol_short!("BAL"))
            .unwrap_or(0);
        let new_bal = bal + amount;
        env.storage()
            .instance()
            .set(&symbol_short!("BAL"), &new_bal);
        new_bal
    }

    pub fn a_withdraw(env: Env, amount: i128, to: Address) -> i128 {
        let bal: i128 = env
            .storage()
            .instance()
            .get(&symbol_short!("BAL"))
            .unwrap_or(0);
        let actual = amount.min(bal);
        env.storage()
            .instance()
            .set(&symbol_short!("BAL"), &(bal - actual));

        // Transfer tokens from adapter to vault (to)
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&symbol_short!("TOKEN"))
            .unwrap();
        let token = TokenClient::new(&env, &token_addr);
        token.transfer(&env.current_contract_address(), &to, &actual);

        actual
    }

    pub fn a_balance(env: Env, _from: Address) -> i128 {
        env.storage()
            .instance()
            .get(&symbol_short!("BAL"))
            .unwrap_or(0)
    }

    pub fn a_get_apy(_env: Env) -> u32 {
        500 // 5% in BPS
    }

    pub fn a_harvest(_env: Env, _to: Address) -> i128 {
        0 // No explicit harvest; yield embedded in b_rate
    }
}

// ============================================================================
// Test helpers
// ============================================================================

fn default_config() -> VaultConfig {
    VaultConfig {
        management_fee_bps: 50,       // 0.5%
        performance_fee_bps: 1000,    // 10%
        min_liquidity_bps: 500,       // 5%
        max_protocol_bps: 9000,       // 90%
        rebalance_threshold_bps: 200, // 2%
    }
}

fn create_vault<'a>(env: &'a Env, admin: &Address, token: &Address) -> VaultContractClient<'a> {
    let contract_id = env.register(VaultContract, ());
    let client = VaultContractClient::new(env, &contract_id);
    client.initialize(
        admin,
        admin, // manager = admin for tests
        token,
        &String::from_str(env, "Neko CETES Vault"),
        &String::from_str(env, "vCETES"),
        &7u32,
        &default_config(),
    );
    client
}

fn create_token<'a>(env: &'a Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token = TokenClient::new(env, &sac.address());
    let token_admin = StellarAssetClient::new(env, &sac.address());
    (token, token_admin)
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_initialize() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let vault = create_vault(&env, &admin, &token.address);

    assert_eq!(vault.get_status(), VaultStatus::Active);
    assert_eq!(vault.get_total_shares(), 0);
    assert_eq!(vault.get_liquid_reserve(), 0);
    assert_eq!(vault.get_nav(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let vault = create_vault(&env, &admin, &token.address);

    // Try to initialize again — should panic with AlreadyInitialized (#1)
    vault.initialize(
        &admin,
        &admin,
        &token.address,
        &String::from_str(&env, "Neko CETES Vault"),
        &String::from_str(&env, "vCETES"),
        &7u32,
        &default_config(),
    );
}

#[test]
fn test_deposit_first_is_one_to_one() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    // Mint 1000 tokens to user
    token_admin.mint(&user, &1_000_0000000i128); // 1000 tokens (7 decimals)

    let vault = create_vault(&env, &admin, &token.address);

    // First deposit: 100 tokens → should receive 100 shares (1:1)
    let amount = 100_0000000i128;
    let shares = vault.deposit(&user, &amount);

    assert_eq!(shares, amount); // 1:1 for first deposit
    assert_eq!(vault.get_total_shares(), amount);
    assert_eq!(vault.get_liquid_reserve(), amount);
    assert_eq!(vault.balance(&user), amount);
}

#[test]
fn test_deposit_share_price_proportional() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user1, &1_000_0000000i128);
    token_admin.mint(&user2, &1_000_0000000i128);

    let vault = create_vault(&env, &admin, &token.address);

    // First deposit: 100 tokens → 100 shares
    let amount1 = 100_0000000i128;
    vault.deposit(&user1, &amount1);

    // Second deposit: same amount → same shares (NAV unchanged)
    let amount2 = 100_0000000i128;
    let shares2 = vault.deposit(&user2, &amount2);

    assert_eq!(shares2, amount2); // Same amount since NAV = 100 and total_shares = 100

    assert_eq!(vault.get_total_shares(), amount1 + amount2);
    assert_eq!(vault.get_liquid_reserve(), amount1 + amount2);
}

#[test]
fn test_withdraw_from_liquid_reserve() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user, &1_000_0000000i128);

    let vault = create_vault(&env, &admin, &token.address);
    let amount = 100_0000000i128;
    let shares = vault.deposit(&user, &amount);

    // Withdraw half
    let half_shares = shares / 2;
    let received = vault.withdraw(&user, &half_shares);

    assert_eq!(received, amount / 2);
    assert_eq!(vault.balance(&user), shares - half_shares);
    assert_eq!(vault.get_liquid_reserve(), amount - received);
}

#[test]
fn test_withdraw_full() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user, &1_000_0000000i128);

    let vault = create_vault(&env, &admin, &token.address);
    let amount = 100_0000000i128;
    let shares = vault.deposit(&user, &amount);

    let received = vault.withdraw(&user, &shares);
    assert_eq!(received, amount);
    assert_eq!(vault.balance(&user), 0);
    assert_eq!(vault.get_total_shares(), 0);
}

#[test]
fn test_add_remove_protocol() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let adapter = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let vault = create_vault(&env, &admin, &token.address);

    let id = symbol_short!("POOL1");
    vault.add_protocol(&id, &adapter, &5000u32, &RiskTier::Low);

    let protocols = vault.get_protocols();
    assert_eq!(protocols.len(), 1);
    assert_eq!(protocols.get(0).unwrap().0, id);

    vault.remove_protocol(&id);
    assert_eq!(vault.get_protocols().len(), 0);
}

#[test]
fn test_pause_blocks_deposit_not_withdraw() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user, &1_000_0000000i128);

    let vault = create_vault(&env, &admin, &token.address);
    let amount = 100_0000000i128;
    let shares = vault.deposit(&user, &amount);

    // Pause the vault
    vault.pause();
    assert_eq!(vault.get_status(), VaultStatus::Paused);

    // Withdraw should still work
    let received = vault.withdraw(&user, &shares);
    assert_eq!(received, amount);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_paused_deposit_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user, &1_000_0000000i128);

    let vault = create_vault(&env, &admin, &token.address);
    vault.pause();

    // Should panic with VaultNotActive (#6)
    vault.deposit(&user, &100_0000000i128);
}

#[test]
fn test_sep41_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user1, &1_000_0000000i128);

    let vault = create_vault(&env, &admin, &token.address);
    let amount = 100_0000000i128;
    vault.deposit(&user1, &amount);

    // Transfer 30 shares from user1 to user2
    vault.transfer(&user1, &user2, &30_0000000i128);

    assert_eq!(vault.balance(&user1), 70_0000000i128);
    assert_eq!(vault.balance(&user2), 30_0000000i128);
}

#[test]
fn test_sep41_approve_transfer_from() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let spender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user1, &1_000_0000000i128);

    let vault = create_vault(&env, &admin, &token.address);
    vault.deposit(&user1, &100_0000000i128);

    // Approve spender to transfer 50 shares
    vault.approve(&user1, &spender, &50_0000000i128, &1000u32);
    assert_eq!(vault.allowance(&user1, &spender), 50_0000000i128);

    // Spender transfers 30 from user1 to user2
    vault.transfer_from(&spender, &user1, &user2, &30_0000000i128);

    assert_eq!(vault.balance(&user1), 70_0000000i128);
    assert_eq!(vault.balance(&user2), 30_0000000i128);
    assert_eq!(vault.allowance(&user1, &spender), 20_0000000i128);
}

#[test]
fn test_decimals_name_symbol() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let vault = create_vault(&env, &admin, &token.address);

    assert_eq!(vault.decimals(), 7u32);
    assert_eq!(vault.name(), String::from_str(&env, "Neko CETES Vault"));
    assert_eq!(vault.symbol(), String::from_str(&env, "vCETES"));
}
