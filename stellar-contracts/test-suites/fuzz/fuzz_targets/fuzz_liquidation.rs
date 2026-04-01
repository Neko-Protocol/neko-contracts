#![allow(unused)]
#![no_main]

use fuzz_common::{
    AddCollateral, Borrow, DebtAsset, Deposit, InitiateLiquidation, NatI128, PassTime,
    RemoveCollateral, Repay, Withdraw, run_fill_liquidation,
};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::arbitrary::arbitrary::{self, Arbitrary};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use test_suites::{create_fixture_for_liquidation_fuzz, INITIAL_USER_BALANCE};

/// Same operation mix as `fuzz_pool_general`, plus liquidation auction open/fill.
#[derive(Arbitrary, Debug)]
struct Input {
    alice_usdc: NatI128,
    alice_xlm: NatI128,
    alice_bond: NatI128,
    bob_usdc: NatI128,
    bob_xlm: NatI128,
    bob_bond: NatI128,

    commands: [Command; 12],
}

#[derive(Arbitrary, Debug)]
enum Command {
    PassTime(PassTime),

    AliceDeposit(Deposit),
    AliceWithdraw(Withdraw),
    AliceAddCollateral(AddCollateral),
    AliceRemoveCollateral(RemoveCollateral),
    AliceBorrow(Borrow),
    AliceRepay(Repay),

    BobDeposit(Deposit),
    BobWithdraw(Withdraw),
    BobAddCollateral(AddCollateral),
    BobRemoveCollateral(RemoveCollateral),
    BobBorrow(Borrow),
    BobRepay(Repay),

    InitiateLiquidation(InitiateLiquidation),
    /// Uses the liquidator address (minted with large USDC/XLM balances).
    FillLiquidation,
}

fuzz_target!(|input: Input| {
    let fixture = create_fixture_for_liquidation_fuzz();

    StellarAssetClient::new(&fixture.env, &fixture.usdc.address)
        .mint(&fixture.alice, &input.alice_usdc.0);
    StellarAssetClient::new(&fixture.env, &fixture.xlm.address)
        .mint(&fixture.alice, &input.alice_xlm.0);
    StellarAssetClient::new(&fixture.env, &fixture.bond.address)
        .mint(&fixture.alice, &input.alice_bond.0);

    StellarAssetClient::new(&fixture.env, &fixture.usdc.address)
        .mint(&fixture.bob, &input.bob_usdc.0);
    StellarAssetClient::new(&fixture.env, &fixture.xlm.address)
        .mint(&fixture.bob, &input.bob_xlm.0);
    StellarAssetClient::new(&fixture.env, &fixture.bond.address)
        .mint(&fixture.bob, &input.bob_bond.0);

    let liquidator = soroban_sdk::Address::generate(&fixture.env);
    let deep = INITIAL_USER_BALANCE.saturating_mul(100);
    StellarAssetClient::new(&fixture.env, &fixture.usdc.address).mint(&liquidator, &deep);
    StellarAssetClient::new(&fixture.env, &fixture.xlm.address).mint(&liquidator, &deep);

    let mut prev_d_rate_usdc = fixture.pool.get_d_token_rate(&fixture.sym_usdc);
    let mut prev_d_rate_xlm = fixture.pool.get_d_token_rate(&fixture.sym_xlm);
    let mut pending_auction: Option<u32> = None;

    for cmd in &input.commands {
        cmd.run(&fixture, &mut pending_auction, &liquidator);

        fixture.assert_pool_solvency();
        fixture.assert_utilization();

        let d_rate_usdc = fixture.pool.get_d_token_rate(&fixture.sym_usdc);
        let d_rate_xlm = fixture.pool.get_d_token_rate(&fixture.sym_xlm);
        assert!(
            d_rate_usdc >= prev_d_rate_usdc,
            "d_rate_usdc decreased: {prev_d_rate_usdc} → {d_rate_usdc}"
        );
        assert!(
            d_rate_xlm >= prev_d_rate_xlm,
            "d_rate_xlm decreased: {prev_d_rate_xlm} → {d_rate_xlm}"
        );
        prev_d_rate_usdc = d_rate_usdc;
        prev_d_rate_xlm = d_rate_xlm;

        assert!(fixture.pool.get_pool_balance(&fixture.sym_usdc) >= 0);
        assert!(fixture.pool.get_pool_balance(&fixture.sym_xlm) >= 0);

        for user in [&fixture.alice, &fixture.bob] {
            assert!(fixture.pool.get_b_token_balance(user, &fixture.sym_usdc) >= 0);
            assert!(fixture.pool.get_b_token_balance(user, &fixture.sym_xlm) >= 0);
        }
    }
});

impl Command {
    fn run(
        &self,
        fixture: &test_suites::NekoFixture<'_>,
        pending: &mut Option<u32>,
        liquidator: &soroban_sdk::Address,
    ) {
        match self {
            Command::PassTime(cmd) => cmd.run(fixture),

            Command::AliceDeposit(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceWithdraw(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceAddCollateral(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceRemoveCollateral(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceBorrow(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceRepay(cmd) => cmd.run(fixture, &fixture.alice.clone()),

            Command::BobDeposit(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobWithdraw(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobAddCollateral(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobRemoveCollateral(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobBorrow(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobRepay(cmd) => cmd.run(fixture, &fixture.bob.clone()),

            Command::InitiateLiquidation(cmd) => cmd.run(fixture, pending),
            Command::FillLiquidation => run_fill_liquidation(fixture, pending, liquidator),
        }
    }
}
