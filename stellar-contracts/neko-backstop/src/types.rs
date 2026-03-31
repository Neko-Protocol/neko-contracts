use soroban_sdk::contracttype;

// ============================================================================
// SCALAR CONSTANTS
// ============================================================================

// ============================================================================
// TTL CONSTANTS
// ============================================================================

pub const ONE_DAY_LEDGERS: u32 = 17280;

pub const INSTANCE_TTL: u32 = ONE_DAY_LEDGERS * 30;
pub const INSTANCE_BUMP: u32 = ONE_DAY_LEDGERS * 31;

pub const USER_TTL: u32 = ONE_DAY_LEDGERS * 100;
pub const USER_BUMP: u32 = ONE_DAY_LEDGERS * 120;

// ============================================================================
// BACKSTOP CONSTANTS
// ============================================================================

pub const BACKSTOP_WITHDRAWAL_QUEUE_DAYS: u64 = 17;
pub const BACKSTOP_WITHDRAWAL_QUEUE_SECONDS: u64 = BACKSTOP_WITHDRAWAL_QUEUE_DAYS * 24 * 60 * 60;

// ============================================================================
// POOL STATE
// ============================================================================

/// Mirror of the pool's PoolState — must keep variant order identical so XDR-encoded
/// u32 ordinals match when backstop calls pool.update_pool_state_from_backstop.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolState {
    Active,
    OnIce,
    Frozen,
}

// ============================================================================
// BACKSTOP DEPOSIT
// ============================================================================

/// Per-depositor record with embedded Q4W (Queue for Withdrawal) state.
/// queued_amount == 0 means the depositor is NOT in the withdrawal queue.
#[contracttype]
#[derive(Clone, Debug)]
pub struct BackstopDeposit {
    pub amount: i128,
    pub deposited_at: u64,
    /// Amount currently in Q4W (0 = not queued)
    pub queued_amount: i128,
    /// Queue entry timestamp (Some if queued_amount > 0)
    pub queued_at: Option<u64>,
}

// ============================================================================
// STORAGE KEYS
// ============================================================================

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // ---- Instance storage (fixed-size scalars) ----
    Admin,
    PoolContract,
    BackstopToken,
    BackstopThreshold,

    // ---- Persistent storage (USER_TTL) ----
    BackstopDeposit(soroban_sdk::Address),

    // ---- Persistent storage (global counters, USER_TTL) ----
    BackstopTotal,
    BackstopQueuedTotal,
}
