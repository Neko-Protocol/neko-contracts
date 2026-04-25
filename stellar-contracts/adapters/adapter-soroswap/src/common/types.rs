use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Debug)]
pub struct AdapterStorage {
    pub vault:   Address,  // Authorized neko-vault address
    pub router:  Address,  // Soroswap router contract
    pub pair:    Address,  // Soroswap pair (token_a / token_b)
    pub token_a: Address,  // deposit_token (single-asset entry point, e.g. USDC)
    pub token_b: Address,  // pair token (e.g. XLM)
    pub admin:   Address,
}
