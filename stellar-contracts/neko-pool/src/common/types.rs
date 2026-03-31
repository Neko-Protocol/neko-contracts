use soroban_sdk::{Address, Map, Symbol, contracttype};

// ============================================================================
// SCALAR CONSTANTS
// ============================================================================

/// 7 decimals - Used for interest rate parameters, utilization, health factors
/// Example: 75% = 7_500_000, 1% = 100_000
pub const SCALAR_7: i128 = 10_000_000;

/// 12 decimals - Used for bToken/dToken rates (exchange rates)
/// Example: 1:1 rate = 1_000_000_000_000
pub const SCALAR_12: i128 = 1_000_000_000_000;

/// Seconds per year for interest calculations
pub const SECONDS_PER_YEAR: u64 = 31_536_000; // 365 days

// ============================================================================
// TTL CONSTANTS
// ============================================================================

/// Ledgers per day (~5 seconds per ledger on Stellar)
pub const ONE_DAY_LEDGERS: u32 = 17280;

/// Instance storage TTL (contract config, admin) - 30 days
pub const INSTANCE_TTL: u32 = ONE_DAY_LEDGERS * 30;
pub const INSTANCE_BUMP: u32 = ONE_DAY_LEDGERS * 31;

/// User storage TTL (positions, balances, CDPs) - 100 days
pub const USER_TTL: u32 = ONE_DAY_LEDGERS * 100;
pub const USER_BUMP: u32 = ONE_DAY_LEDGERS * 120;

// ============================================================================
// HEALTH FACTOR CONSTANTS (7 decimals)
// ============================================================================

/// Health factor representing 1.0 (no margin)
#[allow(dead_code)]
pub const HEALTH_FACTOR_ONE: i128 = 10_000_000; // 1.0

/// Minimum health factor after borrow/remove_collateral operations
/// Ensures CDPs maintain safety margin above liquidation threshold
pub const MIN_HEALTH_FACTOR: i128 = 11_000_000; // 1.1 = 110%

/// Maximum health factor after liquidation
/// Prevents over-liquidation that would leave borrower with excess collateral
pub const MAX_HEALTH_FACTOR: i128 = 11_500_000; // 1.15 = 115%

// ============================================================================
// AUCTION CONSTANTS
// ============================================================================

/// Auction duration in blocks (for Dutch auctions)
/// ~17 minutes on Stellar (200 blocks * ~5 sec/block)
pub const AUCTION_DURATION_BLOCKS: u32 = 200;

/// Maximum blocks before auction is considered stale and can be deleted
#[allow(dead_code)]
pub const AUCTION_MAX_BLOCKS: u32 = 500;

/// Admin proposal TTL — 7 days in ledgers. Proposed admin must accept within this window.
pub const PROPOSAL_TTL: u32 = ONE_DAY_LEDGERS * 7;
pub const PROPOSAL_BUMP: u32 = PROPOSAL_TTL + ONE_DAY_LEDGERS;

/// Auction temporary storage TTL — 7 days in ledgers.
/// Auctions that are never filled auto-expire after this window (Blend v2 pattern).
pub const AUCTION_TTL: u32 = ONE_DAY_LEDGERS * 7;
pub const AUCTION_BUMP: u32 = AUCTION_TTL + ONE_DAY_LEDGERS;

// ============================================================================
// FEE CONSTANTS (7 decimals)
// ============================================================================

/// Reserve factor: 10% of interest goes to treasury
#[allow(dead_code)]
pub const DEFAULT_RESERVE_FACTOR: u32 = 1_000_000;

/// Origination fee: 0.4% charged on borrow
#[allow(dead_code)]
pub const DEFAULT_ORIGINATION_FEE_RATE: u32 = 40_000;

/// Liquidation fee: 1% of collateral received goes to treasury
#[allow(dead_code)]
pub const DEFAULT_LIQUIDATION_FEE_RATE: u32 = 100_000;

// ============================================================================
// BACKSTOP CONSTANTS
// ============================================================================

/// Backstop withdrawal queue timing
pub const BACKSTOP_WITHDRAWAL_QUEUE_DAYS: u64 = 17;
pub const BACKSTOP_WITHDRAWAL_QUEUE_SECONDS: u64 = BACKSTOP_WITHDRAWAL_QUEUE_DAYS * 24 * 60 * 60;
// MAX_BACKSTOP_QUEUE_SIZE removed — Q4W is now per-user with a global counter

/// Bad debt auction lot multiplier (120% = 1.2x safety margin)
/// 7 decimals: 12_000_000 = 1.2
#[allow(dead_code)]
pub const BAD_DEBT_LOT_MULTIPLIER: i128 = 12_000_000;

