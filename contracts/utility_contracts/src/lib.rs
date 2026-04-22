#![no_std]
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env, BytesN, Vec, Symbol,
};

// --- Constants ---
const DEFAULT_BUFFER_DAYS: i128 = 3;
const TRUSTED_BUFFER_DAYS: i128 = 1;
const MINIMUM_BALANCE_TO_FLOW: i128 = 500;
const HOUR_IN_SECONDS: u64 = 60 * 60;
const DAY_IN_SECONDS: u64 = 24 * HOUR_IN_SECONDS;
const GRACE_PERIOD_SECONDS: u64 = 86_400;
const DEBT_THRESHOLD: i128 = -10_000_000;
const MAX_USAGE_PER_UPDATE: i128 = 1_000_000_000_000i128;
const MAX_TIMESTAMP_DELAY: u64 = 300;
const PEAK_HOUR_START: u64 = 18 * HOUR_IN_SECONDS;
const PEAK_HOUR_END: u64 = 21 * HOUR_IN_SECONDS;
const PEAK_RATE_MULTIPLIER: i128 = 3; 
const RATE_PRECISION: i128 = 2;
const XLM_PRECISION: i128 = 10_000_000;
const DEFAULT_TAX_RATE_BPS: i128 = 500;
const MAINTENANCE_FUND_PERCENT_BPS: i128 = 1;
const LEDGER_LIFETIME_EXTENSION: u32 = 1_000_000;

// --- Data Structures ---

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BillingType { PrePaid, PostPaid }

