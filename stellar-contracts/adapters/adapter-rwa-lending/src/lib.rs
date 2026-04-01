#![no_std]

mod admin;
mod common;
mod contract;

#[cfg(test)]
mod test;

pub use common::error::Error;
pub use contract::{RwaLendingAdapter, RwaLendingAdapterClient};

/// Import rwa-lending WASM for cross-contract calls.
/// Build rwa-lending first: cargo build --target wasm32v1-none --release -p rwa-lending
pub mod rwa_lending {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/rwa_lending.wasm"
    );
}
