#![allow(unused)]
#![no_main]

use fuzz_common::{
    BackstopDeposit, BackstopDequeue, BackstopQueue, BackstopWithdraw, PassTime,
};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::arbitrary::arbitrary::{self, Arbitrary, Unstructured};
use test_suites::create_fixture_with_data;

// ── Input schema ──────────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug)]
struct Input {
    commands: [Command; 24],
}

#[derive(Arbitrary, Debug)]
enum Command {
    /// Needed so Q4W entries can expire and `withdraw` exercises real paths.
    PassTime(PassTime),

    AliceDeposit(BackstopDeposit),
    AliceQueue(BackstopQueue),
    AliceDequeue(BackstopDequeue),
    AliceWithdraw(BackstopWithdraw),

    BobDeposit(BackstopDeposit),
    BobQueue(BackstopQueue),
    BobDequeue(BackstopDequeue),
    BobWithdraw(BackstopWithdraw),

    /// Whale seeds most backstop TVL in the fixture — stress queue thresholds / pool state pushes.
    WhaleDeposit(BackstopDeposit),
    WhaleQueue(BackstopQueue),
    WhaleDequeue(BackstopDequeue),
    WhaleWithdraw(BackstopWithdraw),
}

// ── Fuzz target ───────────────────────────────────────────────────────────────

fuzz_target!(|input: Input| {
    let fixture = create_fixture_with_data();

    for cmd in &input.commands {
        cmd.run(&fixture);

        // ── Invariants after every command ────────────────────────────────────

        fixture.assert_backstop_consistency();

        // Backstop notifies the pool on each op — lending book must stay solvent.
        fixture.assert_pool_solvency();
        fixture.assert_utilization();

        assert!(fixture.backstop.get_total() >= 0, "negative backstop total");

        for user in [&fixture.alice, &fixture.bob, &fixture.whale] {
            let bal = fixture.backstop.get_user_balance(user);
            assert!(bal.amount >= 0, "negative user backstop amount for {user:?}");
            assert!(
                bal.q4w.len() <= 20,
                "q4w overflow for {user:?}: {} entries",
                bal.q4w.len()
            );
            for q in bal.q4w.iter() {
                assert!(q.amount > 0, "zero/negative q4w entry for {user:?}");
            }
        }
    }
});

impl Command {
    fn run(&self, fixture: &test_suites::NekoFixture<'_>) {
        match self {
            Command::PassTime(cmd) => cmd.run(fixture),

            Command::AliceDeposit(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceQueue(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceDequeue(cmd) => cmd.run(fixture, &fixture.alice.clone()),
            Command::AliceWithdraw(cmd) => cmd.run(fixture, &fixture.alice.clone()),

            Command::BobDeposit(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobQueue(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobDequeue(cmd) => cmd.run(fixture, &fixture.bob.clone()),
            Command::BobWithdraw(cmd) => cmd.run(fixture, &fixture.bob.clone()),

            Command::WhaleDeposit(cmd) => cmd.run(fixture, &fixture.whale.clone()),
            Command::WhaleQueue(cmd) => cmd.run(fixture, &fixture.whale.clone()),
            Command::WhaleDequeue(cmd) => cmd.run(fixture, &fixture.whale.clone()),
            Command::WhaleWithdraw(cmd) => cmd.run(fixture, &fixture.whale.clone()),
        }
    }
}
