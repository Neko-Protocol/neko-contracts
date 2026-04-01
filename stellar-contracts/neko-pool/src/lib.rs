#![no_std]

mod admin;
mod common;
mod contract;
mod operations;
mod test;

pub use common::error::Error;
pub use common::types::{AssetType, InterestRateParams, PoolInitConfig, PoolState};
pub use contract::{LendingContract, LendingContractClient};

// Import RWA Oracle WASM for reading RWA asset prices
// Both RWA Oracle and Reflector Oracle implement SEP-40 interface,
// so we use the same client with different contract addresses
pub mod neko_oracle {
    soroban_sdk::contractimport!(file = "../target/wasm32v1-none/release/neko_oracle.wasm");
}
