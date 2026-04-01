use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    // Initialization
    AlreadyInitialized = 1,
    NotInitialized = 2,

    // Authorization
    NotAdmin = 3,
    NotManager = 4,

    // Vault state
    VaultPaused = 5,
    VaultNotActive = 6,

    // Amounts
    ZeroAmount = 7,
    InsufficientShares = 8,
    InsufficientLiquidity = 9,

    // Protocols
    ProtocolNotFound = 10,
    ProtocolAlreadyExists = 11,
    MaxProtocolsReached = 12,

    // Math
    ArithmeticError = 13,

    // Config
    InvalidConfig = 14,

    // SEP-41
    InsufficientAllowance = 15,
    InsufficientBalance = 16,
    InvalidLedgerInput = 17,

    // Rebalance
    RebalanceThresholdNotMet = 18,
}
