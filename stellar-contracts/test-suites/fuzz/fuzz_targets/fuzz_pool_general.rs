#![allow(unused)]
#![no_main]

use fuzz_common::{
    AddCollateral, Borrow, DebtAsset, Deposit, NatI128, PassTime, RemoveCollateral, Repay,
    Withdraw,
};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::arbitrary::arbitrary::{self, Arbitrary, Unstructured};
use soroban_sdk::testutils::Address as _;
use test_suites::{create_fixture_with_data, INITIAL_USER_BALANCE, SCALAR_7};

// ── Input schema ─────────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug)]
struct Input {
    // Token balances to mint to alice and bob before running commands.
    // Bounded to avoid absurd numbers that would trivially break arithmetic.
    alice_usdc: NatI128,
    alice_xlm: NatI128,
    alice_bond: NatI128,
    bob_usdc: NatI128,
    bob_xlm: NatI128,
    bob_bond: NatI128,

    commands: [Command; 10],
}

#[derive(Arbitrary, Debug)]
enum Command {
    // Time
    PassTime(PassTime),

    // Alice
    AliceDeposit(Deposit),
    AliceWithdraw(Withdraw),
    AliceAddCollateral(AddCollateral),
    AliceRemoveCollateral(RemoveCollateral),
    AliceBorrow(Borrow),
    AliceRepay(Repay),

    // Bob
    BobDeposit(Deposit),
    BobWithdraw(Withdraw),
    BobAddCollateral(AddCollateral),
    BobRemoveCollateral(RemoveCollateral),
    BobBorrow(Borrow),
    BobRepay(Repay),
}

// ── Fuzz target ───────────────────────────────────────────────────────────────

fuzz_target!(|input: Input| {
    let fixture = create_fixture_with_data();

    use soroban_sdk::token::StellarAssetClient;

    // Mint additional tokens as requested by the fuzzer
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

    // Track d_rate before each command to verify monotonicity
    let mut prev_d_rate_usdc = fixture.pool.get_d_token_rate(&fixture.sym_usdc);
    let mut prev_d_rate_xlm = fixture.pool.get_d_token_rate(&fixture.sym_xlm);

    for cmd in &input.commands {
        cmd.run(&fixture);

        // ── Post-command invariants ───────────────────────────────────────────

        // 1. Solvency + utilization
        fixture.assert_pool_solvency();
        fixture.assert_utilization();

        // 2. d_rate must be monotonically non-decreasing
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

        // 3. No negative pool balances
        assert!(fixture.pool.get_pool_balance(&fixture.sym_usdc) >= 0);
        assert!(fixture.pool.get_pool_balance(&fixture.sym_xlm) >= 0);

        // 4. No negative user bToken balances
        for user in [&fixture.alice, &fixture.bob] {
            assert!(
                fixture.pool.get_b_token_balance(user, &fixture.sym_usdc) >= 0
            );
            assert!(
                fixture.pool.get_b_token_balance(user, &fixture.sym_xlm) >= 0
            );
        }
    }
});

impl Command {
    fn run(&self, fixture: &test_suites::NekoFixture<'_>) {
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
        }
    }
}
