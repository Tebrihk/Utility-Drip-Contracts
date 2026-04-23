use soroban_sdk::{Env, Address, Symbol};

#[derive(Clone)]
pub struct OraclePrice {
    pub rate: i128,        // tokens per fiat unit
    pub timestamp: u64,    // unix seconds
}

pub fn get_oracle_price(env: &Env, oracle: Address) -> OraclePrice {
    let price: OraclePrice = env.storage().get(&format!("oracle:{}", oracle))
        .unwrap_or_else(|| panic!("Oracle not found"));

    let now = env.ledger().timestamp();
    if now - price.timestamp > 300 {
        panic!("Stale oracle data (>5 minutes)");
    }

    price
}

pub fn adjust_stream_rate(env: &Env, user: Address, oracle: Address, fiat_rate_per_kwh: i128) -> i128 {
    let price = get_oracle_price(env, oracle);

    // tokens_per_second = fiat_rate * oracle.rate
    let tokens_per_second = fiat_rate_per_kwh * price.rate;

    // Avoid integer truncation by scaling
    let adjusted_rate = tokens_per_second / 1_000_000; // assume oracle rate scaled

    env.storage().set(&format!("stream_rate:{}", user), &adjusted_rate);

    env.events().publish(
        (Symbol::short("RateAdjusted"),),
        (user, adjusted_rate, price.timestamp),
    );

    adjusted_rate
}

pub fn bill_user(env: &Env, user: Address, oracle: Address, fiat_rate_per_kwh: i128, consumption: i128) {
    let rate = adjust_stream_rate(env, user.clone(), oracle, fiat_rate_per_kwh);

    let debit = rate * consumption;
    let mut balance: i128 = env.storage().get(&format!("balance:{}", user)).unwrap_or(0);

    if balance < debit {
        panic!("Insufficient balance");
    }

    balance -= debit;
    env.storage().set(&format!("balance:{}", user), &balance);

    env.events().publish(
        (Symbol::short("BillingApplied"),),
        (user, debit, balance),
    );
}
