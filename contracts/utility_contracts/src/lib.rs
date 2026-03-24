#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BillingType {
    PrePaid,
    PostPaid,
}

#[contracttype]
#[derive(Clone)]
pub struct UsageData {
    pub total_watt_hours: i128,
    pub current_cycle_watt_hours: i128,
    pub peak_usage_watt_hours: i128,
    pub last_reading_timestamp: u64,
    pub precision_factor: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct Meter {
    pub user: Address,
    pub provider: Address,
    pub billing_type: BillingType,
    pub rate_per_second: i128,
    pub rate_per_unit: i128,
    pub balance: i128,
    pub debt: i128,
    pub collateral_limit: i128,
    pub last_update: u64,
    pub is_active: bool,
    pub token: Address,
    pub usage_data: UsageData,
    pub max_flow_rate_per_hour: i128,
    pub last_claim_time: u64,
    pub claimed_this_hour: i128,
    pub heartbeat: u64,
}

#[contracttype]
pub enum DataKey {
    Meter(u64),
    Count,
    Oracle,
}

#[contract]
pub struct UtilityContract;

fn get_meter(env: &Env, meter_id: u64) -> Meter {
    env.storage()
        .instance()
        .get(&DataKey::Meter(meter_id))
        .ok_or("Meter not found")
        .unwrap()
}

fn remaining_postpaid_collateral(meter: &Meter) -> i128 {
    meter.collateral_limit.saturating_sub(meter.debt)
}

fn refresh_activity(meter: &mut Meter) {
    meter.is_active = match meter.billing_type {
        BillingType::PrePaid => meter.balance > 0,
        BillingType::PostPaid => remaining_postpaid_collateral(meter) > 0,
    };
}

fn reset_claim_window_if_needed(meter: &mut Meter, now: u64) {
    if now.saturating_sub(meter.last_claim_time) >= 3600 {
        meter.claimed_this_hour = 0;
        meter.last_claim_time = now;
    }
}

fn remaining_claim_capacity(meter: &Meter) -> i128 {
    meter
        .max_flow_rate_per_hour
        .saturating_sub(meter.claimed_this_hour)
        .max(0)
}

fn apply_provider_claim(env: &Env, meter: &mut Meter, amount: i128) {
    if amount <= 0 {
        return;
    }

    let client = token::Client::new(env, &meter.token);
    client.transfer(&env.current_contract_address(), &meter.provider, &amount);

    match meter.billing_type {
        BillingType::PrePaid => {
            meter.balance -= amount;
        }
        BillingType::PostPaid => {
            meter.debt += amount;
        }
    }

    meter.claimed_this_hour += amount;
}

#[contractimpl]
impl UtilityContract {
    pub fn set_oracle(env: Env, oracle: Address) {
        env.storage().instance().set(&DataKey::Oracle, &oracle);
    }

    pub fn register_meter(
        env: Env,
        user: Address,
        provider: Address,
        rate: i128,
        token: Address,
    ) -> u64 {
        Self::register_meter_with_mode(env, user, provider, rate, token, BillingType::PrePaid)
    }

    pub fn register_meter_with_mode(
        env: Env,
        user: Address,
        provider: Address,
        rate: i128,
        token: Address,
        billing_type: BillingType,
    ) -> u64 {
        user.require_auth();
        let mut count: u64 = env.storage().instance().get::<DataKey, u64>(&DataKey::Count).unwrap_or(0);
        count += 1;

        let now = env.ledger().timestamp();
        let usage_data = UsageData {
            total_watt_hours: 0,
            current_cycle_watt_hours: 0,
            peak_usage_watt_hours: 0,
            last_reading_timestamp: now,
            precision_factor: 1000,
        };

        let meter = Meter {
            user,
            provider,
            billing_type,
            rate_per_second: rate,
            rate_per_unit: rate, // Preserving rate_per_unit if needed, though rate_per_second is used for claims
            balance: 0,
            debt: 0,
            collateral_limit: 0,
            last_update: now,
            is_active: false,
            token,
            usage_data,
            max_flow_rate_per_hour: rate.saturating_mul(3600),
            last_claim_time: now,
            claimed_this_hour: 0,
            heartbeat: now,
        };

        env.storage().instance().set(&DataKey::Meter(count), &meter);
        env.storage().instance().set(&DataKey::Count, &count);
        count
    }

