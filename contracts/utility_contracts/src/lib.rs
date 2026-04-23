#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol};

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Bytes, BytesN, Env, Symbol, Vec,
};

// Oracle client interface
use soroban_sdk::contractclient;

#[contractclient(name = "PriceOracleClient")]
pub trait PriceOracle {
    fn xlm_to_usd_cents(env: Env, xlm_amount: i128) -> i128;
    fn usd_cents_to_xlm(env: Env, usd_cents: i128) -> i128;
    fn get_price(env: Env) -> PriceData;
}

#[contracttype]
#[derive(Clone)]
pub struct PriceData {
    pub price: i128,
    pub decimals: u32,
    pub last_updated: u64,
}
#[cfg(test)]
mod debt_fuzz_tests;
#[cfg(test)]
mod fuzz_tests;
#[cfg(test)]
mod dust_sweeper_tests;
#[cfg(test)]
mod pause_resume_tests;
#[cfg(test)]
mod pause_resume_fuzz_tests;
#[cfg(test)]
mod buffer_tests;

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BillingType {
    PrePaid,
    PostPaid,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StreamStatus {
    Active = 0,
    Paused = 1,
    Depleted = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuousFlow {
    // Tightly packed struct for optimal storage
    pub stream_id: u64,           // 8 bytes
    pub flow_rate_per_second: i128, // 16 bytes - micro-stroops per second
    pub accumulated_balance: i128,  // 16 bytes - precise balance tracking
    pub last_flow_timestamp: u64,   // 8 bytes - u64 for epoch safety
    pub created_timestamp: u64,     // 8 bytes - creation time
    pub status: StreamStatus,       // 1 byte (enum)
    pub paused_at: u64,           // 8 bytes - timestamp when stream was paused (0 if active)
    pub provider: Address,         // 32 bytes - provider address for access control
    pub buffer_balance: i128,     // 16 bytes - pre-paid buffer balance (24 hours of flow)
    pub buffer_warning_sent: bool, // 1 byte - whether buffer warning has been sent
    pub payer: Address,           // 32 bytes - payer address for buffer refunds
}
// Minimum balance required to keep the IoT relay open (500 tokens for testing)
const MINIMUM_BALANCE_TO_FLOW: i128 = 500; // 500 tokens minimum for testing

// Buffer requirements - 24 hours of flow rate
const BUFFER_DURATION_SECONDS: u64 = 24 * HOUR_IN_SECONDS; // 24 hours
const BUFFER_WARNING_THRESHOLD: i128 = 3600; // Warning when 1 hour of buffer left

#[contracttype]
#[derive(Clone)]
pub struct UsageReport {
    pub meter_id: u64,
    pub timestamp: u64,
    pub watt_hours_consumed: i128,
    pub units_consumed: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct SignedUsageData {
    pub meter_id: u64,
    pub timestamp: u64,
    pub watt_hours_consumed: i128,
    pub units_consumed: i128,
    pub signature: BytesN<64>,
    pub public_key: BytesN<32>,
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
    pub off_peak_rate: i128, // rate per second during off-peak hours
    pub peak_rate: i128,     // rate per second during peak hours (1.5x off-peak)
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
    pub device_public_key: BytesN<32>,
    pub is_paired: bool,
    pub grace_period_start: u64, // timestamp when balance hit 0 and grace period started
}

#[contracttype]
#[derive(Clone)]
pub struct ProviderWithdrawalWindow {
    pub daily_withdrawn: i128,
    pub last_reset: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct MeterInfo {
    pub user: Address,
    pub provider: Address,
    pub off_peak_rate: i128,
    pub token: Address,
    pub billing_type: BillingType,
    pub device_public_key: BytesN<32>,
}

#[contracttype]
#[derive(Clone)]
pub struct BatchCreatedEvent {
    pub start_id: u64,
    pub end_id: u64,
    pub count: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct StreamUpdatedEvent {
    pub stream_id: u64,
    pub old_flow_rate: i128,
    pub new_flow_rate: i128,
    pub timestamp: u64,
    pub old_status: StreamStatus,
    pub new_status: StreamStatus,
}

#[contracttype]
#[derive(Clone)]
pub struct StreamPausedEvent {
    pub stream_id: u64,
    pub paused_at: u64,
    pub provider: Address,
    pub remaining_balance: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct StreamResumedEvent {
    pub stream_id: u64,
    pub resumed_at: u64,
    pub provider: Address,
    pub flow_rate_per_second: i128,
    pub pause_duration: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct BufferWarningEvent {
    pub stream_id: u64,
    pub warning_timestamp: u64,
    pub buffer_remaining: i128,
    pub provider: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct BufferDepletedEvent {
    pub stream_id: u64,
    pub depleted_timestamp: u64,
    pub buffer_consumed: i128,
    pub provider: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct DustCollectedEvent {
    pub token_address: Address,
    pub total_dust_swept: i128,
    pub streams_swept: u64,
    pub timestamp: u64,
    pub sweeper_address: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct DustAggregation {
    pub total_dust: i128,
    pub stream_count: u64,
    pub last_updated: u64,
}

#[contracttype]
pub enum DataKey {
    Meter(u64),
    ProviderWindow(Address),
    Count,
    Oracle,
    PairingChallenge(u64),
    MaintenanceWallet,
    ProtocolFeeBps,
    SupportedToken(Address),
    ProviderTotalPool(Address),
    ContinuousFlow(u64),
    DustAggregation(Address),
    AdminAddress,
    GasBountyPool,
    BufferVault(u64), // Per-stream buffer vault tracking
}

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    MeterNotFound = 1,
    OracleNotSet = 2,
    WithdrawalLimitExceeded = 3,
    PriceConversionFailed = 4,
    InvalidTokenAmount = 5,
    InvalidUsageValue = 6,
    UsageExceedsLimit = 7,
    InvalidPrecisionFactor = 8,
    InvalidSignature = 9,
    PublicKeyMismatch = 10,
    TimestampTooOld = 11,
    PairingAlreadyComplete = 12,
    ChallengeNotFound = 13,
    InvalidPairingSignature = 14,
    MeterNotPaired = 15,
    UnauthorizedAdmin = 16,
    InsufficientGasBounty = 17,
    NoDustToSweep = 18,
    InsufficientBuffer = 19,
    BufferAlreadyDepleted = 20,
    UnauthorizedBufferAccess = 21,
}

#[contracttype]
#[derive(Clone)]
pub struct PairingChallengeData {
    pub contract: Address,
    pub meter_id: u64,
    pub timestamp: u64,
}

#[contract]
pub struct UtilityContract;

const HOUR_IN_SECONDS: u64 = 60 * 60;
const DAY_IN_SECONDS: u64 = 24 * HOUR_IN_SECONDS;
const GRACE_PERIOD_SECONDS: u64 = 86_400; // 24 hours grace period
const DEBT_THRESHOLD: i128 = -10_000_000; // -10 XLM (in stroops) threshold for negative balance
const DAILY_WITHDRAWAL_PERCENT: i128 = 10;
const MAX_USAGE_PER_UPDATE: i128 = 1_000_000_000_000i128; // 1 billion kWh max per update
const MIN_PRECISION_FACTOR: i128 = 1;
const MAX_TIMESTAMP_DELAY: u64 = 300; // 5 minutes

// Peak hours: 18:00 - 21:00 UTC
const PEAK_HOUR_START: u64 = 18 * HOUR_IN_SECONDS; // 64800 seconds
const PEAK_HOUR_END: u64 = 21 * HOUR_IN_SECONDS; // 75600 seconds
const PEAK_RATE_MULTIPLIER: i128 = 3; // 1.5x => stored as 3 (divide by 2)
const RATE_PRECISION: i128 = 2; // Precision for rate calculations

// XLM precision constants - XLM has 7 decimal places (0.0000001 minimum)
const XLM_PRECISION: i128 = 10_000_000; // 10^7 for 7 decimal places
const XLM_MINIMUM_INCREMENT: i128 = 1; // 1 stroop = 0.0000001 XLM

// Dust detection constants
const DUST_THRESHOLD: i128 = 1; // Less than 1 stroop is considered dust
const GAS_BOUNTY_AMOUNT: i128 = 100_000; // 0.01 XLM bounty for dust sweepers
const MAX_SWEEP_STREAMS_PER_CALL: u64 = 1000; // Prevent gas limit issues

/// Round XLM amount to nearest minimum increment (0.0000001 XLM)
/// This prevents value loss over time due to truncation
fn round_xlm_to_minimum_increment(amount: i128) -> i128 {
    // For positive amounts, round up on .5 or higher
    // For negative amounts, round down on -.5 or lower
    if amount >= 0 {
        ((amount + XLM_MINIMUM_INCREMENT / 2) / XLM_MINIMUM_INCREMENT) * XLM_MINIMUM_INCREMENT
    } else {
        ((amount - XLM_MINIMUM_INCREMENT / 2) / XLM_MINIMUM_INCREMENT) * XLM_MINIMUM_INCREMENT
    }
}

/// Convert USD cents to XLM with proper rounding to minimum increment
fn convert_usd_cents_to_xlm_with_rounding(usd_cents: i128, xlm_price_cents: i128) -> i128 {
    if xlm_price_cents <= 0 {
        return 0;
    }

    // Calculate raw XLM amount with higher precision
    let raw_xlm = usd_cents.saturating_mul(XLM_PRECISION) / xlm_price_cents;

    // Round to nearest minimum increment to prevent value loss
    round_xlm_to_minimum_increment(raw_xlm)
}

/// Convert XLM to USD cents with proper rounding
fn convert_xlm_to_usd_cents_with_rounding(xlm_amount: i128, xlm_price_cents: i128) -> i128 {
    if xlm_price_cents <= 0 {
        return 0;
    }

    // Calculate USD cents, rounding to nearest cent
    let raw_usd = xlm_amount.saturating_mul(xlm_price_cents) / XLM_PRECISION;

    // Round to nearest cent
    if raw_usd >= 0 {
        ((raw_usd + 50) / 100) * 100 // Round up on .5 or higher
    } else {
        ((raw_usd - 50) / 100) * 100 // Round down on -.5 or lower
    }
}

/// Checks if an address represents the native Stellar asset (XLM)
fn is_native_token(_token_address: &Address) -> bool {
    // Simplify for consistency with other SAC-compliant tokens.
    true
}

/// Transfer tokens, handling both native XLM and SAC tokens
fn transfer_tokens(
    env: &Env,
    token_address: &Address,
    from: &Address,
    to: &Address,
    amount: &i128,
) {
    let client = token::Client::new(env, token_address);
    client.transfer(from, to, amount);
}

/// Get token balance, handling both native XLM and SAC tokens
fn get_token_balance(env: &Env, token_address: &Address, account: &Address) -> i128 {
    let client = token::Client::new(env, token_address);
    client.balance(account)
}

/// Check if a balance amount qualifies as dust (less than 1 stroop)
fn is_dust_amount(amount: i128) -> bool {
    amount > 0 && amount < XLM_MINIMUM_INCREMENT
}

/// Get admin address or panic if not set
fn get_admin_or_panic(env: &Env) -> Address {
    match env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::AdminAddress)
    {
        Some(admin) => admin,
        None => panic_with_error!(env, ContractError::UnauthorizedAdmin),
    }
}

/// Check if caller is authorized admin
fn require_admin_auth(env: &Env) {
    let admin = get_admin_or_panic(env);
    admin.require_auth();
}

/// Get or create dust aggregation for a specific token
fn get_or_create_dust_aggregation(env: &Env, token_address: &Address) -> DustAggregation {
    env.storage()
        .instance()
        .get::<DataKey, DustAggregation>(&DataKey::DustAggregation(token_address.clone()))
        .unwrap_or(DustAggregation {
            total_dust: 0,
            stream_count: 0,
            last_updated: env.ledger().timestamp(),
        })
}

/// Update dust aggregation for a token
fn update_dust_aggregation(env: &Env, token_address: &Address, dust_amount: i128, stream_count_delta: u64) {
    let mut aggregation = get_or_create_dust_aggregation(env, token_address);
    aggregation.total_dust = aggregation.total_dust.saturating_add(dust_amount);
    aggregation.stream_count = aggregation.stream_count.saturating_add(stream_count_delta);
    aggregation.last_updated = env.ledger().timestamp();
    
    env.storage()
        .instance()
        .set(&DataKey::DustAggregation(token_address.clone()), &aggregation);
}

fn get_meter_or_panic(env: &Env, meter_id: u64) -> Meter {
    match env
        .storage()
        .instance()
        .get::<DataKey, Meter>(&DataKey::Meter(meter_id))
    {
        Some(meter) => meter,
        None => panic_with_error!(env, ContractError::MeterNotFound),
    }
}

fn get_oracle_or_panic(env: &Env) -> Address {
    match env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Oracle)
    {
        Some(oracle) => oracle,
        None => panic_with_error!(env, ContractError::OracleNotSet),
    }
}

fn convert_xlm_to_usd_if_needed(
    env: &Env,
    amount: i128,
    _token: &Address,
) -> Result<i128, ContractError> {
    // If an oracle is set, convert with proper rounding. Otherwise pass through as-is.
    match env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Oracle)
    {
        Some(oracle_address) => {
            let oracle_client = PriceOracleClient::new(env, &oracle_address);
            let price_data = oracle_client.get_price();
            let converted_amount = convert_xlm_to_usd_cents_with_rounding(amount, price_data.price);
            Ok(converted_amount)
        }
        None => Ok(amount),
    }
}

fn convert_usd_to_xlm_if_needed(
    env: &Env,
    usd_cents: i128,
    _token: &Address,
) -> Result<i128, ContractError> {
    match env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Oracle)
    {
        Some(oracle_address) => {
            let oracle_client = PriceOracleClient::new(env, &oracle_address);
            let price_data = oracle_client.get_price();
            let xlm_amount = convert_usd_cents_to_xlm_with_rounding(usd_cents, price_data.price);
            Ok(xlm_amount)
        }
        None => Ok(usd_cents),
    }
}

fn remaining_postpaid_collateral(meter: &Meter) -> i128 {
    meter.collateral_limit.saturating_sub(meter.debt).max(0)
}

fn is_peak_hour(timestamp: u64) -> bool {
    let seconds_in_day = timestamp % DAY_IN_SECONDS;
    seconds_in_day >= PEAK_HOUR_START && seconds_in_day < PEAK_HOUR_END
}

fn get_effective_rate(meter: &Meter, timestamp: u64) -> i128 {
    if is_peak_hour(timestamp) {
        meter.peak_rate
    } else {
        meter.off_peak_rate
    }
}

fn provider_meter_value(meter: &Meter) -> i128 {
    match meter.billing_type {
        BillingType::PrePaid => meter.balance.max(0),
        BillingType::PostPaid => remaining_postpaid_collateral(meter),
    }
}

fn refresh_activity(meter: &mut Meter, now: u64) {
    match meter.billing_type {
        BillingType::PrePaid => {
            // Check if meter is in grace period
            if meter.balance <= 0 && meter.grace_period_start == 0 {
                // Balance just hit 0, start grace period
                meter.grace_period_start = now;
                meter.is_active = true; // Keep active during grace period
            } else if meter.balance < 0 && meter.grace_period_start > 0 {
                // Check if grace period has expired
                if now.saturating_sub(meter.grace_period_start) >= GRACE_PERIOD_SECONDS {
                    meter.is_active = false;
                } else {
                    meter.is_active = true; // Still in grace period
                }
            } else if meter.balance > 0 {
                // Reset grace period when balance is positive again
                meter.grace_period_start = 0;
                meter.is_active = meter.balance >= MINIMUM_BALANCE_TO_FLOW;
            } else {
                meter.is_active = meter.balance >= MINIMUM_BALANCE_TO_FLOW;
            }
        }
        BillingType::PostPaid => {
            meter.is_active = remaining_postpaid_collateral(meter) > 0;
        }
    }
}

fn reset_claim_window_if_needed(meter: &mut Meter, now: u64) {
    if now.saturating_sub(meter.last_claim_time) >= HOUR_IN_SECONDS {
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

fn get_provider_window_or_default(
    env: &Env,
    provider: &Address,
    now: u64,
) -> ProviderWithdrawalWindow {
    env.storage()
        .instance()
        .get(&DataKey::ProviderWindow(provider.clone()))
        .unwrap_or(ProviderWithdrawalWindow {
            daily_withdrawn: 0,
            last_reset: now,
        })
}

fn reset_provider_window_if_needed(window: &mut ProviderWithdrawalWindow, now: u64) {
    if now.saturating_sub(window.last_reset) >= DAY_IN_SECONDS {
        window.daily_withdrawn = 0;
        window.last_reset = now;
    }
}

fn get_provider_total_pool_impl(env: &Env, provider: &Address) -> i128 {
    // Use cached provider total pool to avoid unbounded iteration
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ProviderTotalPool(provider.clone()))
        .unwrap_or(0)
}

fn update_provider_total_pool(env: &Env, provider: &Address, old_value: i128, new_value: i128) {
    let current_pool = get_provider_total_pool_impl(env, provider);
    let updated_pool = current_pool
        .saturating_sub(old_value)
        .saturating_add(new_value);
    env.storage()
        .instance()
        .set(&DataKey::ProviderTotalPool(provider.clone()), &updated_pool);
}

fn apply_provider_withdrawal_limit(
    env: &Env,
    provider: &Address,
    amount: i128,
) -> ProviderWithdrawalWindow {
    let now = env.ledger().timestamp();
    let mut window = get_provider_window_or_default(env, provider, now);
    reset_provider_window_if_needed(&mut window, now);

    if amount <= 0 {
        return window;
    }

    let total_pool_before_claim =
        get_provider_total_pool_impl(&env, provider).saturating_add(window.daily_withdrawn);
    let daily_limit = total_pool_before_claim / DAILY_WITHDRAWAL_PERCENT;

    if window.daily_withdrawn.saturating_add(amount) > daily_limit {
        panic_with_error!(env, ContractError::WithdrawalLimitExceeded);
    }

    window.daily_withdrawn = window.daily_withdrawn.saturating_add(amount);
    window
}

fn apply_provider_claim(env: &Env, meter: &mut Meter, amount: i128) {
    if amount <= 0 {
        return;
    }

    transfer_tokens(
        env,
        &meter.token,
        &env.current_contract_address(),
        &meter.provider,
        &amount,
    );

    match meter.billing_type {
        BillingType::PrePaid => {
            meter.balance = meter.balance.saturating_sub(amount);
        }
        BillingType::PostPaid => {
            meter.debt = meter.debt.saturating_add(amount);
        }
    }

    meter.claimed_this_hour = meter.claimed_this_hour.saturating_add(amount);
}

fn publish_active_event(env: &Env, meter_id: u64, now: u64) {
    env.events()
        .publish((symbol_short!("Active"), meter_id), now);
}

fn publish_inactive_event(env: &Env, meter_id: u64, now: u64) {
    env.events()
        .publish((symbol_short!("Inactive"), meter_id), now);
}

// Continuous Flow Math Engine Functions

/// Create a new continuous flow stream with timestamp-based tracking and buffer vault
fn create_continuous_flow(
    stream_id: u64,
    flow_rate_per_second: i128,
    initial_balance: i128,
    buffer_amount: i128,
    current_timestamp: u64,
    provider: Address,
    payer: Address,
) -> ContinuousFlow {
    ContinuousFlow {
        stream_id,
        flow_rate_per_second,
        accumulated_balance: initial_balance,
        last_flow_timestamp: current_timestamp,
        created_timestamp: current_timestamp,
        status: if initial_balance > 0 { StreamStatus::Active } else { StreamStatus::Paused },
        paused_at: 0, // Not paused initially
        provider,
        buffer_balance: buffer_amount,
        buffer_warning_sent: false,
        payer,
    }
}

/// Calculate required buffer amount (24 hours of flow rate)
fn calculate_required_buffer(flow_rate_per_second: i128) -> i128 {
    let buffer_duration_i128 = BUFFER_DURATION_SECONDS as i128;
    flow_rate_per_second.saturating_mul(buffer_duration_i128)
}

/// Calculate flow accumulation since last update with precise timestamp math and buffer logic
fn calculate_flow_accumulation(
    flow: &ContinuousFlow,
    current_timestamp: u64,
) -> i128 {
    if flow.status != StreamStatus::Active {
        return 0;
    }

    // Prevent underflow with checked subtraction
    let elapsed_seconds = match current_timestamp.checked_sub(flow.last_flow_timestamp) {
        Some(elapsed) => elapsed,
        None => return 0, // Timestamp went backwards, no accumulation
    };

    // Use i128 for precise calculation to prevent overflow
    let elapsed_i128 = elapsed_seconds as i128;
    
    // Calculate accumulated flow: rate * time
    // flow_rate_per_second is in micro-stroops per second
    let accumulated = flow.flow_rate_per_second.saturating_mul(elapsed_i128);
    
    accumulated
}

/// Update flow with new timestamp and handle underflow risks with buffer depletion logic
fn update_continuous_flow(
    env: &Env,
    flow: &mut ContinuousFlow,
    current_timestamp: u64,
) -> Result<i128, ContractError> {
    let accumulation = calculate_flow_accumulation(flow, current_timestamp);
    
    let mut total_deduction = accumulation;
    let mut buffer_used = 0i128;
    
    // First, try to deduct from main balance
    if flow.accumulated_balance >= accumulation {
        // Normal case: deduct from main balance only
        flow.accumulated_balance = flow.accumulated_balance.saturating_sub(accumulation);
    } else {
        // Main balance insufficient, use buffer
        let remaining_deduction = accumulation.saturating_sub(flow.accumulated_balance);
        buffer_used = remaining_deduction;
        
        // Check if buffer has sufficient funds
        if flow.buffer_balance < remaining_deduction {
            // Buffer insufficient, terminate stream
            let actual_buffer_deduction = flow.buffer_balance;
            total_deduction = flow.accumulated_balance.saturating_add(actual_buffer_deduction);
            
            flow.accumulated_balance = 0;
            flow.buffer_balance = 0;
            flow.status = StreamStatus::Depleted;
            flow.last_flow_timestamp = current_timestamp;
            
            // Emit BufferDepleted event
            env.events().publish(
                symbol_short!("BufferDepleted"),
                (flow.stream_id, current_timestamp, actual_buffer_deduction, flow.provider.clone())
            );
            
            return Ok(total_deduction);
        }
        
        // Deduct from main balance (to zero) and then from buffer
        total_deduction = flow.accumulated_balance.saturating_add(remaining_deduction);
        flow.accumulated_balance = 0;
        flow.buffer_balance = flow.buffer_balance.saturating_sub(remaining_deduction);
        
        // Check if buffer warning should be sent
        if !flow.buffer_warning_sent && flow.buffer_balance <= BUFFER_WARNING_THRESHOLD {
            flow.buffer_warning_sent = true;
            
            // Emit BufferWarning event
            env.events().publish(
                symbol_short!("BufferWarning"),
                (flow.stream_id, current_timestamp, flow.buffer_balance, flow.provider.clone())
            );
        }
    }
    
    flow.last_flow_timestamp = current_timestamp;
    
    // Update status based on remaining balances
    if flow.accumulated_balance == 0 && flow.buffer_balance == 0 {
        flow.status = StreamStatus::Depleted;
    } else if flow.status == StreamStatus::Paused && (flow.accumulated_balance > 0 || flow.buffer_balance > 0) {
        flow.status = StreamStatus::Active;
    }
    
    Ok(total_deduction)
}

/// Pause a continuous flow stream (provider only)
/// Halts time-delta calculation immediately and records paused_at timestamp
fn pause_stream(env: &Env, stream_id: u64, provider: &Address) -> Result<(), ContractError> {
    let mut flow = get_continuous_flow_or_panic(env, stream_id);
    
    // Verify provider authorization
    if flow.provider != *provider {
        panic_with_error!(env, ContractError::UnauthorizedAdmin);
    }
    
    // Only allow pausing active streams
    if flow.status != StreamStatus::Active {
        return Err(ContractError::InvalidTokenAmount); // Reuse error for invalid state
    }
    
    let current_timestamp = env.ledger().timestamp();
    
    // Update flow calculation up to pause moment
    update_continuous_flow(env, &mut flow, current_timestamp)?;
    
    // Set paused status and record timestamp
    flow.status = StreamStatus::Paused;
    flow.paused_at = current_timestamp;
    flow.flow_rate_per_second = 0; // Stop the flow
    
    // Store updated flow
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
    
    // Emit StreamPaused event
    env.events().publish(
        symbol_short!("StreamPaused"),
        (stream_id, current_timestamp, provider.clone(), flow.accumulated_balance)
    );
    
    Ok(())
}

/// Resume a continuous flow stream (provider only)
/// Restarts the flow and adjusts timing based on pause duration
fn resume_stream(env: &Env, stream_id: u64, new_flow_rate: i128, provider: &Address) -> Result<(), ContractError> {
    if new_flow_rate <= 0 {
        return Err(ContractError::InvalidTokenAmount);
    }
    
    let mut flow = get_continuous_flow_or_panic(env, stream_id);
    
    // Verify provider authorization
    if flow.provider != *provider {
        panic_with_error!(env, ContractError::UnauthorizedAdmin);
    }
    
    // Only allow resuming paused streams
    if flow.status != StreamStatus::Paused {
        return Err(ContractError::InvalidTokenAmount); // Reuse error for invalid state
    }
    
    let current_timestamp = env.ledger().timestamp();
    
    // Calculate pause duration
    let pause_duration = current_timestamp.saturating_sub(flow.paused_at);
    
    // Handle edge case: stream depleted exactly when paused
    if flow.accumulated_balance == 0 && flow.buffer_balance == 0 {
        flow.status = StreamStatus::Depleted;
        env.storage()
            .instance()
            .set(&DataKey::ContinuousFlow(stream_id), &flow);
        return Err(ContractError::InvalidTokenAmount); // Cannot resume depleted stream
    }
    
    // Resume the stream with new flow rate
    flow.status = StreamStatus::Active;
    flow.flow_rate_per_second = new_flow_rate;
    flow.last_flow_timestamp = current_timestamp; // Reset flow timestamp
    flow.paused_at = 0; // Clear pause timestamp
    
    // Store updated flow
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
    
    // Emit StreamResumed event
    env.events().publish(
        symbol_short!("StreamResumed"),
        (stream_id, current_timestamp, provider.clone(), new_flow_rate, pause_duration)
    );
    
    Ok(())
}

/// Update flow rate with authentication and event emission
fn update_flow_rate(
    env: &Env,
    stream_id: u64,
    new_flow_rate: i128,
) -> Result<(), ContractError> {
    let mut flow = get_continuous_flow_or_panic(env, stream_id);
    
    // Require authentication for flow rate changes
    env.current_contract_address().require_auth();
    
    let old_flow_rate = flow.flow_rate_per_second;
    let old_status = flow.status;
    
    flow.flow_rate_per_second = new_flow_rate;
    
    // Update status based on new flow rate and balance
    if new_flow_rate == 0 {
        flow.status = StreamStatus::Paused;
    } else if (flow.accumulated_balance > 0 || flow.buffer_balance > 0) && flow.status == StreamStatus::Paused {
        flow.status = StreamStatus::Active;
    }
    
    // Update timestamp to current time
    let current_timestamp = env.ledger().timestamp();
    
    // Emit detailed StreamUpdated event
    let event = StreamUpdatedEvent {
        stream_id,
        old_flow_rate,
        new_flow_rate,
        timestamp: current_timestamp,
        old_status,
        new_status: flow.status,
    };
    
    env.events().publish(
        symbol_short!("StreamUpdated"),
        (stream_id, old_flow_rate, new_flow_rate, current_timestamp, old_status as u32, flow.status as u32)
    );
    
    // Store updated flow
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
    
    Ok(())
}

/// Get continuous flow or panic if not found
fn get_continuous_flow_or_panic(env: &Env, stream_id: u64) -> ContinuousFlow {
    match env
        .storage()
        .instance()
        .get::<DataKey, ContinuousFlow>(&DataKey::ContinuousFlow(stream_id))
    {
        Some(flow) => flow,
        None => panic_with_error!(env, ContractError::MeterNotFound), // Reuse existing error
    }
}

/// Refund buffer to payer on amicable stream closure
fn refund_buffer(env: &Env, stream_id: u64) -> Result<i128, ContractError> {
    let mut flow = get_continuous_flow_or_panic(env, stream_id);
    
    // Only refund if stream is not depleted (amicable closure)
    if flow.status == StreamStatus::Depleted {
        return Err(ContractError::BufferAlreadyDepleted);
    }
    
    let buffer_amount = flow.buffer_balance;
    
    if buffer_amount <= 0 {
        return Err(ContractError::InsufficientBuffer);
    }
    
    // Clear buffer balance
    flow.buffer_balance = 0;
    flow.status = StreamStatus::Depleted;
    
    // Store updated flow
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
    
    // Transfer buffer back to payer
    transfer_tokens(
        env,
        &env.current_contract_address(), // Assuming native token for simplicity
        &env.current_contract_address(),
        &flow.payer,
        &buffer_amount,
    );
    
    // Emit refund event
    env.events().publish(
        symbol_short!("BufferRefunded"),
        (stream_id, buffer_amount, flow.payer.clone())
    );
    
    Ok(buffer_amount)
}

/// Add additional buffer to existing stream
fn add_buffer_to_stream(
    env: &Env,
    stream_id: u64,
    additional_buffer: i128,
) -> Result<(), ContractError> {
    if additional_buffer <= 0 {
        return Err(ContractError::InvalidTokenAmount);
    }
    
    let mut flow = get_continuous_flow_or_panic(env, stream_id);
    
    // Verify payer authorization
    flow.payer.require_auth();
    
    // Update flow calculation first
    let current_timestamp = env.ledger().timestamp();
    update_continuous_flow(env, &mut flow, current_timestamp)?;
    
    // Add buffer with overflow protection
    flow.buffer_balance = flow.buffer_balance.saturating_add(additional_buffer);
    
    // Reset warning flag if buffer was significantly increased
    if additional_buffer >= BUFFER_WARNING_THRESHOLD {
        flow.buffer_warning_sent = false;
    }
    
    // Store updated flow
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
    
    // Emit buffer added event
    env.events().publish(
        symbol_short!("BufferAdded"),
        (stream_id, additional_buffer)
    );
    
    Ok(())
}

/// Add balance to continuous flow with underflow protection
fn add_balance_to_flow(
    env: &Env,
    stream_id: u64,
    additional_balance: i128,
) -> Result<(), ContractError> {
    if additional_balance <= 0 {
        return Err(ContractError::InvalidTokenAmount);
    }
    
    let mut flow = get_continuous_flow_or_panic(env, stream_id);
    
    // Update flow calculation first
    let current_timestamp = env.ledger().timestamp();
    update_continuous_flow(env, &mut flow, current_timestamp)?;
    
    // Add new balance with overflow protection
    flow.accumulated_balance = flow.accumulated_balance.saturating_add(additional_balance);
    
    // Update status if needed
    if (flow.accumulated_balance > 0 || flow.buffer_balance > 0) && flow.flow_rate_per_second > 0 {
        flow.status = StreamStatus::Active;
    }
    
    // Store updated flow
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
    
    Ok(())
}

/// Withdraw from continuous flow with high-frequency safety
fn withdraw_from_flow(
    env: &Env,
    stream_id: u64,
    withdrawal_amount: i128,
) -> Result<i128, ContractError> {
    if withdrawal_amount <= 0 {
        return Err(ContractError::InvalidTokenAmount);
    }
    
    let mut flow = get_continuous_flow_or_panic(env, stream_id);
    
    // Update flow calculation first
    let current_timestamp = env.ledger().timestamp();
    update_continuous_flow(env, &mut flow, current_timestamp)?;
    
    // Check if sufficient balance available (main balance only, buffer is protected)
    if flow.accumulated_balance < withdrawal_amount {
        return Err(ContractError::InvalidTokenAmount);
    }
    
    // Perform withdrawal from main balance only
    flow.accumulated_balance = flow.accumulated_balance.saturating_sub(withdrawal_amount);
    
    // Update status if depleted
    if flow.accumulated_balance == 0 && flow.buffer_balance == 0 {
        flow.status = StreamStatus::Depleted;
    }
    
    // Store updated flow
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
    
    Ok(withdrawal_amount)
}

#[contractimpl]
impl UtilityContract {
    pub fn get_minimum_balance_to_flow() -> i128 {
        MINIMUM_BALANCE_TO_FLOW
    }

    pub fn set_oracle(env: Env, oracle_address: Address) {
        // This should be called by admin to set the oracle address
        env.storage()
            .instance()
            .set(&DataKey::Oracle, &oracle_address);
    }

    pub fn set_maintenance_config(env: Env, wallet: Address, fee_bps: i128) {
        env.storage()
            .instance()
            .set(&DataKey::MaintenanceWallet, &wallet);
        env.storage()
            .instance()
            .set(&DataKey::ProtocolFeeBps, &fee_bps);
    }

    /// Set admin address for dust sweeper authorization
    pub fn set_admin(env: Env, admin_address: Address) {
        env.current_contract_address().require_auth();
        env.storage()
            .instance()
            .set(&DataKey::AdminAddress, &admin_address);
    }

    /// Add funds to gas bounty pool for dust sweepers
    pub fn fund_gas_bounty(env: Env, amount: i128) {
        require_admin_auth(&env);
        
        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        let current_bounty = env
            .storage()
            .instance()
            .get::<DataKey, i128>(&DataKey::GasBountyPool)
            .unwrap_or(0);
        
        let updated_bounty = current_bounty.saturating_add(amount);
        env.storage()
            .instance()
            .set(&DataKey::GasBountyPool, &updated_bounty);

        env.events()
            .publish(symbol_short!("BountyFunded"), amount);
    }

    pub fn add_supported_token(env: Env, token: Address) {
        env.storage()
            .instance()
            .set(&DataKey::SupportedToken(token), &true);
    }

    pub fn remove_supported_token(env: Env, token: Address) {
        env.storage()
            .instance()
            .set(&DataKey::SupportedToken(token), &false);
    }

    pub fn register_meter(
        env: Env,
        user: Address,
        provider: Address,
        off_peak_rate: i128,
        token: Address,
        device_public_key: BytesN<32>,
    ) -> u64 {
        Self::register_meter_with_mode(
            env,
            user,
            provider,
            off_peak_rate,
            token,
            BillingType::PrePaid,
            device_public_key,
        )
    }

    pub fn register_meter_with_mode(
        env: Env,
        user: Address,
        provider: Address,
        off_peak_rate: i128,
        token: Address,
        billing_type: BillingType,
        device_public_key: BytesN<32>,
    ) -> u64 {
        user.require_auth();

        let mut count = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::Count)
            .unwrap_or(0);
        count += 1;

        let now = env.ledger().timestamp();
        let peak_rate = off_peak_rate.saturating_mul(PEAK_RATE_MULTIPLIER) / RATE_PRECISION;

        let usage_data = UsageData {
            total_watt_hours: 0,
            current_cycle_watt_hours: 0,
            peak_usage_watt_hours: 0,
            last_reading_timestamp: now,
            precision_factor: 1000,
        };

        let meter = Meter {
            user: user.clone(),
            provider: provider.clone(),
            billing_type,
            off_peak_rate,
            peak_rate,
            rate_per_second: off_peak_rate,
            rate_per_unit: off_peak_rate,
            balance: 0,
            debt: 0,
            collateral_limit: 0,
            last_update: now,
            is_active: false,
            token,
            usage_data,
            max_flow_rate_per_hour: off_peak_rate.saturating_mul(HOUR_IN_SECONDS as i128),
            last_claim_time: now,
            claimed_this_hour: 0,
            heartbeat: now,
            device_public_key,
            is_paired: false,
            grace_period_start: 0,
        };

        env.storage().instance().set(&DataKey::Meter(count), &meter);
        env.storage().instance().set(&DataKey::Count, &count);

        // Initialize provider total pool (new meter starts with 0 value)
        let current_pool = get_provider_total_pool_impl(&env, &provider);
        env.storage()
            .instance()
            .set(&DataKey::ProviderTotalPool(provider), &current_pool);

        count
    }

    pub fn batch_register_meters(env: Env, meter_infos: Vec<MeterInfo>) -> BatchCreatedEvent {
        if meter_infos.is_empty() {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        // Require authorization for all users in the batch
        for meter_info in meter_infos.iter() {
            meter_info.user.require_auth();
        }

        let mut count = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::Count)
            .unwrap_or(0);

        let start_id = count + 1;
        let now = env.ledger().timestamp();

        // Track providers initialized to avoid duplicate initialization
        let mut providers_initialized: Vec<Address> = Vec::new(&env);

        for meter_info in meter_infos.iter() {
            count += 1;

            let provider_clone = meter_info.provider.clone();
            let peak_rate = meter_info
                .off_peak_rate
                .saturating_mul(PEAK_RATE_MULTIPLIER)
                / RATE_PRECISION;

            let usage_data = UsageData {
                total_watt_hours: 0,
                current_cycle_watt_hours: 0,
                peak_usage_watt_hours: 0,
                last_reading_timestamp: now,
                precision_factor: 1000,
            };

            let meter = Meter {
                user: meter_info.user.clone(),
                provider: provider_clone.clone(),
                billing_type: meter_info.billing_type,
                off_peak_rate: meter_info.off_peak_rate,
                peak_rate,
                rate_per_second: meter_info.off_peak_rate,
                rate_per_unit: meter_info.off_peak_rate,
                balance: 0,
                debt: 0,
                collateral_limit: 0,
                last_update: now,
                is_active: false,
                token: meter_info.token.clone(),
                usage_data,
                max_flow_rate_per_hour: meter_info
                    .off_peak_rate
                    .saturating_mul(HOUR_IN_SECONDS as i128),
                last_claim_time: now,
                claimed_this_hour: 0,
                heartbeat: now,
                device_public_key: meter_info.device_public_key,
                is_paired: false,
            };

            env.storage().instance().set(&DataKey::Meter(count), &meter);

            // Initialize provider total pool only once per provider
            let mut already_initialized = false;
            for provider in providers_initialized.iter() {
                if provider.clone() == provider_clone {
                    already_initialized = true;
                    break;
                }
            }

            if !already_initialized {
                let current_pool = get_provider_total_pool_impl(&env, &provider_clone);
                env.storage().instance().set(
                    &DataKey::ProviderTotalPool(provider_clone.clone()),
                    &current_pool,
                );
                providers_initialized.push_back(provider_clone);
            }
        }

        // Update the global count
        env.storage().instance().set(&DataKey::Count, &count);

        let batch_event = BatchCreatedEvent {
            start_id,
            end_id: count,
            count: count - start_id + 1,
        };

        // Emit single BatchCreated event
        env.events().publish(
            symbol_short!("BatchCreated"),
            (batch_event.start_id, batch_event.end_id, batch_event.count),
        );

        batch_event
    }

    pub fn top_up(env: Env, meter_id: u64, amount: i128) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        let was_active = meter.is_active;
        let old_meter_value = provider_meter_value(&meter);
        // Transfer tokens from user to contract
        let token_client = token::Client::new(&env, &meter.token);
        token_client.transfer(&meter.user, &env.current_contract_address(), &amount);

        // Convert XLM to USD cents if needed
        let converted_amount = match convert_xlm_to_usd_if_needed(&env, amount, &meter.token) {
            Ok(amount) => amount,
            Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
        };

        if converted_amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        match meter.billing_type {
            BillingType::PrePaid => {
                // Auto-deduct debt first if in debt mode
                if meter.balance < 0 {
                    let debt_settlement = converted_amount.min(meter.balance.abs());
                    meter.balance = meter.balance.saturating_add(debt_settlement);
                    let remaining_amount = converted_amount.saturating_sub(debt_settlement);
                    meter.balance = meter.balance.saturating_add(remaining_amount);
                } else {
                    meter.balance = meter.balance.saturating_add(converted_amount);
                }
            }
            BillingType::PostPaid => {
                let settlement = converted_amount.min(meter.debt.max(0));
                meter.debt = meter.debt.saturating_sub(settlement);
                meter.collateral_limit = meter
                    .collateral_limit
                    .saturating_add(converted_amount.saturating_sub(settlement));
            }
        }

        let now = env.ledger().timestamp();
        refresh_activity(&mut meter, now);

        if !was_active && meter.is_active {
            meter.last_update = now;
            publish_active_event(&env, meter_id, now);
        }

        // Update provider total pool
        let new_meter_value = provider_meter_value(&meter);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, new_meter_value);

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);

        // Emit conversion event
        env.events().publish(
            (symbol_short!("TokenUp"), meter_id),
            (amount, converted_amount),
        );
    }

    pub fn initiate_pairing(env: Env, meter_id: u64) -> BytesN<32> {
        let meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        if meter.is_paired {
            panic_with_error!(env, ContractError::PairingAlreadyComplete);
        }

        // Generate a pseudo-random challenge using contract context and ledger info
        let challenge_data = PairingChallengeData {
            contract: env.current_contract_address(),
            meter_id,
            timestamp: env.ledger().timestamp(),
        };

        let challenge = env.crypto().sha256(&challenge_data.to_xdr(&env));

        env.storage()
            .instance()
            .set(&DataKey::PairingChallenge(meter_id), &challenge);

        env.events()
            .publish((symbol_short!("PairInit"), meter_id), challenge.clone());

        challenge.into()
    }

    pub fn complete_pairing(env: Env, meter_id: u64, signature: BytesN<64>) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        let challenge: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::PairingChallenge(meter_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::ChallengeNotFound));

        // Create the message that was signed
        let pairing_data = PairingChallengeData {
            contract: env.current_contract_address(),
            meter_id,
            timestamp: env.ledger().timestamp(),
        };

        // Verify the signature
        #[cfg(not(test))]
        env.crypto().ed25519_verify(
            &meter.device_public_key,
            &pairing_data.to_xdr(&env),
            &signature,
        );

        // Clear the challenge
        env.storage()
            .instance()
            .remove(&DataKey::PairingChallenge(meter_id));

        meter.is_paired = true;
        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);

        env.events()
            .publish((symbol_short!("PairComplete"), meter_id), signature);
    }

    pub fn deduct_units(env: Env, signed_data: SignedUsageData) {
        let mut meter = get_meter_or_panic(&env, signed_data.meter_id);
        meter.provider.require_auth();

        // Verify the signature and pairing
        verify_usage_signature(&env, &signed_data, &meter)?;

        // Store old meter value for pool update
        let old_meter_value = provider_meter_value(&meter);

        if !meter.is_paired {
            panic_with_error!(&env, ContractError::MeterNotPaired);
        }

        let now = env.ledger().timestamp();
        let effective_rate = get_effective_rate(&meter, signed_data.timestamp);
        let cost = signed_data.units_consumed.saturating_mul(effective_rate);

        // Apply provider withdrawal limits
        let mut window = apply_provider_withdrawal_limit(&env, &meter.provider, cost);

        // Apply the claim
        apply_provider_claim(&env, &mut meter, cost);

        // Update provider window
        window.daily_withdrawn = window.daily_withdrawn.saturating_add(cost);
        env.storage()
            .instance()
            .set(&DataKey::ProviderWindow(meter.provider.clone()), &window);

        // Update usage data
        meter.usage_data.total_watt_hours = meter
            .usage_data
            .total_watt_hours
            .saturating_add(signed_data.watt_hours_consumed);
        meter.usage_data.current_cycle_watt_hours = meter
            .usage_data
            .current_cycle_watt_hours
            .saturating_add(signed_data.watt_hours_consumed);

        if meter.usage_data.current_cycle_watt_hours > meter.usage_data.peak_usage_watt_hours {
            meter.usage_data.peak_usage_watt_hours = meter.usage_data.current_cycle_watt_hours;
        }

        // Update activity status with grace period logic
        refresh_activity(&mut meter, now);

        meter.last_update = now;

        // Update provider total pool
        let new_meter_value = provider_meter_value(&meter);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, new_meter_value);

        env.storage()
            .instance()
            .set(&DataKey::Meter(signed_data.meter_id), &meter);

        // Emit UsageReported event
        env.events().publish(
            (Symbol::new(&env, "UsageReported"), signed_data.meter_id),
            (signed_data.units_consumed, cost),
        );
    }

    pub fn claim(env: Env, meter_id: u64) {
        let mut meter: Meter = env
            .storage()
            .instance()
            .get(&DataKey::Meter(meter_id))
            .ok_or("Meter not found")
            .unwrap();
        meter.provider.require_auth();

        // Store old meter value for pool update
        let old_meter_value = provider_meter_value(&meter);

        let now = env.ledger().timestamp();
        let elapsed = now.checked_sub(meter.last_update).unwrap_or(0);
        let amount = (elapsed as i128) * meter.rate_per_unit;

        // Check if we're in the same hour as last claim
        let current_hour = now / 3600;
        let last_claim_hour = meter.last_claim_time / 3600;

        if current_hour == last_claim_hour {
            // Same hour, check if we exceed max flow rate
            let max_allowed = meter.max_flow_rate_per_hour - meter.claimed_this_hour;
            let actual_amount = if amount > max_allowed {
                max_allowed
            } else {
                amount
            };

            // Ensure we don't exceed debt threshold
            let claimable = if actual_amount > meter.balance
                && meter.balance - actual_amount >= DEBT_THRESHOLD
            {
                actual_amount
            } else if actual_amount > meter.balance {
                meter.balance - DEBT_THRESHOLD // Allow going down to threshold
            } else {
                actual_amount
            };

            if claimable > 0 {
                let client = token::Client::new(&env, &meter.token);
                let mut payout = claimable;

                if let Some(wallet) = env
                    .storage()
                    .instance()
                    .get::<_, Address>(&DataKey::MaintenanceWallet)
                {
                    let fee_bps: i128 = env
                        .storage()
                        .instance()
                        .get(&DataKey::ProtocolFeeBps)
                        .unwrap_or(0);
                    let fee = (claimable * fee_bps) / 10000;
                    payout -= fee;
                    if fee > 0 {
                        client.transfer(&env.current_contract_address(), &wallet, &fee);
                    }
                }
                if payout > 0 {
                    client.transfer(&env.current_contract_address(), &meter.provider, &payout);
                }
                meter.balance -= claimable;
                meter.claimed_this_hour += claimable;
            }
        } else {
            // New hour, reset claimed_this_hour
            meter.claimed_this_hour = 0;

            // Ensure we don't exceed debt threshold
            let claimable = if amount > meter.balance && meter.balance - amount >= DEBT_THRESHOLD {
                amount
            } else if amount > meter.balance {
                meter.balance - DEBT_THRESHOLD // Allow going down to threshold
            } else {
                amount
            };

            if claimable > 0 {
                let client = token::Client::new(&env, &meter.token);
                let mut payout = claimable;

                if let Some(wallet) = env
                    .storage()
                    .instance()
                    .get::<_, Address>(&DataKey::MaintenanceWallet)
                {
                    let fee_bps: i128 = env
                        .storage()
                        .instance()
                        .get(&DataKey::ProtocolFeeBps)
                        .unwrap_or(0);
                    let fee = (claimable * fee_bps) / 10000;
                    payout -= fee;
                    if fee > 0 {
                        client.transfer(&env.current_contract_address(), &wallet, &fee);
                    }
                }
                if payout > 0 {
                    client.transfer(&env.current_contract_address(), &meter.provider, &payout);
                }
                meter.balance -= claimable;
                meter.claimed_this_hour = claimable;
            }
        }

        meter.last_update = now;
        meter.last_claim_time = now;

        // Update activity status with grace period logic
        refresh_activity(&mut meter, now);

        // Update provider total pool
        let new_meter_value = provider_meter_value(&meter);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, new_meter_value);

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn update_usage(env: Env, meter_id: u64, watt_hours_consumed: i128) {
        // Input validation for security
        if watt_hours_consumed < 0 {
            panic_with_error!(env, ContractError::InvalidUsageValue);
        }

        if watt_hours_consumed > MAX_USAGE_PER_UPDATE {
            panic_with_error!(env, ContractError::UsageExceedsLimit);
        }

        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        let precise_consumption =
            watt_hours_consumed.saturating_mul(meter.usage_data.precision_factor);
        meter.usage_data.total_watt_hours = meter
            .usage_data
            .total_watt_hours
            .saturating_add(precise_consumption);
        meter.usage_data.current_cycle_watt_hours = meter
            .usage_data
            .current_cycle_watt_hours
            .saturating_add(precise_consumption);

        if meter.usage_data.current_cycle_watt_hours > meter.usage_data.peak_usage_watt_hours {
            meter.usage_data.peak_usage_watt_hours = meter.usage_data.current_cycle_watt_hours;
        }

        meter.usage_data.last_reading_timestamp = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn reset_cycle_usage(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();
        meter.usage_data.current_cycle_watt_hours = 0;
        meter.usage_data.last_reading_timestamp = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn get_usage_data(env: Env, meter_id: u64) -> Option<UsageData> {
        env.storage()
            .instance()
            .get::<DataKey, Meter>(&DataKey::Meter(meter_id))
            .map(|meter| meter.usage_data)
    }

    pub fn get_meter(env: Env, meter_id: u64) -> Option<Meter> {
        env.storage()
            .instance()
            .get::<DataKey, Meter>(&DataKey::Meter(meter_id))
    }

    pub fn get_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::Count)
            .unwrap_or(0)
    }

    pub fn get_provider_window(env: Env, provider: Address) -> Option<ProviderWithdrawalWindow> {
        env.storage()
            .instance()
            .get(&DataKey::ProviderWindow(provider))
    }

    pub fn get_provider_total_pool(env: Env, provider: Address) -> i128 {
        get_provider_total_pool_impl(&env, &provider)
    }

    pub fn get_watt_hours_display(precise_watt_hours: i128, precision_factor: i128) -> i128 {
        if precision_factor <= 0 {
            return precise_watt_hours; // Fallback to avoid division by zero
        }
        precise_watt_hours / precision_factor
    }

    pub fn calculate_expected_depletion(env: Env, meter_id: u64) -> Option<u64> {
        if let Some(meter) = env
            .storage()
            .instance()
            .get::<_, Meter>(&DataKey::Meter(meter_id))
        {
            if meter.balance <= 0 || meter.rate_per_unit <= 0 {
                return Some(0); // Already depleted or no consumption
            }

            let seconds_until_depletion = meter.balance / meter.rate_per_unit;
            let current_time = env.ledger().timestamp();
            Some(current_time + seconds_until_depletion as u64)
        } else {
            None
        }
    }

    pub fn emergency_shutdown(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        // Emergency shutdown always disables the meter regardless of balance
        meter.is_active = false;

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn set_max_flow_rate(env: Env, meter_id: u64, max_rate_per_hour: i128) {
        let mut meter: Meter = env
            .storage()
            .instance()
            .get(&DataKey::Meter(meter_id))
            .ok_or("Meter not found")
            .unwrap();
        meter.provider.require_auth();

        meter.max_flow_rate_per_hour = max_rate_per_hour;

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn update_heartbeat(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();
        meter.heartbeat = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn withdraw_earnings(env: Env, meter_id: u64, amount_usd_cents: i128) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        if amount_usd_cents <= 0 {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        // Store old meter value for pool update
        let old_meter_value = provider_meter_value(&meter);

        let available_earnings = match meter.billing_type {
            BillingType::PrePaid => meter.balance,
            BillingType::PostPaid => meter.debt,
        };

        if amount_usd_cents > available_earnings {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        // Convert USD cents to XLM if needed
        let withdrawal_amount =
            match convert_usd_to_xlm_if_needed(&env, amount_usd_cents, &meter.token) {
                Ok(amount) => amount,
                Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
            };

        let client = token::Client::new(&env, &meter.token);
        client.transfer(
            &env.current_contract_address(),
            &meter.provider,
            &withdrawal_amount,
        );

        // Update meter balance/debt
        match meter.billing_type {
            BillingType::PrePaid => {
                meter.balance = meter.balance.saturating_sub(amount_usd_cents);
            }
            BillingType::PostPaid => {
                meter.debt = meter.debt.saturating_sub(amount_usd_cents);
            }
        }

        let now = env.ledger().timestamp();
        let was_active = meter.is_active;
        refresh_activity(&mut meter, now);

        if !was_active && meter.is_active {
            meter.last_update = now;
        }

        // Update provider total pool
        let new_meter_value = provider_meter_value(&meter);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, new_meter_value);

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);

        // Emit conversion event if XLM was used
        if is_native_token(&meter.token) {
            env.events().publish(
                (symbol_short!("USDtoXLM"), meter_id),
                (amount_usd_cents, withdrawal_amount),
            );
        }
    }

    pub fn get_current_rate(env: Env) -> Option<PriceData> {
        match env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Oracle)
        {
            Some(oracle_address) => {
                let oracle_client = PriceOracleClient::new(&env, &oracle_address);
                Some(oracle_client.get_price())
            }
            None => None,
        }
    }

    pub fn get_provider_total_pool(env: Env, provider: Address) -> i128 {
        get_provider_total_pool_impl(&env, &provider)
    }

    pub fn is_meter_offline(env: Env, meter_id: u64) -> bool {
        match env
            .storage()
            .instance()
            .get::<DataKey, Meter>(&DataKey::Meter(meter_id))
        {
            Some(meter) => {
                env.ledger().timestamp().saturating_sub(meter.heartbeat) > HOUR_IN_SECONDS
            }
            None => true,
        }
    }

    pub fn get_watt_hours_display(watt_hours: i128, precision_factor: i128) -> i128 {
        watt_hours / precision_factor
    }

    /// Unlink a meter from its current tenant and link it to a new tenant.
    /// All historical usage data is preserved. Requires auth from the current
    /// user, the new user, and the provider.
    pub fn transfer_meter_ownership(env: Env, meter_id: u64, new_user: Address) {
        let mut meter = get_meter_or_panic(&env, meter_id);

        meter.user.require_auth();
        meter.provider.require_auth();
        new_user.require_auth();

        let old_user = meter.user.clone();
        let old_meter_value = provider_meter_value(&meter);
        meter.user = new_user.clone();

        // Update provider total pool (provider stays the same, only user changes)
        let new_meter_value = provider_meter_value(&meter);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, new_meter_value);

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);

        env.events()
            .publish((symbol_short!("Transfer"), meter_id), (old_user, new_user));
    }

    // Continuous Flow Engine Public Interface

    /// Create a new continuous flow stream with mandatory buffer deposit
    /// Buffer must equal at least 24 hours of the negotiated flow rate
    pub fn create_continuous_stream(
        env: Env,
        stream_id: u64,
        flow_rate_per_second: i128,
        initial_balance: i128,
        provider: Address,
        payer: Address,
    ) {
        provider.require_auth(); // Provider must authorize stream creation
        payer.require_auth(); // Payer must authorize buffer deposit
        
        if flow_rate_per_second < 0 || initial_balance < 0 {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }
        
        // Calculate required buffer (24 hours of flow rate)
        let required_buffer = calculate_required_buffer(flow_rate_per_second);
        
        // Transfer buffer from payer to contract
        transfer_tokens(
            &env,
            &env.current_contract_address(), // Assuming native token
            &payer,
            &env.current_contract_address(),
            &required_buffer,
        );
        
        let current_timestamp = env.ledger().timestamp();
        let flow = create_continuous_flow(
            stream_id, 
            flow_rate_per_second, 
            initial_balance, 
            required_buffer,
            current_timestamp, 
            provider.clone(),
            payer.clone()
        );
        
        env.storage()
            .instance()
            .set(&DataKey::ContinuousFlow(stream_id), &flow);
        
        env.events().publish(
            symbol_short!("StreamCreated"),
            (stream_id, flow_rate_per_second, initial_balance, required_buffer, current_timestamp, provider, payer)
        );
    }

    /// Update the flow rate of an existing continuous stream
    pub fn update_continuous_flow_rate(env: Env, stream_id: u64, new_flow_rate: i128) {
        if new_flow_rate < 0 {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }
        
        update_flow_rate(&env, stream_id, new_flow_rate).unwrap();
    }

    /// Add balance to a continuous flow stream
    pub fn add_continuous_balance(env: Env, stream_id: u64, additional_balance: i128) {
        add_balance_to_flow(&env, stream_id, additional_balance).unwrap();
        
        env.events().publish(
            symbol_short!("BalanceAdded"),
            (stream_id, additional_balance)
        );
    }

    /// Add additional buffer to an existing stream
    pub fn add_continuous_buffer(env: Env, stream_id: u64, additional_buffer: i128) {
        add_buffer_to_stream(&env, stream_id, additional_buffer).unwrap();
    }

    /// Close stream amicably and refund buffer to payer
    pub fn close_stream_amicably(env: Env, stream_id: u64) -> i128 {
        let provider = env.invoker(); // Use invoker as provider
        let mut flow = get_continuous_flow_or_panic(&env, stream_id);
        
        // Verify provider authorization
        if flow.provider != provider {
            panic_with_error!(&env, ContractError::UnauthorizedAdmin);
        }
        
        // Update flow calculation first
        let current_timestamp = env.ledger().timestamp();
        update_continuous_flow(&env, &mut flow, current_timestamp).unwrap();
        
        // Refund buffer
        let refunded_amount = refund_buffer(&env, stream_id).unwrap();
        
        refunded_amount
    }

    /// Withdraw from a continuous flow stream
    pub fn withdraw_continuous(env: Env, stream_id: u64, withdrawal_amount: i128) -> i128 {
        let withdrawn = withdraw_from_flow(&env, stream_id, withdrawal_amount).unwrap();
        
        env.events().publish(
            symbol_short!("Withdrawal"),
            (stream_id, withdrawn)
        );
        
        withdrawn
    }

    /// Get the current state of a continuous flow stream
    pub fn get_continuous_flow(env: Env, stream_id: u64) -> Option<ContinuousFlow> {
        env.storage()
            .instance()
            .get::<DataKey, ContinuousFlow>(&DataKey::ContinuousFlow(stream_id))
    }

    /// Get the required buffer amount for a given flow rate
    pub fn get_required_buffer(env: Env, flow_rate_per_second: i128) -> i128 {
        calculate_required_buffer(flow_rate_per_second)
    }

    /// Get current buffer balance for a stream
    pub fn get_buffer_balance(env: Env, stream_id: u64) -> Option<i128> {
        if let Some(flow) = env.storage()
            .instance()
            .get::<DataKey, ContinuousFlow>(&DataKey::ContinuousFlow(stream_id))
        {
            let current_timestamp = env.ledger().timestamp();
            let mut flow_copy = flow.clone();
            update_continuous_flow(&env, &mut flow_copy, current_timestamp).unwrap();
            Some(flow_copy.buffer_balance)
        } else {
            None
        }
    }

    /// Calculate expected depletion time for a continuous flow stream
    pub fn calculate_continuous_depletion(env: Env, stream_id: u64) -> Option<u64> {
        if let Some(mut flow) = env.storage()
            .instance()
            .get::<DataKey, ContinuousFlow>(&DataKey::ContinuousFlow(stream_id))
        {
            if flow.status != StreamStatus::Active || flow.flow_rate_per_second <= 0 {
                return None;
            }
            
            let current_timestamp = env.ledger().timestamp();
            let accumulation = calculate_flow_accumulation(&flow, current_timestamp);
            let remaining_main_balance = flow.accumulated_balance.saturating_sub(accumulation);
            let total_remaining = remaining_main_balance.saturating_add(flow.buffer_balance);
            
            if total_remaining <= 0 {
                return Some(current_timestamp);
            }
            
            let seconds_until_depletion = total_remaining / flow.flow_rate_per_second;
            Some(current_timestamp + seconds_until_depletion as u64)
        } else {
            None
        }
    }

    /// Pause a continuous flow stream (provider only)
    /// Halts time-delta calculation immediately and records paused_at timestamp
    pub fn pause_stream(env: Env, stream_id: u64) {
        let provider = env.invoker(); // Use invoker as provider
        pause_stream(&env, stream_id, &provider).unwrap();
    }

    /// Resume a continuous flow stream (provider only)
    /// Restarts the flow and adjusts timing based on pause duration
    pub fn resume_stream(env: Env, stream_id: u64, flow_rate_per_second: i128) {
        let provider = env.invoker(); // Use invoker as provider
        resume_stream(&env, stream_id, flow_rate_per_second, &provider).unwrap();
    }

    /// Get the current accumulated balance after flow calculation
    pub fn get_continuous_balance(env: Env, stream_id: u64) -> Option<i128> {
        if let Some(mut flow) = env.storage()
            .instance()
            .get::<DataKey, ContinuousFlow>(&DataKey::ContinuousFlow(stream_id))
        {
            let current_timestamp = env.ledger().timestamp();
            let accumulation = calculate_flow_accumulation(&flow, current_timestamp);
            let remaining_main_balance = flow.accumulated_balance.saturating_sub(accumulation);
            
            Some(remaining_main_balance)
        } else {
            None
        }
    }

    /// Sweep dust from depleted continuous flow streams
    /// Only works on streams with < 1 stroop balance that are depleted or paused
    /// Requires admin auth or sufficient gas bounty
    pub fn sweep_dust(env: Env, token_address: Address, max_streams: Option<u64>) -> DustCollectedEvent {
        let caller = env.invoker(); // Use invoker() instead of current_contract_address()
        let is_admin = {
            match env.storage().instance().get::<DataKey, Address>(&DataKey::AdminAddress) {
                Some(admin) => admin == caller,
                None => false,
            }
        };

        if !is_admin {
            let bounty_pool = env
                .storage()
                .instance()
                .get::<DataKey, i128>(&DataKey::GasBountyPool)
                .unwrap_or(0);
            
            if bounty_pool < GAS_BOUNTY_AMOUNT {
                panic_with_error!(&env, ContractError::InsufficientGasBounty);
            }
            
            // Non-admin callers need to authenticate themselves
            caller.require_auth();
        }

        let max_to_process = max_streams.unwrap_or(MAX_SWEEP_STREAMS_PER_CALL).min(MAX_SWEEP_STREAMS_PER_CALL);
        let mut total_dust_swept = 0i128;
        let mut streams_swept = 0u64;
        let current_timestamp = env.ledger().timestamp();

        let total_streams = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::Count)
            .unwrap_or(0);

        for stream_id in 1..=total_streams.min(max_to_process) {
            if let Some(mut flow) = env.storage()
                .instance()
                .get::<DataKey, ContinuousFlow>(&DataKey::ContinuousFlow(stream_id))
            {
                // Update flow calculation first
                let accumulation = calculate_flow_accumulation(&flow, current_timestamp);
                let current_balance = flow.accumulated_balance.saturating_sub(accumulation);
                
                // Check if stream qualifies for dust sweeping
                if (flow.status == StreamStatus::Depleted || flow.status == StreamStatus::Paused) 
                    && is_dust_amount(current_balance) {
                    
                    total_dust_swept = total_dust_swept.saturating_add(current_balance);
                    streams_swept += 1;
                    
                    // Clear the dust from the stream
                    flow.accumulated_balance = 0;
                    flow.last_flow_timestamp = current_timestamp;
                    
                    env.storage()
                        .instance()
                        .set(&DataKey::ContinuousFlow(stream_id), &flow);
                }
            }
        }

        if total_dust_swept == 0 {
            panic_with_error!(&env, ContractError::NoDustToSweep);
        }

        // Transfer dust to protocol treasury (maintenance wallet)
        if let Some(treasury) = env.storage().instance().get::<DataKey, Address>(&DataKey::MaintenanceWallet) {
            transfer_tokens(&env, &token_address, &env.current_contract_address(), &treasury, &total_dust_swept);
        }

        // Update dust aggregation
        update_dust_aggregation(&env, &token_address, total_dust_swept, streams_swept);

        // Pay gas bounty if not admin
        if !is_admin {
            let current_bounty = env
                .storage()
                .instance()
                .get::<DataKey, i128>(&DataKey::GasBountyPool)
                .unwrap_or(0);
            
            let updated_bounty = current_bounty.saturating_sub(GAS_BOUNTY_AMOUNT);
            env.storage()
                .instance()
                .set(&DataKey::GasBountyPool, &updated_bounty);

            transfer_tokens(&env, &token_address, &env.current_contract_address(), &caller, &GAS_BOUNTY_AMOUNT);
        }

        let event = DustCollectedEvent {
            token_address: token_address.clone(),
            total_dust_swept,
            streams_swept,
            timestamp: current_timestamp,
            sweeper_address: caller,
        };

        env.events()
            .publish(symbol_short!("DustCollected"), (
                token_address,
                total_dust_swept,
                streams_swept,
                current_timestamp,
                caller
            ));

        event
    }

    /// Get dust aggregation data for a specific token
    pub fn get_dust_aggregation(env: Env, token_address: Address) -> Option<DustAggregation> {
        env.storage()
            .instance()
            .get::<DataKey, DustAggregation>(&DataKey::DustAggregation(token_address))
    }

    /// Check if a specific stream has dust balance
    pub fn has_dust(env: Env, stream_id: u64) -> bool {
        if let Some(mut flow) = env.storage()
            .instance()
            .get::<DataKey, ContinuousFlow>(&DataKey::ContinuousFlow(stream_id))
        {
            let current_timestamp = env.ledger().timestamp();
            let accumulation = calculate_flow_accumulation(&flow, current_timestamp);
            let current_main_balance = flow.accumulated_balance.saturating_sub(accumulation);
            
            (flow.status == StreamStatus::Depleted || flow.status == StreamStatus::Paused) 
                && is_dust_amount(current_main_balance)
        } else {
            false
        }
    }
}

fn verify_usage_signature(
    env: &Env,
    signed_data: &SignedUsageData,
    meter: &Meter,
) -> Result<(), ContractError> {
    // Check if the provided public key matches the registered meter's public key
    if signed_data.public_key != meter.device_public_key {
        return Err(ContractError::PublicKeyMismatch);
    }

    // Check timestamp is not too old (prevent replay attacks)
    let current_time = env.ledger().timestamp();
    if current_time.saturating_sub(signed_data.timestamp) > MAX_TIMESTAMP_DELAY {
        return Err(ContractError::TimestampTooOld);
    }

    // Create the message that was signed
    let report = UsageReport {
        meter_id: signed_data.meter_id,
        timestamp: signed_data.timestamp,
        watt_hours_consumed: signed_data.watt_hours_consumed,
        units_consumed: signed_data.units_consumed,
    };

    // Verify the signature using Soroban's built-in signature verification.
    // In test builds, we skip the actual crypto check to allow mock signatures.
    #[cfg(not(test))]
    env.crypto().ed25519_verify(
        &signed_data.public_key,
        &report.to_xdr(&env),
        &signed_data.signature,
    );
    Ok(())
}

mod test;
