#![no_std]

#[cfg(any(test, feature = "testutils"))]
extern crate std;

mod contract;
mod error;
mod events;
mod storage;

#[cfg(test)]
mod test;

pub use contract::{NekoFactory, NekoFactoryClient};
pub use error::Error;
