use soroban_sdk::{contractevent, Address, Env};

#[contractevent]
pub struct DepositedEvent {
    pub adapter:   Address,
    pub token:     Address,
    pub amount_in: i128,
    pub lp_value:  i128,
}

#[contractevent]
pub struct WithdrawnEvent {
    pub adapter:    Address,
    pub token:      Address,
    pub amount_out: i128,
}

#[contractevent]
pub struct HarvestedEvent {
    pub adapter:        Address,
    pub aqua_token:     Address,
    pub aqua_harvested: i128,
}

pub struct Events;

impl Events {
    pub fn deposited(env: &Env, adapter: &Address, token: &Address, amount_in: i128, lp_value: i128) {
        DepositedEvent {
            adapter:   adapter.clone(),
            token:     token.clone(),
            amount_in,
            lp_value,
        }
        .publish(env);
    }

    pub fn withdrawn(env: &Env, adapter: &Address, token: &Address, amount_out: i128) {
        WithdrawnEvent {
            adapter:    adapter.clone(),
            token:      token.clone(),
            amount_out,
        }
        .publish(env);
    }

    pub fn harvested(env: &Env, adapter: &Address, aqua_token: &Address, aqua_harvested: i128) {
        HarvestedEvent {
            adapter:        adapter.clone(),
            aqua_token:     aqua_token.clone(),
            aqua_harvested,
        }
        .publish(env);
    }
}
