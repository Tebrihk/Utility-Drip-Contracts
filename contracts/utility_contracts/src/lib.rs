#![no_std]
use soroban_sdk::{contract, contracttype, contractimpl, Address, Env, token};

#[contracttype]
#[derive(Clone)]
pub struct UsageData {
    pub total_watt_hours: i128,
    pub current_cycle_watt_hours: i128,
    pub peak_usage_watt_hours: i128,
    pub last_reading_timestamp: u64,
    pub precision_factor: i128, // For decimal precision (e.g., 1000 for 3 decimal places)
}

#[contracttype]
#[derive(Clone)]
pub struct Meter {
    pub user: Address,
    pub provider: Address,
    pub rate_per_second: i128,
    pub balance: i128,
    pub last_update: u64,
    pub is_active: bool,
    pub token: Address,
    pub usage_data: UsageData,
}

#[contracttype]
pub enum DataKey {
    Meter(u64),
    Count,
}

#[contract]
pub struct UtilityContract;

#[contractimpl]
impl UtilityContract {
    pub fn register_meter(
        env: Env,
        user: Address,
        provider: Address,
        rate: i128,
        token: Address,
    ) -> u64 {
        user.require_auth();
        let mut count: u64 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        count += 1;

        let usage_data = UsageData {
            total_watt_hours: 0,
            current_cycle_watt_hours: 0,
            peak_usage_watt_hours: 0,
            last_reading_timestamp: env.ledger().timestamp(),
            precision_factor: 1000, // 3 decimal places for precision
        };

        let meter = Meter {
            user,
            provider,
            rate_per_second: rate,
            balance: 0,
            last_update: env.ledger().timestamp(),
            is_active: false,
            token,
            usage_data,
        };

        env.storage().instance().set(&DataKey::Meter(count), &meter);
        env.storage().instance().set(&DataKey::Count, &count);
        count
    }

    pub fn top_up(env: Env, meter_id: u64, amount: i128) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.user.require_auth();

        let client = token::Client::new(&env, &meter.token);
        client.transfer(&meter.user, &env.current_contract_address(), &amount);

        meter.balance += amount;
        meter.is_active = true;
        meter.last_update = env.ledger().timestamp();
        
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn claim(env: Env, meter_id: u64) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.provider.require_auth();

        let now = env.ledger().timestamp();
        let elapsed = now.checked_sub(meter.last_update).unwrap_or(0);
        let amount = (elapsed as i128) * meter.rate_per_second;
        
        // Ensure we don't overdraw the balance
        let claimable = if amount > meter.balance {
            meter.balance
        } else {
            amount
        };

        if claimable > 0 {
            let client = token::Client::new(&env, &meter.token);
            client.transfer(&env.current_contract_address(), &meter.provider, &claimable);
            meter.balance -= claimable;
        }

        meter.last_update = now;
        if meter.balance <= 0 {
            meter.is_active = false;
        }

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn update_usage(env: Env, meter_id: u64, watt_hours_consumed: i128) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.user.require_auth();

        // Update usage data with high precision
        let precise_consumption = watt_hours_consumed * meter.usage_data.precision_factor;
        meter.usage_data.total_watt_hours += precise_consumption;
        meter.usage_data.current_cycle_watt_hours += precise_consumption;
        
        // Update peak usage if current is higher
        if meter.usage_data.current_cycle_watt_hours > meter.usage_data.peak_usage_watt_hours {
            meter.usage_data.peak_usage_watt_hours = meter.usage_data.current_cycle_watt_hours;
        }
        
        meter.usage_data.last_reading_timestamp = env.ledger().timestamp();
        
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn reset_cycle_usage(env: Env, meter_id: u64) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.provider.require_auth();
        
        meter.usage_data.current_cycle_watt_hours = 0;
        meter.usage_data.last_reading_timestamp = env.ledger().timestamp();
        
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn get_usage_data(env: Env, meter_id: u64) -> Option<UsageData> {
        if let Some(meter) = env.storage().instance().get::<DataKey, Meter>(&DataKey::Meter(meter_id)) {
            Some(meter.usage_data)
        } else {
            None
        }
    }

    pub fn get_meter(env: Env, meter_id: u64) -> Option<Meter> {
        env.storage().instance().get(&DataKey::Meter(meter_id))
    }

    pub fn get_watt_hours_display(precise_watt_hours: i128, precision_factor: i128) -> i128 {
        precise_watt_hours / precision_factor
    }
}

mod test;