    pub fn top_up(env: Env, meter_id: u64, amount: i128) {
        let mut meter = get_meter(&env, meter_id);
        meter.user.require_auth();
        let was_active = meter.is_active;

        let client = token::Client::new(&env, &meter.token);
        client.transfer(&meter.user, &env.current_contract_address(), &amount);

        match meter.billing_type {
            BillingType::PrePaid => {
                meter.balance += amount;
            }
            BillingType::PostPaid => {
                let settlement = amount.min(meter.debt);
                meter.debt -= settlement;
                meter.collateral_limit += amount.saturating_sub(settlement);
            }
        }

        refresh_activity(&mut meter);
        if !was_active && meter.is_active {
            meter.last_update = env.ledger().timestamp();
            env.events().publish((Symbol::new(&env, "Active"), meter_id), env.ledger().timestamp());
        }

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn claim(env: Env, meter_id: u64) {
        let mut meter = get_meter(&env, meter_id);
        meter.provider.require_auth();

        let now = env.ledger().timestamp();
        if !meter.is_active {
            meter.last_update = now;
            env.storage()
                .instance()
                .set(&DataKey::Meter(meter_id), &meter);
            return;
        }

        reset_claim_window_if_needed(&mut meter, now);

        let elapsed = now.checked_sub(meter.last_update).unwrap_or(0);
        let requested = (elapsed as i128).saturating_mul(meter.rate_per_second);
        let capped = requested.min(remaining_claim_capacity(&meter));

        let claimable = match meter.billing_type {
            BillingType::PrePaid => capped.min(meter.balance),
            BillingType::PostPaid => capped.min(remaining_postpaid_collateral(&meter)),
        };

        apply_provider_claim(&env, &mut meter, claimable);

        meter.last_update = now;
        refresh_activity(&mut meter);

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn deduct_units(env: Env, meter_id: u64, units_consumed: i128) {
        let oracle: Address = env
            .storage()
            .instance()
            .get(&DataKey::Oracle)
            .expect("Oracle address not set");
        oracle.require_auth();

        let mut meter = get_meter(&env, meter_id);
        let now = env.ledger().timestamp();
        reset_claim_window_if_needed(&mut meter, now);

        // Peak hour tariff logic from Issue #13
        let current_hour = (now % 86400) / 3600;
        let is_peak = current_hour >= 18 && current_hour < 22; // 6 PM to 10 PM UTC
        let base_cost = units_consumed.saturating_mul(meter.rate_per_unit);
        let mut cost = if is_peak {
            base_cost.saturating_mul(15) / 10
        } else {
            base_cost
        };

        // Enforce max flow rate hourly cap
        let remaining_this_hour = remaining_claim_capacity(&meter);
        if cost > remaining_this_hour {
            cost = remaining_this_hour;
        }

        let was_active = meter.is_active;

        let claimable = match meter.billing_type {
            BillingType::PrePaid => cost.min(meter.balance),
            BillingType::PostPaid => cost.min(remaining_postpaid_collateral(&meter)),
        };

        apply_provider_claim(&env, &mut meter, claimable);

        meter.last_update = now;
        refresh_activity(&mut meter);
        
        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);

        // Emit events
        env.events().publish(
            (Symbol::new(&env, "UsageReported"), meter_id),
            (units_consumed, claimable),
        );

        if was_active && !meter.is_active {
            env.events().publish((Symbol::new(&env, "Inactive"), meter_id), now);
        }
    }

    pub fn set_max_flow_rate(env: Env, meter_id: u64, amount: i128) {
        let mut meter = get_meter(&env, meter_id);
        meter.provider.require_auth();
        meter.max_flow_rate_per_hour = amount.max(0);
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn update_usage(env: Env, meter_id: u64, watt_hours_consumed: i128) {
        let mut meter = get_meter(&env, meter_id);
        meter.user.require_auth();

        let precise_consumption = watt_hours_consumed.saturating_mul(meter.usage_data.precision_factor);
        meter.usage_data.total_watt_hours += precise_consumption;
        meter.usage_data.current_cycle_watt_hours += precise_consumption;

        if meter.usage_data.current_cycle_watt_hours > meter.usage_data.peak_usage_watt_hours {
            meter.usage_data.peak_usage_watt_hours = meter.usage_data.current_cycle_watt_hours;
        }

        meter.usage_data.last_reading_timestamp = env.ledger().timestamp();

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn reset_cycle_usage(env: Env, meter_id: u64) {
        let mut meter = get_meter(&env, meter_id);
        meter.provider.require_auth();

        meter.usage_data.current_cycle_watt_hours = 0;
        meter.usage_data.last_reading_timestamp = env.ledger().timestamp();

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn get_usage_data(env: Env, meter_id: u64) -> Option<UsageData> {
        if let Some(meter) = env.storage().instance().get::<DataKey, Meter>(&DataKey::Meter(meter_id))
        {
            Some(meter.usage_data)
        } else {
            None
        }
    }

    pub fn get_meter(env: Env, meter_id: u64) -> Option<Meter> {
        env.storage().instance().get::<DataKey, Meter>(&DataKey::Meter(meter_id))
    }

    pub fn calculate_expected_depletion(env: Env, meter_id: u64) -> Option<u64> {
        if let Some(meter) = env.storage().instance().get::<DataKey, Meter>(&DataKey::Meter(meter_id))
        {
            if meter.rate_per_unit <= 0 {
                return Some(0);
            }

            let available = match meter.billing_type {
                BillingType::PrePaid => meter.balance,
                BillingType::PostPaid => remaining_postpaid_collateral(&meter),
            };

            if available <= 0 {
                return Some(0);
            }

            let units_until_depletion = available / meter.rate_per_unit;
            let current_time = env.ledger().timestamp();
            Some(current_time + units_until_depletion as u64)
        } else {
            None
        }
    }

    pub fn emergency_shutdown(env: Env, meter_id: u64) {
        let mut meter = get_meter(&env, meter_id);
        meter.provider.require_auth();
        meter.is_active = false;
        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn update_heartbeat(env: Env, meter_id: u64) {
        let mut meter = get_meter(&env, meter_id);
        meter.user.require_auth();
        meter.heartbeat = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn is_meter_offline(env: Env, meter_id: u64) -> bool {
        if let Some(meter) = env.storage().instance().get::<DataKey, Meter>(&DataKey::Meter(meter_id)) {
            let current_time = env.ledger().timestamp();
            let time_since_heartbeat = current_time.checked_sub(meter.heartbeat).unwrap_or(0);
            time_since_heartbeat > 3600
        } else {
            true
        }
    }
}

impl UtilityContract {
    pub fn get_watt_hours_display(precise_watt_hours: i128, precision_factor: i128) -> i128 {
        precise_watt_hours / precision_factor
    }
}

#[cfg(test)]
mod test;
