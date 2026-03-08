use soroban_sdk::{contract, contractimpl, vec, Address, Env, IntoVal, Symbol, Vec};

use crate::storage::Storage;
use crate::types::MintRequest;

#[contract]
pub struct Faucet;

#[contractimpl]
impl Faucet {
    /// Initialize the faucet with an admin address.
    /// The admin must be the same account that controls the rwa-token contracts.
    pub fn initialize(env: Env, admin: Address) {
        assert!(!Storage::is_initialized(&env), "Faucet: already initialized");
        admin.require_auth();
        Storage::set_admin(&env, &admin);
        Storage::set_initialized(&env);
    }

    /// Mint multiple tokens in a single invocation.
    /// The faucet contract must be the admin of each token contract. The caller must
    /// authorize the faucet (e.g. faucet admin signs for the faucet) so that
    /// cross-contract calls to set_authorized and mint succeed.
    pub fn bulk_mint(env: Env, requests: Vec<MintRequest>) {
        env.current_contract_address().require_auth();
        for req in requests.iter() {
            env.invoke_contract::<()>(
                &req.token,
                &Symbol::new(&env, "set_authorized"),
                vec![&env, req.to.into_val(&env), true.into_val(&env)],
            );
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

    /// Transfer admin of each token to a new address.
    /// Only the current admin (faucet) can invoke set_admin on tokens. This function
    /// invokes set_admin on each token so the faucet can hand off admin to another address.
    pub fn transfer_token_admins(env: Env, tokens: Vec<Address>, new_admin: Address) {
        for token in tokens.iter() {
            env.invoke_contract::<()>(
                &token,
                &Symbol::new(&env, "set_admin"),
                vec![&env, new_admin.into_val(&env)],
            );
        }
    }
}