#[contracttype]
#[derive(Clone)]
pub struct UsageData {
    pub total_watt_hours: i128,
    pub current_cycle_watt_hours: i128,
    pub peak_usage_watt_hours: i128,
    pub last_reading_timestamp: u64,
    pub precision_factor: i128,
    pub renewable_watt_hours: i128,
    pub renewable_percentage: i128,
    pub monthly_volume: i128,
    pub last_volume_reset: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavingGoal {
    pub target_amount: i128,
    pub current_savings: i128,
    pub marketplace: Address,
    pub is_completed: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct Meter {
    pub user: Address,
    pub provider: Address,
    pub billing_type: BillingType,
    pub off_peak_rate: i128,
    pub peak_rate: i128,
    pub rate_per_unit: i128,
    pub balance: i128,
    pub debt: i128,
    pub last_update: u64,
    pub is_active: bool,
    pub token: Address,
    pub usage_data: UsageData,
    pub device_public_key: BytesN<32>,
    pub end_date: u64,
    pub rent_deposit: i128,
    pub priority_index: u32,
    pub green_energy_discount_bps: i128,
    pub is_paused: bool,
    pub is_disputed: bool,
}

#[contracttype]
pub enum DataKey {
    Meter(u64),
    Count,
    Oracle,
    ActiveMetersCount,
    SeasonalFactor,
    Treasury,
    ProviderVolume(Address),
    SavingGoal(u64),
    NativeToken,
    TaxRateBps,
    ProtocolFeeBps,
    MaintenanceFund(u64),
}

// --- Internal Helpers ---

fn get_seasonal_multiplier(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::SeasonalFactor).unwrap_or(100)
}

fn is_peak_hour(timestamp: u64) -> bool {
    let day_seconds = timestamp % DAY_IN_SECONDS;
    day_seconds >= PEAK_HOUR_START && day_seconds <= PEAK_HOUR_END
}

fn get_effective_rate(env: &Env, meter: &Meter, timestamp: u64) -> i128 {
    let base_rate = if is_peak_hour(timestamp) {
        meter.peak_rate
    } else {
        meter.off_peak_rate
    };
    
    let multiplier = get_seasonal_multiplier(env);
    base_rate.saturating_mul(multiplier) / 100
}

fn apply_provider_claim(env: &Env, meter: &mut Meter, meter_id: u64, amount: i128) {
    if amount <= 0 { return; }

    let mut provider_share = amount;
    
    // 1. Savings Goal Redirection (20%)
    if let Some(mut goal) = env.storage().instance().get::<DataKey, SavingGoal>(&DataKey::SavingGoal(meter_id)) {
        if !goal.is_completed {
            let contribution = amount / 5; 
            goal.current_savings = goal.current_savings.saturating_add(contribution);
            provider_share = provider_share.saturating_sub(contribution);

            if goal.current_savings >= goal.target_amount {
                goal.is_completed = true;
                env.events().publish((symbol_short!("AutoBuy"), meter.user.clone()), (goal.marketplace.clone(), goal.target_amount));
            }
            env.storage().instance().set(&DataKey::SavingGoal(meter_id), &goal);
        }
    }

    // 2. Sustainability / Protocol Fee (0.1% if vol > $100k)
    let mut provider_vol = env.storage().instance().get::<DataKey, i128>(&DataKey::ProviderVolume(meter.provider.clone())).unwrap_or(0);
    if provider_vol >= 10_000_000 {
        let fee = provider_share / 1000;
        if fee > 0 {
            if let Some(treasury) = env.storage().instance().get::<DataKey, Address>(&DataKey::Treasury) {
                let client = token::Client::new(env, &meter.token);
                client.transfer(&env.current_contract_address(), &treasury, &fee);
                provider_share = provider_share.saturating_sub(fee);
            }
        }
    }

    // 3. Payout to Provider
    if provider_share > 0 {
        let client = token::Client::new(env, &meter.token);
        client.transfer(&env.current_contract_address(), &meter.provider, &provider_share);
    }
    
    // 4. State Update
    provider_vol = provider_vol.saturating_add(provider_share);
    env.storage().instance().set(&DataKey::ProviderVolume(meter.provider.clone()), &provider_vol);

    match meter.billing_type {
        BillingType::PrePaid => meter.balance = meter.balance.saturating_sub(amount),
        BillingType::PostPaid => meter.debt = meter.debt.saturating_add(amount),
    }
}

#[contract]
pub struct UtilityContract;

#[contractimpl]
impl UtilityContract {
    pub fn register_meter(
        env: Env,
        user: Address,
        provider: Address,
        off_peak_rate: i128,
        token: Address,
        device_public_key: BytesN<32>,
        end_date: u64,
        rent_deposit: i128,
        priority_index: u32,
    ) -> u64 {
        user.require_auth();
        
        let mut count = env.storage().instance().get::<DataKey, u64>(&DataKey::Count).unwrap_or(0);
        count += 1;

        let now = env.ledger().timestamp();
        let peak_rate = off_peak_rate.saturating_mul(PEAK_RATE_MULTIPLIER) / RATE_PRECISION;

        let meter = Meter {
            user,
            provider,
            billing_type: BillingType::PrePaid,
            off_peak_rate,
            peak_rate,
            rate_per_unit: off_peak_rate,
            balance: 0,
            debt: 0,
            last_update: now,
            is_active: true,
            token,
            usage_data: UsageData {
                total_watt_hours: 0,
                current_cycle_watt_hours: 0,
                peak_usage_watt_hours: 0,
                last_reading_timestamp: now,
                precision_factor: 1,
                renewable_watt_hours: 0,
                renewable_percentage: 0,
                monthly_volume: 0,
                last_volume_reset: now,
            },
            device_public_key,
            end_date,
            rent_deposit,
            priority_index,
            green_energy_discount_bps: 0,
            is_paused: false,
            is_disputed: false,
        };

        env.storage().instance().set(&DataKey::Meter(count), &meter);
        env.storage().instance().set(&DataKey::Count, &count);
        count
    }

    pub fn deduct_units_signed(env: Env, signed_data: SignedUsageData) {
        let mut meter = env.storage().instance().get::<DataKey, Meter>(&DataKey::Meter(signed_data.meter_id)).unwrap();
        meter.provider.require_auth();

        // Security Checks
        if meter.is_disputed || meter.is_paused { panic_with_error!(&env, ContractError::InDispute); }

        let now = env.ledger().timestamp();
        let effective_rate = get_effective_rate(&env, &meter, now);

        // Apply Green Discount
        let discounted_rate = if signed_data.is_renewable_energy && meter.green_energy_discount_bps > 0 {
            effective_rate.saturating_mul(10000 - meter.green_energy_discount_bps) / 10000
        } else {
            effective_rate
        };

        let cost = signed_data.units_consumed.saturating_mul(discounted_rate);
        
        // Process Settlement
        apply_provider_claim(&env, &mut meter, signed_data.meter_id, cost);
        
        meter.last_update = now;
        env.storage().instance().set(&DataKey::Meter(signed_data.meter_id), &meter);
        
        env.events().publish((symbol_short!("Usage"), signed_data.meter_id), (signed_data.units_consumed, cost));
    }
}