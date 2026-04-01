use soroban_sdk::{Address, Env, contractevent};

#[contractevent]
pub struct DeployEvent {
    pub pool: Address,
}

pub fn deploy(env: &Env, pool: Address) {
    DeployEvent { pool }.publish(env);
}
