use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotVault = 3,
    NotAdmin = 4,
    ZeroAmount = 5,
    ArithmeticError = 6,
    InsufficientBalance = 7,
    InvalidReserve = 8,
}
