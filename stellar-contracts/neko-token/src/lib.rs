#![no_std]

pub mod admin;
pub mod common;
pub mod compliance;
pub mod oracle;
pub mod token;

pub use common::error::Error;

// Import RWA Oracle WASM for reading RWA asset prices
pub mod neko_oracle {
    soroban_sdk::contractimport!(file = "../target/wasm32v1-none/release/neko_oracle.wasm");
}

pub mod contract;

#[cfg(test)]
mod test;
