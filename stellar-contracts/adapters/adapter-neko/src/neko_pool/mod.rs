/// Import neko-pool WASM for cross-contract calls.
/// Build neko-pool first: cargo build --target wasm32v1-none --release -p neko-pool
soroban_sdk::contractimport!(
    file = "../../target/wasm32v1-none/release/neko_pool.wasm"
);
