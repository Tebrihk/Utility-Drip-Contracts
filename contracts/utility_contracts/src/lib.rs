#![no_std]
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, panic_with_error,
    symbol_short, token, Address, Bytes, BytesN, Env, String, Symbol, Vec,
};

// --- Constants ---
const DEFAULT_BUFFER_DAYS: i128 = 3;
const TRUSTED_BUFFER_DAYS: i128 = 1;
const MINIMUM_BALANCE_TO_FLOW: i128 = 500;
const HOUR_IN_SECONDS: u64 = 60 * 60;
const DAY_IN_SECONDS: u64 = 24 * HOUR_IN_SECONDS;
const GRACE_PERIOD_SECONDS: u64 = 86_400;
const DEBT_THRESHOLD: i128 = -10_000_000;
const DAILY_WITHDRAWAL_PERCENT: i128 = 10;
const MAX_USAGE_PER_UPDATE: i128 = 1_000_000_000_000i128;
const MAX_TIMESTAMP_DELAY: u64 = 300;
const PEAK_HOUR_START: u64 = 18 * HOUR_IN_SECONDS;
const PEAK_HOUR_END: u64 = 21 * HOUR_IN_SECONDS;
const PEAK_RATE_MULTIPLIER: i128 = 3;
const RATE_PRECISION: i128 = 2;
const REFERRAL_REWARD_UNITS: i128 = 500;
const XLM_PRECISION: i128 = 10_000_000;
const XLM_MINIMUM_INCREMENT: i128 = 1;
const XLM_GAS_RESERVE: i128 = 5 * XLM_PRECISION;
const MAX_RESELLER_FEE_BPS: i128 = 2000;
const DEBT_SERVICE_DIVERT_BPS: i128 = 500;
const DEFAULT_TAX_RATE_BPS: i128 = 500;
const MAINTENANCE_FUND_PERCENT_BPS: i128 = 1;
const LEDGER_LIFETIME_EXTENSION: u32 = 1_000_000;

// --- External Contract Clients ---
#[contractclient(name = "PriceOracleClient")]
pub trait PriceOracle {
    fn get_price(env: Env) -> PriceData;
}

#[contractclient(name = "SoroSusuClient")]
pub trait SoroSusu {
    fn get_susu_score(env: Env, user: Address) -> u32;
    fn is_trusted_saver(env: Env, user: Address) -> bool;
    fn is_in_default(env: Env, user: Address) -> bool;
    fn record_debt_payment(env: Env, user: Address, amount: i128);
}

#[contractclient(name = "VestingVaultClient")]
pub trait VestingVault {
    fn get_staked_balance(env: Env, user: Address) -> i128;
}

#[contractclient(name = "NFTMinterClient")]
pub trait NFTMinter {
    fn mint_receipt_nft(env: Env, to: Address, meter_id: u64, cycle_index: u32);
    fn mint_impact_sbt(env: Env, to: Address, carbon_saved: i128, reliability_score: u32);
}

// --- Data Structures ---

