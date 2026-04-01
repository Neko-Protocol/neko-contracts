#![allow(unused)]
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::arbitrary::arbitrary::{self, Arbitrary, Unstructured};
use soroban_sdk::{testutils::Address as _, Symbol};
use test_suites::NekoFixture;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// `try_*` clients return `Result<T, Result<E, InvokeError>>` with `E` = `#[contracterror]` enum.
pub type TryInvokeResult<T, E> = Result<T, Result<E, soroban_sdk::InvokeError>>;

/// Panic only when the contract surfaced `WasmVm` + `InvalidAction` (plain `panic!` in WASM).
/// Same criterion as Blend v2 `test-suites/fuzz/lib.rs`. Protocol errors (`InsufficientBalance`, etc.)
/// are normal; `E` must convert to `soroban_sdk::Error` for the check (Soroban `#[contracterror]`).
#[track_caller]
pub fn verify_contract_result<T, E>(env: &soroban_sdk::Env, r: &TryInvokeResult<T, E>)
where
    E: Copy + Into<soroban_sdk::Error>,
{
    use soroban_sdk::testutils::Events;
    use soroban_sdk::xdr::{ScErrorCode, ScErrorType};
    match r {
        Err(Ok(e)) => {
            let sdk_err: soroban_sdk::Error = (*e).into();
            if sdk_err.is_type(ScErrorType::WasmVm) && sdk_err.is_code(ScErrorCode::InvalidAction) {
                let msg = "contract failed with InvalidAction — unexpected panic?";
                eprintln!("{msg}");
                eprintln!("recent events (10):");
                let contract_events = env.events().all();
                let slice = contract_events.events();
                let n = slice.len().min(10);
                let start = slice.len().saturating_sub(n);
                for (i, event) in slice[start..].iter().rev().enumerate() {
                    eprintln!("{i}: {event:?}");
                }
                panic!("{msg}");
            }
        }
        _ => {}
    }
}

// ── Arbitrary wrappers ────────────────────────────────────────────────────────

/// Non-negative i128 bounded by i64::MAX to avoid overflow in internal math.
#[derive(Arbitrary, Debug, Clone)]
pub struct NatI128(
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=(i64::MAX as i128)))]
    pub i128,
);

/// Borrower used as liquidation target.
#[derive(Arbitrary, Debug, Clone, Copy)]
pub enum LiquidationVictim {
    Alice,
    Bob,
}

/// Which debt asset to operate on.
#[derive(Arbitrary, Debug, Clone, Copy)]
pub enum DebtAsset {
    Usdc,
    Xlm,
}

impl DebtAsset {
    pub fn symbol<'a>(&self, fixture: &'a NekoFixture<'_>) -> &'a Symbol {
        match self {
            DebtAsset::Usdc => &fixture.sym_usdc,
            DebtAsset::Xlm => &fixture.sym_xlm,
        }
    }
}

// ── Lending commands ──────────────────────────────────────────────────────────

/// deposit `amount` of `asset` into the lending pool.
#[derive(Arbitrary, Debug)]
pub struct Deposit {
    pub asset: DebtAsset,
    pub amount: NatI128,
}

/// Withdraw `b_tokens` bTokens from the lending pool.
#[derive(Arbitrary, Debug)]
pub struct Withdraw {
    pub asset: DebtAsset,
    pub b_tokens: NatI128,
}

/// Add `amount` of BOND collateral to the CDP.
#[derive(Arbitrary, Debug)]
pub struct AddCollateral {
    pub amount: NatI128,
}

/// Remove `amount` of BOND collateral from the CDP.
#[derive(Arbitrary, Debug)]
pub struct RemoveCollateral {
    pub amount: NatI128,
}

/// Borrow `amount` of `asset`.
#[derive(Arbitrary, Debug)]
pub struct Borrow {
    pub asset: DebtAsset,
    pub amount: NatI128,
}

/// Repay `d_tokens` dTokens of outstanding debt.
#[derive(Arbitrary, Debug)]
pub struct Repay {
    pub asset: DebtAsset,
    pub d_tokens: NatI128,
}

/// Advance time by `amount` seconds.
#[derive(Arbitrary, Debug)]
pub struct PassTime {
    pub amount: u64,
}

