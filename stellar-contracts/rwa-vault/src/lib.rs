#![no_std]

pub mod adapters;
mod admin;
mod common;
mod contract;
mod strategies;
mod vault;

#[cfg(test)]
mod test;

pub use common::error::Error;
pub use contract::{VaultContract, VaultContractClient};
