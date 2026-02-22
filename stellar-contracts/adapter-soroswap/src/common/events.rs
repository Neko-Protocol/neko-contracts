use soroban_sdk::{contractevent, Address, Env};

#[contractevent]
pub struct DepositedEvent {
    pub adapter:  Address,
    pub token_a:  Address,
    pub amount_a: i128,
    pub lp_minted: i128,
}

#[contractevent]
pub struct WithdrawnEvent {
    pub adapter:      Address,
    pub token_a:      Address,
    pub amount_out:   i128,
}

pub struct Events;

impl Events {
    pub fn deposited(env: &Env, adapter: &Address, token_a: &Address, amount_a: i128, lp_minted: i128) {
        DepositedEvent {
            adapter: adapter.clone(),
            token_a: token_a.clone(),
            amount_a,
            lp_minted,
        }
        .publish(env);
    }

    pub fn withdrawn(env: &Env, adapter: &Address, token_a: &Address, amount_out: i128) {
        WithdrawnEvent {
            adapter:    adapter.clone(),
            token_a:    token_a.clone(),
            amount_out,
        }
        .publish(env);
    }
}