/// Start a Dutch liquidation auction (`initiate_liquidation`).
#[derive(Arbitrary, Debug)]
pub struct InitiateLiquidation {
    pub victim: LiquidationVictim,
    pub debt_asset: DebtAsset,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(1_000_000u32..=10_000_000u32))]
    pub liquidation_percent: u32,
}

// ── Backstop commands ─────────────────────────────────────────────────────────

/// Deposit `amount` of backstop tokens.
#[derive(Arbitrary, Debug)]
pub struct BackstopDeposit {
    pub amount: NatI128,
}

/// Queue `amount` for withdrawal from the backstop.
#[derive(Arbitrary, Debug)]
pub struct BackstopQueue {
    pub amount: NatI128,
}

/// Cancel the most-recent queued withdrawal.
#[derive(Arbitrary, Debug)]
pub struct BackstopDequeue;

/// Complete the oldest expired withdrawal.
#[derive(Arbitrary, Debug)]
pub struct BackstopWithdraw {
    pub amount: NatI128,
}

// ── run() impls ───────────────────────────────────────────────────────────────

impl Deposit {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let sym = self.asset.symbol(fixture);
        let r = fixture.pool.try_deposit(user, sym, &self.amount.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl Withdraw {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let sym = self.asset.symbol(fixture);
        let r = fixture.pool.try_withdraw(user, sym, &self.b_tokens.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl AddCollateral {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let r = fixture
            .pool
            .try_add_collateral(user, &fixture.bond.address, &self.amount.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl RemoveCollateral {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let r = fixture
            .pool
            .try_remove_collateral(user, &fixture.bond.address, &self.amount.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl Borrow {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let sym = self.asset.symbol(fixture);
        let r = fixture.pool.try_borrow(user, sym, &self.amount.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl Repay {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let sym = self.asset.symbol(fixture);
        let r = fixture.pool.try_repay(user, sym, &self.d_tokens.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl PassTime {
    pub fn run(&self, fixture: &NekoFixture<'_>) {
        let secs = self.amount % (30 * 24 * 60 * 60);
        fixture.jump(secs);
    }
}

impl InitiateLiquidation {
    pub fn run(&self, fixture: &NekoFixture<'_>, pending_auction: &mut Option<u32>) {
        let borrower = match self.victim {
            LiquidationVictim::Alice => fixture.alice.clone(),
            LiquidationVictim::Bob => fixture.bob.clone(),
        };
        let sym = self.debt_asset.symbol(fixture);
        let r = fixture.pool.try_initiate_liquidation(
            &borrower,
            &fixture.bond.address,
            sym,
            &self.liquidation_percent,
        );
        verify_contract_result(&fixture.env, &r);
        if let Ok(Ok(id)) = r {
            *pending_auction = Some(id);
        }
    }
}

/// Fill the last successfully opened liquidation auction (if any).
pub fn run_fill_liquidation(
    fixture: &NekoFixture<'_>,
    pending_auction: &mut Option<u32>,
    liquidator: &soroban_sdk::Address,
) {
    let Some(id) = *pending_auction else {
        return;
    };
    let r = fixture.pool.try_fill_auction(&id, liquidator);
    verify_contract_result(&fixture.env, &r);
    if r.is_ok() {
        *pending_auction = None;
    }
}

impl BackstopDeposit {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        use soroban_sdk::token::StellarAssetClient;
        StellarAssetClient::new(&fixture.env, &fixture.backstop_token.address)
            .mint(user, &self.amount.0);
        let r = fixture.backstop.try_deposit(user, &self.amount.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl BackstopQueue {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let r = fixture.backstop.try_queue_withdrawal(user, &self.amount.0);
        verify_contract_result(&fixture.env, &r);
    }
}

impl BackstopDequeue {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let r = fixture.backstop.try_dequeue_withdrawal(user);
        verify_contract_result(&fixture.env, &r);
    }
}

impl BackstopWithdraw {
    pub fn run(&self, fixture: &NekoFixture<'_>, user: &soroban_sdk::Address) {
        let r = fixture.backstop.try_withdraw(user, &self.amount.0);
        verify_contract_result(&fixture.env, &r);
    }
}
