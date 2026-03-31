#![no_std]

mod admin;
mod common;
mod contract;
pub mod neko_pool;

#[cfg(test)]
mod test;

pub use common::error::Error;
pub use contract::{NekoAdapter, NekoAdapterClient};
