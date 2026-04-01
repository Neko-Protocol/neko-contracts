use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    token::TokenClient,
    vec, Address, Env, IntoVal, Symbol,
};

use crate::aquarius_pool_contract;
use crate::common::types::AdapterStorage;

const BPS: u128 = 10_000;

/// Apply slippage tolerance to an expected amount.
/// Returns `expected * (BPS - slippage_bps) / BPS`.
fn min_with_slippage(expected: u128, slippage_bps: u32) -> u128 {
    expected
        .checked_mul(BPS - slippage_bps as u128)
        .unwrap_or(0)
        .checked_div(BPS)
        .unwrap_or(0)
}

/// Deposit `amount` of deposit_token into the Aquarius pool.
///
/// Flow:
///   1. Swap half deposit_token → pair_token via pool.swap().
///   2. Add liquidity with (remaining deposit_token, received pair_token).
///   3. LP share tokens are held by this adapter.
///
/// Pre-condition: `amount` of deposit_token is already held by the adapter.
/// Returns the adapter's LP position value in deposit_token units after deposit.
pub fn deposit(env: &Env, amount: i128, storage: &AdapterStorage) -> i128 {
    let adapter = env.current_contract_address();
    let pool    = aquarius_pool_contract::PoolClient::new(env, &storage.pool);

    let swap_amount = amount / 2;
    let remaining   = amount - swap_amount;

    // ── Step 1: estimate swap output, then swap half deposit_token → pair_token ──
    let estimated_pair = pool.estimate_swap(
        &storage.deposit_token_idx,
        &storage.pair_token_idx,
        &(swap_amount as u128),
    );
    let min_pair_out = min_with_slippage(estimated_pair, storage.max_slippage_bps);

    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.deposit_token.clone(),
                fn_name:  Symbol::new(env, "transfer"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    storage.pool.clone().into_val(env),
                    swap_amount.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
    ]);

    let pair_received = pool.swap(
        &adapter,
        &storage.deposit_token_idx,
        &storage.pair_token_idx,
        &(swap_amount as u128),
        &min_pair_out,
    ) as i128;

    // ── Step 2: compute optimal pair_token amount ─────────────────────────
    // Aquarius takes the full desired amounts and refunds any excess.
    // Mirror its formula: pair_optimal = remaining × reserve_pair / reserve_deposit
    let reserves        = pool.get_reserves();
    let reserve_deposit = reserves.get(storage.deposit_token_idx).unwrap_or(0) as i128;
    let reserve_pair    = reserves.get(storage.pair_token_idx).unwrap_or(0) as i128;

    let pair_optimal = if reserve_deposit > 0 {
        remaining
            .checked_mul(reserve_pair)
            .unwrap_or(0)
            .checked_div(reserve_deposit)
            .unwrap_or(0)
    } else {
        pair_received
    };
    let pair_to_add = pair_optimal.min(pair_received);

    // ── Step 3: estimate shares and add liquidity ─────────────────────────
    // desired_amounts must be in pool token order (index 0 first, then index 1)
    let desired = if storage.deposit_token_idx == 0 {
        vec![env, remaining as u128, pair_to_add as u128]
    } else {
        vec![env, pair_to_add as u128, remaining as u128]
    };

    let estimated_shares = pool.estimate_deposit(&desired);
    let min_shares = min_with_slippage(estimated_shares, storage.max_slippage_bps);

    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.deposit_token.clone(),
                fn_name:  Symbol::new(env, "transfer"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    storage.pool.clone().into_val(env),
                    remaining.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.pair_token.clone(),
                fn_name:  Symbol::new(env, "transfer"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    storage.pool.clone().into_val(env),
                    pair_to_add.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
    ]);

    pool.deposit(&adapter, &desired, &min_shares);

    position_value(env, &adapter, storage)
}