// ============================================================================
// ASSET TYPE
// ============================================================================

/// Determines which oracle to use for price queries.
/// - Crypto: uses the Reflector oracle (USDC, XLM, etc.)
/// - Rwa: uses the RWA oracle (USDY, CETES, etc.)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetType {
    Crypto,
    Rwa,
}

// ============================================================================
// POOL STATE
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolState {
    Active, // All operations enabled
    OnIce,  // Only borrowing disabled
    Frozen, // Both borrowing and depositing disabled
}

// ============================================================================
// INTEREST RATE PARAMETERS
// ============================================================================

/// Interest rate parameters for a reserve
/// All values in 7 decimals (SCALAR_7)
///
/// Example configuration for USDC:
/// ```
/// InterestRateParams {
///     target_util: 7_500_000,    // 75%
///     max_util: 9_500_000,       // 95%
///     r_base: 100_000,           // 1% base rate
///     r_one: 500_000,            // 5% slope to target
///     r_two: 5_000_000,          // 50% slope to max
///     r_three: 15_000_000,       // 150% slope above max
///     reactivity: 200,           // 0.00002 reactivity
/// }
/// ```
#[contracttype]
#[derive(Clone, Debug)]
pub struct InterestRateParams {
    /// Target utilization rate (7 decimals, e.g., 7_500_000 = 75%)
    pub target_util: u32,

    /// Maximum utilization rate before extreme rates kick in (7 decimals, e.g., 9_500_000 = 95%)
    pub max_util: u32,

    /// Base interest rate R0 (7 decimals, always applied)
    pub r_base: u32,

    /// Interest rate slope R1 (7 decimals, applied up to target_util)
    pub r_one: u32,

    /// Interest rate slope R2 (7 decimals, applied from target_util to max_util)
    pub r_two: u32,

    /// Interest rate slope R3 (7 decimals, applied above max_util)
    pub r_three: u32,

    /// Reactivity constant for rate modifier adjustment (7 decimals)
    pub reactivity: u32,

    /// Liability factor (7 decimals, e.g. 8_000_000 = 80%)
    /// Applied to debt when computing health factor and borrow limits:
    ///   effective_debt = debt_usd * SCALAR_7 / l_factor
    /// Lower l_factor → stricter (debt counts as larger). Default: SCALAR_7 (1.0 = no change).
    pub l_factor: u32,

    /// Maximum underlying tokens the reserve can hold (0 = unlimited)
    pub supply_cap: i128,

    /// Whether this reserve accepts new deposits and borrows
    pub enabled: bool,
}

// ============================================================================
// RESERVE DATA
// ============================================================================

/// Reserve state data for an asset
/// Token rates use 12 decimals (SCALAR_12)
#[contracttype]
#[derive(Clone, Debug)]
pub struct ReserveData {
    /// bToken to underlying conversion rate (12 decimals)
    /// underlying = b_tokens * b_rate / SCALAR_12
    pub b_rate: i128,

    /// dToken to underlying conversion rate (12 decimals)
    /// underlying = d_tokens * d_rate / SCALAR_12
    pub d_rate: i128,

    /// Interest rate modifier (7 decimals)
    /// Adjusts dynamically based on utilization vs target
    /// Range: SCALAR_7 / 10 to SCALAR_7 * 10 (0.1x to 10x)
    pub ir_mod: i128,

    /// Total bToken supply
    pub b_supply: i128,

    /// Total dToken supply
    pub d_supply: i128,

    /// Interest owed to backstop (accumulated)
    pub backstop_credit: i128,

    /// Fees owed to treasury (accumulated: reserve factor + origination fees)
    pub treasury_credit: i128,

    /// Last interest accrual timestamp
    pub last_time: u64,
}

impl ReserveData {
    /// Create new reserve data with initial 1:1 rates
    pub fn new(timestamp: u64) -> Self {
        Self {
            b_rate: SCALAR_12, // 1:1 initial rate
            d_rate: SCALAR_12, // 1:1 initial rate
            ir_mod: SCALAR_7,  // 1.0 initial modifier
            b_supply: 0,
            d_supply: 0,
            backstop_credit: 0,
            treasury_credit: 0,
            last_time: timestamp,
        }
    }
}

// ============================================================================
// CDP (Collateralized Debt Position)
// ============================================================================

#[contracttype]
#[derive(Clone, Debug)]
pub struct CDP {
    /// Collateral (RWA tokens): token address -> amount
    pub collateral: Map<Address, i128>,

    /// Debt asset symbol (only one: USDC, XLM, etc.)
    pub debt_asset: Option<Symbol>,

