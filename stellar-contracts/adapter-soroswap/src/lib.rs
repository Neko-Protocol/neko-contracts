#![no_std]

mod admin;
pub mod soroswap_pool;
mod common;
mod contract;

#[cfg(test)]
mod test;

pub use common::error::Error;
pub use contract::{SoroswapAdapter, SoroswapAdapterClient};

/// Soroswap Router interface (generated from WASM at compile time).
pub mod soroswap_router {
    soroban_sdk::contractimport!(file = "../external_wasms/soroswap/router.wasm");
    pub type RouterClient<'a> = Client<'a>;
}

/// Soroswap Pair interface (generated from WASM at compile time).
pub mod soroswap_pair {
    soroban_sdk::contractimport!(file = "../external_wasms/soroswap/pair.wasm");
    pub type PairClient<'a> = Client<'a>;
}
