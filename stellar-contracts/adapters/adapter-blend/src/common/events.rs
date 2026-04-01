use soroban_sdk::{contractevent, Address, Env};

#[contractevent]
pub struct DepositedEvent {
    pub adapter: Address,
    pub asset: Address,
    pub amount: i128,
}

#[contractevent]
pub struct WithdrawnEvent {
    pub adapter: Address,
    pub asset: Address,
    pub amount: i128,
}

#[contractevent]
pub struct HarvestedEvent {
    pub adapter: Address,
    pub blend_token: Address,
    pub blnd_amount: i128,
}

pub struct Events;

impl Events {
    pub fn deposited(env: &Env, adapter: &Address, asset: &Address, amount: i128) {
        DepositedEvent {
            adapter: adapter.clone(),
            asset: asset.clone(),
            amount,
        }
        .publish(env);
    }

    pub fn withdrawn(env: &Env, adapter: &Address, asset: &Address, amount: i128) {
        WithdrawnEvent {
            adapter: adapter.clone(),
            asset: asset.clone(),
            amount,
        }
        .publish(env);
    }

    pub fn harvested(env: &Env, adapter: &Address, blend_token: &Address, blnd_amount: i128) {
        HarvestedEvent {
            adapter: adapter.clone(),
            blend_token: blend_token.clone(),
            blnd_amount,
        }
        .publish(env);
    }
}