    /// dTokens of the borrowed asset
    pub d_tokens: i128,

    /// Creation timestamp
    pub created_at: u64,

    /// Last update timestamp
    pub last_update: u64,
}

// ============================================================================
// AUCTION TYPES
// ============================================================================

/// Type of auction
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuctionType {
    /// Liquidate unhealthy user positions
    UserLiquidation = 0,
    /// Auction backstop's bad debt
    BadDebt = 1,
    /// Distribute accrued interest to backstop
    Interest = 2,
}

/// Dutch Auction data structure (unified for all auction types)
#[contracttype]
#[derive(Clone, Debug)]
pub struct AuctionData {
    /// Type of auction
    pub auction_type: AuctionType,

    /// The user associated with this auction
    /// For UserLiquidation: the borrower being liquidated
    /// For BadDebt: the borrower with bad debt
    /// For Interest: the contract itself (protocol)
    pub user: Address,

    /// Assets/tokens being bid (what filler pays)
    /// For UserLiquidation: debt tokens
    /// For BadDebt: underlying debt asset
    /// For Interest: backstop tokens
    pub bid: Map<Address, i128>,

    /// Assets/tokens being auctioned (what filler receives)
    /// For UserLiquidation: collateral tokens
    /// For BadDebt: backstop tokens
    /// For Interest: interest tokens
    pub lot: Map<Address, i128>,

    /// Auction start block
    pub block: u32,
}

// ============================================================================
// BACKSTOP TYPES
// ============================================================================

/// Backstop deposit record with embedded Q4W (Queue for Withdrawal) state.
/// queued_amount == 0 means the depositor is NOT in the withdrawal queue.
/// The global BackstopQueuedTotal counter is kept in sync with the sum of all queued_amounts.
#[contracttype]
#[derive(Clone, Debug)]
pub struct BackstopDeposit {
    /// Total deposited amount
    pub amount: i128,

    /// Deposit timestamp
    pub deposited_at: u64,

    /// Amount currently in Q4W (0 = not queued)
    pub queued_amount: i128,

    /// Queue entry timestamp (Some if queued_amount > 0)
    pub queued_at: Option<u64>,
}

// ============================================================================
// ORACLE TYPES
// ============================================================================

/// Price data from oracle (SEP-40 compatible)
#[contracttype]
#[derive(Clone, Debug)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

// ============================================================================
// ROUNDING HELPERS (12 decimals)
// ============================================================================

#[allow(dead_code)]
pub mod rounding {
    use super::SCALAR_12;
    use crate::common::error::Error;

    /// Convert underlying asset amount to bTokens with rounding down (floor)
    /// Used when depositing: favors the protocol (mints fewer bTokens)
    /// Formula: b_tokens = floor(amount * SCALAR_12 / b_rate)
    pub fn to_b_token_down(amount: i128, b_rate: i128) -> Result<i128, Error> {
        amount
            .checked_mul(SCALAR_12)
            .ok_or(Error::ArithmeticError)?
            .checked_div(b_rate)
            .ok_or(Error::ArithmeticError)
    }

