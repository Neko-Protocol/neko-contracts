#![no_std]
mod admin;
pub mod blend_pool;
mod common;
mod contract;
#[cfg(test)]
mod test;

pub use common::error::Error;
pub use contract::{BlendAdapter, BlendAdapterClient};

/// Blend pool contract interface — generated from pool.wasm at compile time.
///
/// Build order:
///   1. Copy pool.wasm from Blend releases into stellar-contracts/wasms/external_wasms/blend/pool.wasm
///   2. cargo build --target wasm32v1-none --release -p adapter-blend
///
/// Source: https://github.com/blend-capital/blend-contracts
pub mod blend {
    soroban_sdk::contractimport!(file = "../../wasms/external_wasms/blend/pool.wasm");
    pub type PoolClient<'a> = Client<'a>;
}
