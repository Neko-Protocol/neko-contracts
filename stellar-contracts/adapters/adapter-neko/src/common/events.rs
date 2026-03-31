use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent]
pub struct DepositedEvent {
    pub adapter: Address,
    pub asset: Symbol,
    pub amount: i128,
    pub b_tokens: i128,
}

#[contractevent]
pub struct WithdrawnEvent {
    pub adapter: Address,
    pub asset: Symbol,
    pub amount: i128,
}

pub struct Events;

impl Events {
    pub fn deposited(env: &Env, adapter: &Address, asset: &Symbol, amount: i128, b_tokens: i128) {
        DepositedEvent {
            adapter: adapter.clone(),
            asset: asset.clone(),
            amount,
            b_tokens,
        }
        .publish(env);
    }

    pub fn withdrawn(env: &Env, adapter: &Address, asset: &Symbol, amount: i128) {
        WithdrawnEvent {
            adapter: adapter.clone(),
            asset: asset.clone(),
            amount,
        }
        .publish(env);
    }
}