/// Withdraw tokens from the Aquarius pool and transfer deposit_token to `to` (the vault).
///
/// Burns LP tokens proportional to `amount / total_balance`.
/// Removes liquidity, swaps received pair_token back to deposit_token, transfers to vault.
/// Returns the actual deposit_token amount sent to vault.
pub fn withdraw(env: &Env, amount: i128, to: &Address, storage: &AdapterStorage) -> i128 {
    let adapter = env.current_contract_address();
    let pool    = aquarius_pool_contract::PoolClient::new(env, &storage.pool);

    let share_token = TokenClient::new(env, &storage.share_token);
    let lp_balance  = share_token.balance(&adapter);
    if lp_balance == 0 {
        return 0;
    }

    let total_value = position_value(env, &adapter, storage);
    if total_value == 0 {
        return 0;
    }

    let lp_to_burn = if amount >= total_value {
        lp_balance
    } else {
        lp_balance
            .checked_mul(amount)
            .unwrap_or(lp_balance)
            .checked_div(total_value)
            .unwrap_or(lp_balance)
    };
    if lp_to_burn == 0 {
        return 0;
    }

    // ── Compute min_amounts for remove_liquidity ───────────────────────────
    // Expected amounts are proportional to the adapter's share of pool reserves.
    let reserves        = pool.get_reserves();
    let total_lp        = pool.get_total_shares();
    let reserve_deposit = reserves.get(storage.deposit_token_idx).unwrap_or(0);
    let reserve_pair    = reserves.get(storage.pair_token_idx).unwrap_or(0);

    let expected_deposit = reserve_deposit
        .checked_mul(lp_to_burn as u128)
        .unwrap_or(0)
        .checked_div(total_lp)
        .unwrap_or(0);
    let expected_pair = reserve_pair
        .checked_mul(lp_to_burn as u128)
        .unwrap_or(0)
        .checked_div(total_lp)
        .unwrap_or(0);

    let min_deposit_out = min_with_slippage(expected_deposit, storage.max_slippage_bps);
    let min_pair_out    = min_with_slippage(expected_pair, storage.max_slippage_bps);

    let min_amounts = if storage.deposit_token_idx == 0 {
        vec![env, min_deposit_out, min_pair_out]
    } else {
        vec![env, min_pair_out, min_deposit_out]
    };

    // ── Remove liquidity ──────────────────────────────────────────────────
    // pool.withdraw → burn_shares → share_token.burn(adapter, amount)
    // burn() calls from.require_auth(); adapter is not the direct invoker here,
    // so we must pre-authorize the burn sub-invocation explicitly.
    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage.share_token.clone(),
                fn_name:  Symbol::new(env, "burn"),
                args:     vec![
                    env,
                    adapter.clone().into_val(env),
                    lp_to_burn.into_val(env),
                ],
            },
            sub_invocations: vec![env],
        }),
    ]);

    let amounts_out  = pool.withdraw(&adapter, &(lp_to_burn as u128), &min_amounts);
    let deposit_out  = amounts_out.get(storage.deposit_token_idx).unwrap_or(0) as i128;
    let pair_out     = amounts_out.get(storage.pair_token_idx).unwrap_or(0) as i128;

    // ── Swap pair_token → deposit_token ───────────────────────────────────
    let total_deposit = if pair_out > 0 {
        let estimated_deposit = pool.estimate_swap(
            &storage.pair_token_idx,
            &storage.deposit_token_idx,
            &(pair_out as u128),
        );
        let min_deposit_from_swap = min_with_slippage(estimated_deposit, storage.max_slippage_bps);

        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: storage.pair_token.clone(),
                    fn_name:  Symbol::new(env, "transfer"),
                    args:     vec![
                        env,
                        adapter.clone().into_val(env),
                        storage.pool.clone().into_val(env),
                        pair_out.into_val(env),
                    ],
                },
                sub_invocations: vec![env],
            }),
        ]);

        let swapped = pool.swap(
            &adapter,
            &storage.pair_token_idx,
            &storage.deposit_token_idx,
            &(pair_out as u128),
            &min_deposit_from_swap,
        ) as i128;

        deposit_out + swapped
    } else {
        deposit_out
    };

    // ── Transfer deposit_token to vault ───────────────────────────────────
    if total_deposit > 0 {
        let deposit_token = TokenClient::new(env, &storage.deposit_token);
        deposit_token.transfer(&adapter, to, &total_deposit);
    }

    total_deposit
}

/// Returns the adapter's LP position value in deposit_token units.
///
/// value = share_of_reserve_deposit + (share_of_reserve_pair × spot_price_pair_in_deposit)
/// spot_price = reserve_deposit / reserve_pair  (constant-product AMM spot price)
pub fn position_value(env: &Env, lender: &Address, storage: &AdapterStorage) -> i128 {
    let share_token = TokenClient::new(env, &storage.share_token);
    let lp_balance  = share_token.balance(lender);
    if lp_balance == 0 {
        return 0;
    }

    let pool     = aquarius_pool_contract::PoolClient::new(env, &storage.pool);
    let total_lp = pool.get_total_shares() as i128;
    if total_lp == 0 {
        return 0;
    }

    let reserves        = pool.get_reserves();
    let reserve_deposit = reserves.get(storage.deposit_token_idx).unwrap_or(0) as i128;
    let reserve_pair    = reserves.get(storage.pair_token_idx).unwrap_or(0) as i128;
    if reserve_pair == 0 {
        return 0;
    }

    // Use u128 intermediates to avoid i128 overflow on large pools.
    let lp_u        = lp_balance as u128;
    let total_u     = total_lp as u128;
    let rd          = reserve_deposit as u128;
    let rp          = reserve_pair as u128;

    let share_deposit = rd.checked_mul(lp_u).unwrap_or(0).checked_div(total_u).unwrap_or(0);
    let share_pair    = rp.checked_mul(lp_u).unwrap_or(0).checked_div(total_u).unwrap_or(0);

    // convert pair share to deposit_token using AMM spot price: share_pair × rd / rp
    let pair_in_deposit = share_pair.checked_mul(rd).unwrap_or(0).checked_div(rp).unwrap_or(0);

    (share_deposit + pair_in_deposit) as i128
}

/// Claim AQUA rewards from the pool and forward them to `to` (the vault).
///
/// Aquarius emits AQUA to LP providers each block. pool.claim() is permissionless —
/// no user.require_auth() is required. The reward is transferred from pool to adapter,
/// then forwarded to the vault.
/// Returns the AQUA amount harvested (0 if no rewards have accrued).
pub fn claim(env: &Env, to: &Address, storage: &AdapterStorage) -> i128 {
    let adapter = env.current_contract_address();
    let pool    = aquarius_pool_contract::PoolClient::new(env, &storage.pool);

    let aqua_harvested = pool.claim(&adapter) as i128;

    if aqua_harvested > 0 {
        let aqua = TokenClient::new(env, &storage.aqua_token);
        aqua.transfer(&adapter, to, &aqua_harvested);
    }

    aqua_harvested
}
