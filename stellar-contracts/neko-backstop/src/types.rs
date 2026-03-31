use soroban_sdk::{Address, Vec, contracttype};

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

pub const Q4W_LOCK_SECONDS: u64 = 17 * 24 * 60 * 60; // 17 days

/// Maximum number of simultaneous withdrawal queue entries per depositor.
/// Matches Blend v2.
pub const MAX_Q4W_SIZE: u32 = 20;

// ============================================================================
// POOL STATE
// ============================================================================

/// Mirror of the pool's PoolState — variant order must stay identical so the
/// u32 ordinal pushed via pool.update_pool_state_from_backstop stays consistent.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolState {
    Active,
    OnIce,
    Frozen,
}

// ============================================================================
// Q4W — Queue-for-Withdrawal entry
// ============================================================================

/// A single withdrawal queue entry.
/// `exp` is the earliest timestamp at which the depositor may execute the withdrawal.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Q4W {
    /// Token amount queued for withdrawal.
    pub amount: i128,
    /// Expiration timestamp: creation_time + Q4W_LOCK_SECONDS.
    pub exp: u64,
}

// ============================================================================
// USER BALANCE
// ============================================================================

/// Per-depositor balance record.
///
/// `q4w` holds up to MAX_Q4W_SIZE simultaneous withdrawal queue entries.
/// Entries are ordered oldest-first; dequeue_withdrawal() removes the newest
/// (tail), withdraw() consumes from the oldest (head) once expired.
#[contracttype]
#[derive(Clone, Debug)]
pub struct UserBalance {
    /// Tokens actively deposited (not queued).
    pub amount: i128,
    /// Pending withdrawal queue entries (oldest → newest).
    pub q4w: Vec<Q4W>,
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

    // ---- Persistent storage (per depositor, USER_TTL) ----
    UserBalance(Address),

    // ---- Persistent storage (global counters, USER_TTL) ----
    BackstopTotal,
    BackstopQueuedTotal,
}
