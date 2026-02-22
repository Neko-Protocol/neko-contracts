use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    token::TokenClient,
    vec, Address, Env, IntoVal, Symbol,
};

use crate::common::types::AdapterStorage;
use crate::soroswap_pair;
use crate::soroswap_router;

// No slippage protection for MVP — suitable for tests, configure min_out for prod.
const MIN_OUT: i128 = 0;

/// Deposit `amount` of token_a into the Soroswap pair.
///
/// Flow:
///   1. Swap half of token_a → token_b via the router.
///   2. Add liquidity with (remaining token_a, received token_b).
///   3. LP tokens held by this adapter.
///
/// Pre-condition: `amount` of token_a is already held by the adapter.
/// Returns the adapter's LP token balance after deposit.
pub fn deposit(env: &Env, amount: i128, storage: &AdapterStorage) -> i128 {
    let adapter = env.current_contract_address();
    let deadline = env.ledger().timestamp() + 3600;

    let swap_amount = amount / 2;
    let remaining_a = amount - swap_amount;

    let router = soroswap_router::RouterClient::new(env, &storage.router);
    let pair   = soroswap_pair::PairClient::new(env, &storage.pair);

    // ── Step 1: swap half token_a → token_b ──────────────────────────────
    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.token_a.clone(),
                fn_name:  Symbol::new(env, "transfer"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    storage.pair.clone().into_val(env),
                    swap_amount.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
    ]);

    let path = vec![env, storage.token_a.clone(), storage.token_b.clone()];
    let swap_out = router.swap_exact_tokens_for_tokens(
        &swap_amount,
        &MIN_OUT,
        &path,
        &adapter,
        &deadline,
    );
    let b_received = swap_out.last().unwrap_or(0);

    // ── Step 2: compute exact token_b the router will use ────────────────
    // Soroswap's add_liquidity adjusts amounts to the optimal ratio based on
    // current reserves. We must pre-authorize with the EXACT amount the router
    // will use, not b_received (which may be slightly higher than optimal).
    let (reserve0, reserve1) = pair.get_reserves();
    let token0 = pair.token_0();
    let (reserve_a, reserve_b) = if token0 == storage.token_a {
        (reserve0, reserve1)
    } else {
        (reserve1, reserve0)
    };
    // b_optimal = remaining_a × reserve_b / reserve_a  (router formula)
    let b_optimal = remaining_a
        .checked_mul(reserve_b)
        .unwrap_or(0)
        .checked_div(reserve_a)
        .unwrap_or(0);
    let b_to_add = b_optimal.min(b_received);

    // ── Add liquidity with exact amounts ──────────────────────────────────
    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.token_a.clone(),
                fn_name:  Symbol::new(env, "transfer"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    storage.pair.clone().into_val(env),
                    remaining_a.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.token_b.clone(),
                fn_name:  Symbol::new(env, "transfer"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    storage.pair.clone().into_val(env),
                    b_to_add.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
    ]);

    router.add_liquidity(
        &storage.token_a,
        &storage.token_b,
        &remaining_a,
        &b_to_add,
        &MIN_OUT,
        &MIN_OUT,
        &adapter,
        &deadline,
    );

    pair.balance(&adapter)
}

/// Withdraw tokens from the Soroswap pair and transfer token_a to `to` (the vault).
///
/// Computes LP tokens to burn proportional to `amount / total_balance`.
/// Removes liquidity, swaps token_b back to token_a, transfers result to vault.
/// Returns the actual token_a amount sent to vault.
pub fn withdraw(env: &Env, amount: i128, to: &Address, storage: &AdapterStorage) -> i128 {
    let adapter  = env.current_contract_address();
    let deadline = env.ledger().timestamp() + 3600;

    let router = soroswap_router::RouterClient::new(env, &storage.router);
    let pair   = soroswap_pair::PairClient::new(env, &storage.pair);

    let lp_balance = pair.balance(&adapter);
    if lp_balance == 0 {
        return 0;
    }

    // Proportional LP burn: lp_to_burn = lp_balance * amount / total_balance
    let total_balance = position_value(env, &adapter, storage);
    if total_balance == 0 {
        return 0;
    }

    let lp_to_burn = if amount >= total_balance {
        lp_balance
    } else {
        lp_balance
            .checked_mul(amount)
            .unwrap_or(lp_balance)
            .checked_div(total_balance)
            .unwrap_or(lp_balance)
    };

    if lp_to_burn == 0 {
        return 0;
    }

    // ── Remove liquidity ─────────────────────────────────────────────────
    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.pair.clone(),
                fn_name:  Symbol::new(env, "transfer"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    storage.pair.clone().into_val(env),
                    lp_to_burn.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
    ]);

    let (a_out, b_out) = router.remove_liquidity(
        &storage.token_a,
        &storage.token_b,
        &lp_to_burn,
        &MIN_OUT,
        &MIN_OUT,
        &adapter,
        &deadline,
    );

    // ── Swap token_b → token_a ───────────────────────────────────────────
    let total_a = if b_out > 0 {
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: storage.token_b.clone(),
                    fn_name:  Symbol::new(env, "transfer"),
                    args:     vec![
                        env,
                        adapter.clone().into_val(env),
                        storage.pair.clone().into_val(env),
                        b_out.into_val(env),
                    ],
                },
                sub_invocations: vec![env],
            }),
        ]);

        let path     = vec![env, storage.token_b.clone(), storage.token_a.clone()];
        let swap_out = router.swap_exact_tokens_for_tokens(
            &b_out,
            &MIN_OUT,
            &path,
            &adapter,
            &deadline,
        );
        let swapped_a = swap_out.last().unwrap_or(0);
        a_out + swapped_a
    } else {
        a_out
    };

    // ── Transfer token_a to vault ────────────────────────────────────────
    if total_a > 0 {
        let token = TokenClient::new(env, &storage.token_a);
        token.transfer(&adapter, to, &total_a);
    }

    total_a
}

/// Returns the adapter's LP position value in token_a units.
///
/// value = adapter_share_of_reserve_a + (adapter_share_of_reserve_b × price_b_in_a)
/// price_b_in_a = reserve_a / reserve_b  (AMM spot price)
pub fn position_value(env: &Env, lender: &Address, storage: &AdapterStorage) -> i128 {
    let pair = soroswap_pair::PairClient::new(env, &storage.pair);

    let lp_balance = pair.balance(lender);
    if lp_balance == 0 {
        return 0;
    }

    let total_lp = pair.total_supply();
    if total_lp == 0 {
        return 0;
    }

    let (reserve_a, reserve_b) = pair.get_reserves();
    if reserve_b == 0 {
        return 0;
    }

    // adapter's share of each reserve
    let share_a = reserve_a
        .checked_mul(lp_balance)
        .unwrap_or(0)
        .checked_div(total_lp)
        .unwrap_or(0);
    let share_b = reserve_b
        .checked_mul(lp_balance)
        .unwrap_or(0)
        .checked_div(total_lp)
        .unwrap_or(0);

    // convert share_b to token_a using AMM spot price
    let b_in_a = share_b
        .checked_mul(reserve_a)
        .unwrap_or(0)
        .checked_div(reserve_b)
        .unwrap_or(0);

    share_a + b_in_a
}
