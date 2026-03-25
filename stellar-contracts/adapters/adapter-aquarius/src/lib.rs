#![no_std]

mod admin;
mod common;
mod contract;
pub mod aquarius_pool;

#[cfg(test)]
mod test;

pub use common::error::Error;
pub use contract::{AquariusAdapter, AquariusAdapterClient};

/// Aquarius liquidity pool interface (generated from WASM at compile time).
pub mod aquarius_pool_contract {
    soroban_sdk::contractimport!(
        file = "../../wasms/external_wasms/aquarius/soroban_liquidity_pool_contract.wasm"
    );
    pub type PoolClient<'a> = Client<'a>;
}
