use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // Admin errors
    NotAuthorized = 1,
    NotInitialized = 2,
    AlreadyInitialized = 3,

    // General errors
    NotPositive = 4,
    ArithmeticError = 5,

    // Token errors
    TokenContractNotSet = 10,

    // Backstop errors
    InsufficientBackstopDeposit = 20,
    WithdrawalQueueActive = 21,
    WithdrawalQueueNotExpired = 22,
    BadDebtNotCovered = 23,
    BackstopThresholdNotMet = 24,
}
