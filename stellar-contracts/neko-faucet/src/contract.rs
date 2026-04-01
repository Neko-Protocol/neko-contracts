use soroban_sdk::{contract, contractimpl, vec, Address, BytesN, Env, IntoVal, Symbol, Vec};

use crate::storage::Storage;
use crate::types::MintRequest;

#[contract]
pub struct Faucet;

#[contractimpl]
impl Faucet {
    /// Initialize the faucet with an admin address.
    /// The admin must be the same account that controls the neko-token contracts.
    pub fn initialize(env: Env, admin: Address) {
        assert!(!Storage::is_initialized(&env), "Faucet: already initialized");
        admin.require_auth();
        Storage::set_admin(&env, &admin);
        Storage::set_initialized(&env);
    }

    /// Mint multiple tokens in a single invocation.
    /// Each distinct recipient must authorize once; repeating the same `to` does not call
    /// `require_auth` again (Soroban rejects duplicate auth for the same address in one frame).
    pub fn bulk_mint(env: Env, requests: Vec<MintRequest>) {
        let mut authorized_recipients = Vec::<Address>::new(&env);
        for req in requests.iter() {
            let mut seen = false;
            for prev in authorized_recipients.iter() {
                if prev == req.to {
                    seen = true;
                    break;
                }
            }
            if !seen {
                req.to.require_auth();
                authorized_recipients.push_back(req.to.clone());
            }
            env.invoke_contract::<()>(
                &req.token,
                &Symbol::new(&env, "mint"),
                vec![&env, req.to.into_val(&env), req.amount.into_val(&env)],
            );
        }
    }

    /// Return the admin address.
    pub fn admin(env: Env) -> Address {
        Storage::get_admin(&env)
    }

    /// Upgrade the contract to new WASM. Admin-only.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        Storage::get_admin(&env).require_auth();
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    /// Transfer admin of each token to a new address.
    /// Only the current admin (faucet) can invoke set_admin on tokens. This function
    /// invokes set_admin on each token so the faucet can hand off admin to another address.
    pub fn transfer_token_admins(env: Env, tokens: Vec<Address>, new_admin: Address) {
        Storage::get_admin(&env).require_auth();
        for token in tokens.iter() {
            env.invoke_contract::<()>(
                &token,
                &Symbol::new(&env, "set_admin"),
                vec![&env, new_admin.into_val(&env)],
            );
        }
    }
}
