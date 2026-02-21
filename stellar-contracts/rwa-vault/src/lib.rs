#![no_std]

mod admin;
pub mod adapters;
mod common;
mod contract;
mod strategies;
mod vault;

#[cfg(test)]
mod test;

pub use common::error::Error;
pub use contract::{VaultContract, VaultContractClient};