    /// Convert underlying asset amount to bTokens with rounding up (ceil)
    /// Used when withdrawing: favors the protocol (burns more bTokens)
    /// Formula: b_tokens = ceil(amount * SCALAR_12 / b_rate)
    pub fn to_b_token_up(amount: i128, b_rate: i128) -> Result<i128, Error> {
        let numerator = amount
            .checked_mul(SCALAR_12)
            .ok_or(Error::ArithmeticError)?
            .checked_add(b_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_sub(1)
            .ok_or(Error::ArithmeticError)?;
        numerator.checked_div(b_rate).ok_or(Error::ArithmeticError)
    }

    /// Convert bTokens to underlying asset amount with rounding down (floor)
    /// Used when calculating withdrawable amount
    /// Formula: underlying = floor(b_tokens * b_rate / SCALAR_12)
    pub fn to_underlying_from_b_token(b_tokens: i128, b_rate: i128) -> Result<i128, Error> {
        b_tokens
            .checked_mul(b_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_div(SCALAR_12)
            .ok_or(Error::ArithmeticError)
    }

    /// Convert underlying asset amount to dTokens with rounding up (ceil)
    /// Used when borrowing: favors the protocol (mints more dTokens)
    /// Formula: d_tokens = ceil(amount * SCALAR_12 / d_rate)
    pub fn to_d_token_up(amount: i128, d_rate: i128) -> Result<i128, Error> {
        let numerator = amount
            .checked_mul(SCALAR_12)
            .ok_or(Error::ArithmeticError)?
            .checked_add(d_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_sub(1)
            .ok_or(Error::ArithmeticError)?;
        numerator.checked_div(d_rate).ok_or(Error::ArithmeticError)
    }

    /// Convert underlying asset amount to dTokens with rounding down (floor)
    /// Used when repaying: favors the protocol (burns fewer dTokens)
    /// Formula: d_tokens = floor(amount * SCALAR_12 / d_rate)
    pub fn to_d_token_down(amount: i128, d_rate: i128) -> Result<i128, Error> {
        amount
            .checked_mul(SCALAR_12)
            .ok_or(Error::ArithmeticError)?
            .checked_div(d_rate)
            .ok_or(Error::ArithmeticError)
    }

    /// Convert dTokens to underlying debt amount with rounding up (ceil)
    /// Used when calculating total debt owed
    /// Formula: underlying = ceil(d_tokens * d_rate / SCALAR_12)
    pub fn to_underlying_from_d_token(d_tokens: i128, d_rate: i128) -> Result<i128, Error> {
        let numerator = d_tokens
            .checked_mul(d_rate)
            .ok_or(Error::ArithmeticError)?
            .checked_add(SCALAR_12)
            .ok_or(Error::ArithmeticError)?
            .checked_sub(1)
            .ok_or(Error::ArithmeticError)?;
        numerator
            .checked_div(SCALAR_12)
            .ok_or(Error::ArithmeticError)
    }

    /// Multiply two values with 7 decimal precision
    /// Result = (a * b) / SCALAR_7
    pub fn mul_scalar_7(a: i128, b: i128) -> Result<i128, Error> {
        a.checked_mul(b)
            .ok_or(Error::ArithmeticError)?
            .checked_div(super::SCALAR_7)
            .ok_or(Error::ArithmeticError)
    }

    /// Divide two values with 7 decimal precision
    /// Result = (a * SCALAR_7) / b
    pub fn div_scalar_7(a: i128, b: i128) -> Result<i128, Error> {
        a.checked_mul(super::SCALAR_7)
            .ok_or(Error::ArithmeticError)?
            .checked_div(b)
            .ok_or(Error::ArithmeticError)
    }
}

// ============================================================================
// SHARED STORAGE TTL (reserve data, pool balances, auctions)
// ============================================================================

pub const SHARED_TTL: u32 = ONE_DAY_LEDGERS * 45;
pub const SHARED_BUMP: u32 = ONE_DAY_LEDGERS * 46;

// ============================================================================
// COMPOSITE KEY STRUCTS
// ============================================================================

/// Key for per-user per-asset data (bTokens, dTokens)
#[contracttype]
#[derive(Clone)]
pub struct UserAssetKey {
    pub user: Address,
    pub asset: Symbol,
}

// ============================================================================
// STORAGE KEYS
// ============================================================================

/// Typed storage keys for the lending pool.
///
/// Layout:
/// - Instance storage          : fixed-size scalar config (Admin, PoolState, fee rates, oracles)
/// - Persistent SHARED per-entry: per-asset config set by admin (CollateralFactor, TokenContract…)
///   and per-asset state (PoolBalance, ReserveData, InterestRateParams, Auction)
///   and global counters (BackstopTotal, BackstopQueuedTotal)
/// - Persistent USER per-entry : per-user positions (BTokenBalance, DTokenBalance, Cdp, BackstopDeposit)
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // ---- Instance storage (lean config — fixed-size scalars only) ----
    Admin,
    PoolState,
    NekoOracle,
    ReflectorOracle,
    BackstopToken,
    BackstopThreshold,
    BackstopTakeRate,
    Treasury,
    ReserveFactor,
    OriginationFeeRate,
    LiquidationFeeRate,

    // ---- Persistent storage (per-asset config, SHARED_TTL — set by admin) ----
    TokenContract(Symbol),
    AssetType(Symbol),
    CollateralAssetType(Address),
    CollateralSymbol(Address),
    CollateralFactor(Address),

    // ---- Persistent storage (per-asset state, SHARED_TTL) ----
    PoolBalance(Symbol),
    ReserveData(Symbol),
    InterestRateParams(Symbol),

    // ---- Persistent storage (per user, USER_TTL) ----
    BTokenBalance(UserAssetKey),
    DTokenBalance(UserAssetKey),
    Cdp(Address),
    BackstopDeposit(Address),

    // ---- Persistent storage (global mutable, SHARED_TTL) ----
    BackstopTotal,
    BackstopQueuedTotal,

    // ---- Temporary storage (auto-expires) ----
    // Unfilled auctions are garbage-collected automatically by Soroban.
    Auction(u32),
    // Pending admin proposal — expires after PROPOSAL_TTL if not accepted.
    ProposedAdmin,
}
