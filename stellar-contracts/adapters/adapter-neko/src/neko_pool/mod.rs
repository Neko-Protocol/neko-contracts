// Import neko-pool WASM for cross-contract calls.
// Build neko-oracle, neko-token, then neko-pool for wasm32v1-none (artifact: target/wasm32v1-none/release/deps/neko_pool.wasm).
soroban_sdk::contractimport!(
    file = "../../target/wasm32v1-none/release/deps/neko_pool.wasm"
);