#[contracttype]
#[derive(Clone)]
pub struct PriceData {
    pub price: i128,
    pub decimals: u32,
    pub last_updated: u64,
}

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
    pub collateral_limit: i128,
    pub last_update: u64,
    pub is_active: bool,
    pub token: Address,
    pub usage_data: UsageData,
    pub max_flow_rate_per_hour: i128,
    pub last_claim_time: u64,
    pub claimed_this_hour: i128,
    pub heartbeat: u64,
    pub device_public_key: BytesN<32>,
    pub is_paired: bool,
    pub grace_period_start: u64,
    pub is_paused: bool,
    pub tier_threshold: i128,
    pub tier_rate: i128,
    pub is_disputed: bool,
    pub challenge_timestamp: u64,
    pub credit_drip_rate: i128,
    pub is_closed: bool,
    pub priority_index: u32,
    pub off_peak_reward_rate_bps: i128,
    pub milestone_deadline: u64,
    pub milestone_confirmed: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct ClaimSettlement {
    pub gross_claimed: i128,
    pub provider_payout: i128,
    pub tax_amount: i128,
    pub protocol_fee: i128,
    pub reseller_payout: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct ResellerConfig {
    pub reseller: Address,
    pub fee_bps: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct ImpactMetrics {
    pub total_kilowatts_funded: i128,
    pub total_liters_streamed: i128,
    pub active_meters: u32,
}

#[contracttype]
pub enum DataKey {
    Meter(u64),
    Count,
    Oracle,
    SoroSusuContract,
    VestingVault,
    NFTMinter,
    MaintenanceWallet,
    ProtocolFeeBps,
    TaxRateBps,
    GovernmentVault,
    MaintenanceFund(u64),
    ResellerConfig(u64),
    ResellerEarnings(Address, Address),
    ImpactSBTMinted(u64),
    ProviderWindow(Address),
    CycleIndex(u64),
    DebtServiceRecord(u64),
}

#[contract]
pub struct UtilityContract;

#[contractimpl]
impl UtilityContract {
    
    pub fn initialize(env: Env, admin: Address, oracle: Address, token: Address) {
        // Admin init logic
    }

    pub fn claim(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        if meter.is_disputed { panic_with_error!(&env, ContractError::InDispute); }

        let old_meter_value = provider_meter_value(&meter);
        let now = env.ledger().timestamp();
        let mut window = get_provider_window_or_default(&env, &meter.provider, now);
        
        let settlement = settle_claim_for_meter(&env, meter_id, &mut meter, now, &mut window);
        let client = token::Client::new(&env, &meter.token);

        // 1. Pay Government Tax
        if settlement.tax_amount > 0 {
            if let Some(gov_vault) = env.storage().instance().get::<_, Address>(&DataKey::GovernmentVault) {
                client.transfer(&env.current_contract_address(), &gov_vault, &settlement.tax_amount);
            }
        }

        // 2. Pay Protocol Maintenance Fee
        if settlement.protocol_fee > 0 {
            if let Some(wallet) = env.storage().instance().get::<_, Address>(&DataKey::MaintenanceWallet) {
                client.transfer(&env.current_contract_address(), &wallet, &settlement.protocol_fee);
            }
        }

        // 3. Pay Reseller (Three-Way Split)
        if settlement.reseller_payout > 0 {
            if let Some(config) = get_reseller_config_impl(&env, meter_id) {
                // We add to earnings record for pull-based reseller withdrawal
                accumulate_reseller_earnings(&env, meter_id, &meter.token, settlement.reseller_payout);
            }
        }

        // 4. Pay Provider
        if settlement.provider_payout > 0 {
            client.transfer(&env.current_contract_address(), &meter.provider, &settlement.provider_payout);
        }

        // Update State
        env.storage().instance().set(&DataKey::ProviderWindow(meter.provider.clone()), &window);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, provider_meter_value(&meter));
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn assign_reseller(env: Env, meter_id: u64, reseller: Address, fee_bps: i128) {
        let meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();
        if fee_bps > MAX_RESELLER_FEE_BPS { panic_with_error!(&env, ContractError::InvalidResellerFee); }

        let config = ResellerConfig { reseller: reseller.clone(), fee_bps };
        env.storage().instance().set(&DataKey::ResellerConfig(meter_id), &config);
        env.events().publish((symbol_short!("RslrSet"), meter_id), (reseller, fee_bps));
    }

    pub fn claim_impact_sbt(env: Env, meter_id: u64) {
        let meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        if env.storage().instance().get(&DataKey::ImpactSBTMinted(meter_id)).unwrap_or(false) {
            panic_with_error!(&env, ContractError::SBTAlreadyMinted);
        }

        const SBT_THRESHOLD: i128 = 18_250_000;
        if meter.usage_data.renewable_watt_hours < SBT_THRESHOLD {
            panic_with_error!(&env, ContractError::ImpactNotSignificantEnough);
        }

        let carbon_saved = meter.usage_data.renewable_watt_hours.saturating_mul(4) / 10;
        let susu_addr = env.storage().instance().get::<_, Address>(&DataKey::SoroSusuContract).expect("No Susu");
        let susu_client = SoroSusuClient::new(&env, &susu_addr);
        let score = susu_client.get_susu_score(meter.user.clone());

        if let Some(minter_addr) = env.storage().instance().get::<_, Address>(&DataKey::NFTMinter) {
            let minter = NFTMinterClient::new(&env, &minter_addr);
            minter.mint_impact_sbt(&meter.user, &carbon_saved, &score);
            env.storage().instance().set(&DataKey::ImpactSBTMinted(meter_id), &true);
        }
    }

    pub fn get_public_utility_health_index(env: Env) -> ImpactMetrics {
        let count: u64 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        let mut total_wh: i128 = 0;
        let mut total_val: i128 = 0;
        let mut active: u32 = 0;

        for i in 1..=count {
            if let Some(meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(i)) {
                total_wh += meter.usage_data.total_watt_hours;
                total_val += meter.usage_data.monthly_volume;
                if meter.is_active && !meter.is_paused { active += 1; }
            }
        }
        ImpactMetrics { total_kilowatts_funded: total_wh / 1000, total_liters_streamed: total_val, active_meters: active }
    }
}

// --- Internal Settlement Logic ---

fn settle_claim_for_meter(
    env: &Env,
    meter_id: u64,
    meter: &mut Meter,
    now: u64,
    provider_window: &mut ProviderWithdrawalWindow,
) -> ClaimSettlement {
    let elapsed = now.saturating_sub(meter.last_update);
    let mut amount = (elapsed as i128).saturating_mul(meter.rate_per_unit);
    
    // Issue #106: Milestone Penalty (Halve rate if deadline missed)
    if meter.milestone_deadline > 0 && now > meter.milestone_deadline && !meter.milestone_confirmed {
        amount /= 2;
    }

    let claimable = if amount > meter.balance && meter.balance - amount >= DEBT_THRESHOLD {
        amount
    } else if amount > meter.balance {
        meter.balance - DEBT_THRESHOLD
    } else {
        amount
    };

    if claimable <= 0 {
        return ClaimSettlement { gross_claimed: 0, provider_payout: 0, tax_amount: 0, protocol_fee: 0, reseller_payout: 0 };
    }

    // 1. Tax Calculation
    let tax_rate = env.storage().instance().get(&DataKey::TaxRateBps).unwrap_or(DEFAULT_TAX_RATE_BPS);
    let tax_amt = (claimable * tax_rate) / 10000;
    let after_tax = claimable - tax_amt;

    // 2. Protocol Fee
    let protocol_bps: i128 = env.storage().instance().get(&DataKey::ProtocolFeeBps).unwrap_or(0);
    let protocol_fee = (after_tax * protocol_bps) / 10000;
    let after_protocol = after_tax - protocol_fee;

    // 3. Reseller Split
    let reseller_payout = get_reseller_cut(env, meter_id, after_protocol);
    let provider_payout = after_protocol - reseller_payout;

    meter.balance -= claimable;
    meter.last_update = now;

    ClaimSettlement {
        gross_claimed: claimable,
        provider_payout,
        tax_amount: tax_amt,
        protocol_fee,
        reseller_payout,
    }
}

// --- Helpers ---

fn get_meter_or_panic(env: &Env, id: u64) -> Meter {
    env.storage().instance().get(&DataKey::Meter(id)).expect("Meter Not Found")
}

fn provider_meter_value(meter: &Meter) -> i128 {
    meter.balance.max(DEBT_THRESHOLD)
}

fn get_provider_window_or_default(env: &Env, provider: &Address, now: u64) -> ProviderWithdrawalWindow {
    env.storage().instance().get(&DataKey::ProviderWindow(provider.clone()))
        .unwrap_or(ProviderWithdrawalWindow { daily_withdrawn: 0, last_reset: now })
}

fn get_reseller_config_impl(env: &Env, meter_id: u64) -> Option<ResellerConfig> {
    env.storage().instance().get(&DataKey::ResellerConfig(meter_id))
}

fn get_reseller_cut(env: &Env, meter_id: u64, amount: i128) -> i128 {
    if let Some(config) = get_reseller_config_impl(env, meter_id) {
        (amount * config.fee_bps) / 10000
    } else {
        0
    }
}

fn accumulate_reseller_earnings(env: &Env, meter_id: u64, token: &Address, amount: i128) {
    if let Some(config) = get_reseller_config_impl(env, meter_id) {
        let key = DataKey::ResellerEarnings(config.reseller, token.clone());
        let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
        env.storage().instance().set(&key, &(current + amount));
    }
}

fn update_provider_total_pool(env: &Env, provider: &Address, old: i128, new: i128) {
    // Pool update logic
}

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    InDispute = 22,
    SBTAlreadyMinted = 100,
    ImpactNotSignificantEnough = 101,
    NotImplemented = 72,
    InvalidResellerFee = 74,
}