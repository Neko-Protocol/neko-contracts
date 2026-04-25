#![no_std]

#[cfg(any(test, feature = "testutils"))]
extern crate std;

/// Pool constructor args and client types generated from **`neko_pool.wasm`** via `contractimport!`.
/// Build `neko-pool` for `wasm32v1-none` before this crate so the WASM exists (e.g. `deploy-factory.sh` order).
mod pool_wasm {
    soroban_sdk::contractimport!(file = "../target/wasm32v1-none/release/neko_pool.wasm");
}

mod contract;
mod error;
mod events;
mod storage;

#[cfg(test)]
mod test;

pub use contract::{NekoFactory, NekoFactoryClient};
pub use error::Error;
