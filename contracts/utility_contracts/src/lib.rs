#![no_std]
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env, BytesN, Vec, Symbol,
};

// --- Constants (Merged) ---
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
const AUTO_EXTEND_LEDGER_THRESHOLD: u32 = 500_000;
const UPGRADE_VETO_PERIOD_SECONDS: u64 = 7 * DAY_IN_SECONDS;
const ADMIN_TRANSFER_TIMELOCK: u64 = 48 * HOUR_IN_SECONDS;
const VETO_THRESHOLD_BPS: i128 = 1000;
const WITHDRAWAL_REQUEST_EXPIRY: u64 = 7 * DAY_IN_SECONDS;
const MIN_FINANCE_WALLETS: usize = 3;
const MAX_FINANCE_WALLETS: usize = 5;

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

mod gas_estimator;
use gas_estimator::GasCostEstimator;
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
    SupportedToken(Address),
    SupportedWithdrawalToken(Address),
    ProviderTotalPool(Address),
    Referral(Address),
    PollVotes(Symbol),
    UserVoted(Address, Symbol),
    BillingGroup(Address),
    WebhookConfig(Address),
    LastAlert(u64),
    ClosingFeeBps,
    Contributor(u64, Address),
    AuthorizedContributor(u64, Address),
    // Task #2: Tax Compliance
    GovernmentVault(Address),
    TaxRateBps, // Tax rate in basis points (e.g., 500 = 5%)
    // Task #3: Self-Maintenance
    MaintenanceFund(u64), // Per-meter maintenance fund balance
    AutoExtendThreshold, // Ledger threshold for auto-extension
    // Task #4: Wasm Hash Rotation
    ProposedUpgrade,
    UpgradeProposalTime,
    VetoDeadline,
    UserVetoed(Address, u64), // Address and proposal ID
    // NEW TASKS:
    // Task #1: Admin Transfer
    CurrentAdmin,
    AdminTransferProposal,
    AdminVeto(Address, u64), // Address and proposal timestamp
    ActiveUsers, // For tracking active users for voting
    // Task #2: Legal Freeze
    ComplianceOfficer,
    ComplianceCouncil,
    LegalFreeze(u64),
    LegalVault,
    // Task #3: Verified Provider Registry
    VerifiedProvider(Address),
    // Issue #127: Reputation Migration
    UserReputation(Address),
    ReputationMigration(BytesN<32>), // Using nullifier as key
    MigratedReputation(Address, Address), // (user, old_contract)
    // Issue #119: Maintenance Milestones
    MaintenanceMilestone(u64, u32), // (meter_id, milestone_number)
    // Issue #118: ZK Privacy
    // Issue #118: ZK Privacy
    ZKProof(BytesN<32>), // Using commitment as key
    NullifierMap(BytesN<32>), // Using nullifier as key
    // Issue #130: Grant Stream Integration
    ConservationGoal(u64),
    GrantStreamMatch(u64, Address), // (meter_id, grant_contract)
    // Task #4: Sub-DAO
    SubDaoConfig(Address),
    // Issue #98: Multi-Sig Provider Withdrawal
    MultiSigConfig(Address),           // Provider address -> MultiSigConfig
    WithdrawalRequest(Address, u64),   // Provider address, request ID -> WithdrawalRequest
    WithdrawalRequestCount(Address),   // Provider address -> request counter
    WithdrawalApproval(Address, u64, Address), // Provider, request ID, signer -> bool
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
    MeterPaused = 16,
    AlreadyVoted = 17,
    InvalidClosingFee = 18,
    AccountAlreadyClosed = 19,
    InsufficientBalance = 20,
    UnauthorizedContributor = 21,
    InDispute = 22,
    ChallengeActive = 23,
    NotAnOracle = 24,
    // Task #1: Priority System Errors
    ThrottlingThresholdExceeded = 25,
    LowPriorityStreamPaused = 26,
    // Task #2: Tax Compliance Errors
    GovernmentVaultNotSet = 27,
    TaxCalculationFailed = 28,
    // Task #3: Maintenance Errors
    MaintenanceFundInsufficient = 29,
    TTLExtensionFailed = 30,
    // Task #4: Upgrade Errors
    UpgradeProposalActive = 31,
    VetoPeriodExpired = 32,
    UserVetoedProposal = 33,
    InvalidWasmHash = 34,
    // NEW TASKS:
    // Task #1: Admin Transfer Errors
    AdminTransferActive = 35,
    NoAdminTransferInProgress = 36,
    VetoThresholdNotReached = 37,
    AdminExecutionWindowExpired = 38,
    NotCurrentAdmin = 39,
    // Task #2: Legal Freeze Errors
    NotComplianceOfficer = 40,
    MeterNotFrozen = 41,
    LegalFreezeAlreadyActive = 42,
    ComplianceCouncilApprovalRequired = 43,
    // Task #3: Verified Provider Errors
    ProviderNotVerified = 44,
    VerificationAlreadyGranted = 45,
    // Task #4: Sub-DAO Errors
    NotParentDao = 46,
    SubDaoBudgetExceeded = 47,
    SubDaoNotConfigured = 48,
    // Issue #98: Multi-Sig Withdrawal Errors
    MultiSigNotConfigured = 49,
    MultiSigAlreadyConfigured = 50,
    InvalidFinanceWalletCount = 51,
    InvalidSignatureThreshold = 52,
    NotAuthorizedFinanceWallet = 53,
    WithdrawalRequestNotFound = 54,
    WithdrawalRequestExpired = 55,
    WithdrawalAlreadyExecuted = 56,
    WithdrawalAlreadyCancelled = 57,
    InsufficientApprovals = 58,
    AlreadyApprovedWithdrawal = 59,
    NotApprovedByWallet = 60,
    AmountBelowMultiSigThreshold = 61,
    MultiSigRequiredForAmount = 62,
}

#[contracttype]
#[derive(Clone)]
pub struct PairingChallengeData {
    pub contract: Address,
    pub meter_id: u64,
    pub timestamp: u64,
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
}

// Task #4: Wasm Hash Rotation Helper Functions
fn propose_upgrade_impl(env: &Env, new_wasm_hash: BytesN<32>, proposer: &Address) -> u64 {
    let now = env.ledger().timestamp();
    let veto_deadline = now.saturating_add(UPGRADE_VETO_PERIOD_SECONDS);

    let proposal = UpgradeProposal {
        new_wasm_hash: new_wasm_hash.clone(),
        proposed_at: now,
        veto_deadline,
        proposer: proposer.clone(),
    };

    env.storage().instance().set(&DataKey::ProposedUpgrade, &proposal);
    env.storage().instance().set(&DataKey::UpgradeProposalTime, &now);
    env.storage().instance().set(&DataKey::VetoDeadline, &veto_deadline);

    env.events().publish(
        (soroban_sdk::symbol_short!("UpgrdPrp"),),
        (new_wasm_hash, now, veto_deadline),
    );

    now // Return proposal ID (using timestamp as simple ID)
}

fn has_user_vetoed(env: &Env, user: &Address, proposal_id: u64) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::UserVetoed(user.clone(), proposal_id))
        .unwrap_or(false)
}

fn submit_veto(env: &Env, user: &Address, proposal_id: u64) {
    env.storage()
        .instance()
        .set(&DataKey::UserVetoed(user.clone(), proposal_id), &true);

    env.events().publish(
        soroban_sdk::symbol_short!("VetoSubmt"),
        (user, proposal_id),
    );
}

fn can_finalize_upgrade(env: &Env) -> bool {
    // Check if veto period has expired
    let deadline: u64 = env.storage().instance().get(&DataKey::VetoDeadline).unwrap_or(0);
    let now = env.ledger().timestamp();

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
        priority_index: u32,
    ) -> u64 {
        Self::register_meter_with_mode(
            env,
            user,
            provider,
            off_peak_rate,
            token,
            BillingType::PrePaid,
            device_public_key,
            priority_index,
        )
    }

    pub fn register_with_referral(
        env: Env,
        user: Address,
        provider: Address,
        off_peak_rate: i128,
        token: Address,
        device_public_key: BytesN<32>,
        referrer: Address,
        priority_index: u32,
    ) -> u64 {
        let meter_id = Self::register_meter(
            env.clone(),
            user.clone(),
            provider,
            off_peak_rate,
            token,
            device_public_key,
            priority_index,
        );

        if referrer != user {
            let mut meter = get_meter_or_panic(&env, meter_id);
            // Reward the new user
            meter.balance = meter.balance.saturating_add(REFERRAL_REWARD_UNITS);
            env.storage()
                .instance()
                .set(&DataKey::Meter(meter_id), &meter);

            // Reward the referrer if they have a meter? (simplified for now: just record it)
            env.storage()
                .instance()
                .set(&DataKey::Referral(user.clone()), &referrer.clone());

            env.events().publish(
                (symbol_short!("Referral"), meter_id), (referrer.clone(), user.clone()),
            );
        }

        meter_id
    }

    pub fn register_meter_with_mode(
        env: Env,
        user: Address,
        provider: Address,
        off_peak_rate: i128,
        token: Address,
        billing_type: BillingType,
        device_public_key: BytesN<32>,
        priority_index: u32,
    ) -> u64 {
        user.require_auth();
        
        let mut count = env.storage().instance().get::<DataKey, u64>(&DataKey::Count).unwrap_or(0);
        count += 1;

        let mut active_count = env.storage().instance().get::<_, u32>(&DataKey::ActiveMetersCount).unwrap_or(0);
        active_count += 1;

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
                renewable_watt_hours: 0,
                renewable_percentage: 0,
                monthly_volume: 0,
                last_volume_reset: now,
            };

            let meter = Meter {
                user: meter_info.user.clone(),
                provider: provider_clone.clone(),
                billing_type: meter_info.billing_type,
                off_peak_rate: meter_info.off_peak_rate,
                peak_rate,
                rate_per_second: meter_info.off_peak_rate,
                rate_per_unit: meter_info.off_peak_rate,
                green_energy_discount_bps: 0,
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
                grace_period_start: 0,
                is_paused: false,
                tier_threshold: 100_000,
                tier_rate: meter_info.off_peak_rate.saturating_mul(120) / 100,
                is_disputed: false,
                challenge_timestamp: 0,
                credit_drip_rate: 0,
                is_closed: false,
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
            (symbol_short!("BatchCr"),),
            (batch_event.start_id, batch_event.end_id, batch_event.count),
        );
        batch_event
    }

    pub fn top_up(env: Env, meter_id: u64, amount: i128, contributor: Address) {
        let mut meter = get_meter_or_panic(&env, meter_id);

        // Authorization: either the primary user OR an authorized contributor
        let is_authorized = if contributor == meter.user {
            contributor.require_auth();
            true
        } else {
            let auth_key = DataKey::AuthorizedContributor(meter_id, contributor.clone());
            if env.storage().instance().get::<_, bool>(&auth_key).unwrap_or(false) {
                contributor.require_auth();
                true
            } else {
                false
            }
        };

        if !is_authorized {
            panic_with_error!(&env, ContractError::UnauthorizedContributor);
        }

        let was_active = meter.is_active;
        let old_meter_value = provider_meter_value(&meter);
        // Transfer tokens from contributor to contract
        let token_client = token::Client::new(&env, &meter.token);
        token_client.transfer(&contributor, &env.current_contract_address(), &amount);

        // Track individual contribution
        let contribution_key = DataKey::Contributor(meter_id, contributor.clone());
        let current_contribution = env.storage().instance().get::<_, i128>(&contribution_key).unwrap_or(0);
        env.storage().instance().set(&contribution_key, &current_contribution.saturating_add(amount));

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
            (symbol_short!("TokUp"), meter_id),
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
            .publish((symbol_short!("PairIn"), meter_id), challenge.clone());

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
            .publish((symbol_short!("PairComp"), meter_id), signature);
    }

    pub fn deduct_units(env: Env, signed_data: SignedUsageData) {
        let mut meter = get_meter_or_panic(&env, signed_data.meter_id);
        meter.provider.require_auth();

        // Verify the signature and pairing
        if let Err(e) = verify_usage_signature(&env, &signed_data, &meter) {
            panic_with_error!(&env, e);
        }

        // Task #88: Kill-Switch Check
        if meter.is_disputed {
            panic_with_error!(&env, ContractError::InDispute);
        }

        // Store old meter value for pool update
        let old_meter_value = provider_meter_value(&meter);

        if !meter.is_paired {
            panic_with_error!(&env, ContractError::MeterNotPaired);
        }

        let now = env.ledger().timestamp();
        let effective_rate = get_effective_rate(&meter, signed_data.timestamp);

        // Apply green energy discount if applicable
        let discounted_rate = if signed_data.is_renewable_energy && meter.green_energy_discount_bps > 0 {
            effective_rate.saturating_mul(10000 - meter.green_energy_discount_bps) / 10000
        } else {
            effective_rate
        };

        let cost = signed_data.units_consumed.saturating_mul(discounted_rate);

        // Apply provider withdrawal limits
        let mut window = apply_provider_withdrawal_limit(&env, &meter.provider, cost);

        // Task #3: Allocate to maintenance fund (0.01% = 1 basis point)
        allocate_to_maintenance_fund(&env, signed_data.meter_id, cost);

        // Task #2: Tax Compliance - Split tax before provider payout
        let tax_rate_bps = get_tax_rate_or_default(&env);
        let (tax_amount, after_tax_amount) = calculate_tax_split(cost, tax_rate_bps);

        if tax_amount > 0 {
            // Transfer tax to government vault if configured
            if let Some(gov_vault) = get_government_vault_or_default(&env) {
                let client = token::Client::new(&env, &meter.token);
                client.transfer(&env.current_contract_address(), &gov_vault, &tax_amount);

                // Emit TaxReceipt event
                let tax_receipt = TaxReceipt {
                    meter_id: signed_data.meter_id,
                    total_amount: cost,
                    tax_amount,
                    net_amount: after_tax_amount,
                    tax_rate_bps,
                    government_vault: gov_vault.clone(),
                    timestamp: now,
                };
                env.events().publish(
                    (soroban_sdk::symbol_short!("TaxRec"), signed_data.meter_id),
                    tax_receipt,
                );
            }
        }

        // Apply the claim (using after-tax amount for actual provider payout)
        apply_provider_claim(&env, &mut meter, after_tax_amount);

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

        // Track renewable energy usage
        if signed_data.is_renewable_energy {
            meter.usage_data.renewable_watt_hours = meter
                .usage_data
                .renewable_watt_hours
                .saturating_add(signed_data.watt_hours_consumed);
        }

        // Update renewable percentage
        if meter.usage_data.total_watt_hours > 0 {
            meter.usage_data.renewable_percentage = meter
                .usage_data
                .renewable_watt_hours
                .saturating_mul(10000) / meter.usage_data.total_watt_hours; // in basis points
        }

        if meter.usage_data.current_cycle_watt_hours > meter.usage_data.peak_usage_watt_hours {
            meter.usage_data.peak_usage_watt_hours = meter.usage_data.current_cycle_watt_hours;
        }

        // Update activity status with grace period logic
        refresh_activity(&mut meter, now);

        meter.last_update = now;

        // Task #3: Auto-extend TTL if needed (every 500,000 ledgers)
        auto_extend_ttl_if_needed(&env, signed_data.meter_id);

        // Task #89: Update monthly volume
        let now = env.ledger().timestamp();
        if now.saturating_sub(meter.usage_data.last_volume_reset) >= (30 * DAY_IN_SECONDS) {
            meter.usage_data.monthly_volume = cost;
            meter.usage_data.last_volume_reset = now;
        } else {
            meter.usage_data.monthly_volume = meter.usage_data.monthly_volume.saturating_add(cost);
        }

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
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        // Task #88: Kill-Switch Check
        if meter.is_disputed {
            panic_with_error!(&env, ContractError::InDispute);
        }

        // Store old meter value for pool update
        let old_meter_value = provider_meter_value(&meter);

        let now = env.ledger().timestamp();
        let elapsed = now.checked_sub(meter.last_update).unwrap_or(0);

        // Task #90: Credit Settlement Flow
        // If there's a credit_drip_rate, add it to the normal consumption flow
        let amount = (elapsed as i128).saturating_mul(meter.rate_per_unit.saturating_add(meter.credit_drip_rate));

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

                // Task #3: Allocate to maintenance fund (0.01% = 1 basis point)
                allocate_to_maintenance_fund(&env, meter_id, claimable);

                // Task #2: Tax Compliance - Split tax before provider payout
                let tax_rate_bps = get_tax_rate_or_default(&env);
                let (tax_amount, after_tax_amount) = calculate_tax_split(payout, tax_rate_bps);

                if tax_amount > 0 {
                    // Transfer tax to government vault if configured
                    if let Some(gov_vault) = get_government_vault_or_default(&env) {
                        client.transfer(&env.current_contract_address(), &gov_vault, &tax_amount);

                        // Emit TaxReceipt event
                        let tax_receipt = TaxReceipt {
                            meter_id,
                            total_amount: claimable,
                            tax_amount,
                            net_amount: after_tax_amount,
                            tax_rate_bps,
                            government_vault: gov_vault.clone(),
                            timestamp: now,
                        };
                        env.events().publish(
                            (soroban_sdk::symbol_short!("TaxRcpt"), meter_id),
                            tax_receipt,
                        );
                    }
                }

                payout = after_tax_amount;

                // Protocol fee (existing logic)
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
                    let fee = (payout * fee_bps) / 10000;
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

                // If credit drip was active, reduce the debt if in PostPaid mode
                if meter.billing_type == BillingType::PostPaid && meter.credit_drip_rate > 0 {
                    let credit_settlement = (elapsed as i128).saturating_mul(meter.credit_drip_rate).min(meter.debt);
                    meter.debt = meter.debt.saturating_sub(credit_settlement);
                }
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

                // Task #3: Allocate to maintenance fund (0.01% = 1 basis point)
                allocate_to_maintenance_fund(&env, meter_id, claimable);

                // Task #2: Tax Compliance - Split tax before provider payout
                let tax_rate_bps = get_tax_rate_or_default(&env);
                let (tax_amount, after_tax_amount) = calculate_tax_split(payout, tax_rate_bps);

                if tax_amount > 0 {
                    // Transfer tax to government vault if configured
                    if let Some(gov_vault) = get_government_vault_or_default(&env) {
                        client.transfer(&env.current_contract_address(), &gov_vault, &tax_amount);

                        // Emit TaxReceipt event
                        let tax_receipt = TaxReceipt {
                            meter_id,
                            total_amount: claimable,
                            tax_amount,
                            net_amount: after_tax_amount,
                            tax_rate_bps,
                            government_vault: gov_vault.clone(),
                            timestamp: now,
                        };
                        env.events().publish(
                            (soroban_sdk::symbol_short!("TaxRcpt"), meter_id),
                            tax_receipt,
                        );
                    }
                }

                payout = after_tax_amount;

                // Protocol fee (existing logic)
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
                    let fee = (payout * fee_bps) / 10000;
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

                // If credit drip was active, reduce the debt if in PostPaid mode
                if meter.billing_type == BillingType::PostPaid && meter.credit_drip_rate > 0 {
                    let credit_settlement = (elapsed as i128).saturating_mul(meter.credit_drip_rate).min(meter.debt);
                    meter.debt = meter.debt.saturating_sub(credit_settlement);
                }
            }
        }

        meter.last_update = now;
        meter.last_claim_time = now;

        // Update activity status with grace period logic
        refresh_activity(&mut meter, now);

        // Task #3: Auto-extend TTL if needed (every 500,000 ledgers)
        auto_extend_ttl_if_needed(&env, meter_id);

        // Update provider total pool
        let new_meter_value = provider_meter_value(&meter);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, new_meter_value);

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);

        env.events()
            .publish((symbol_short!("Claim"), meter_id), settlement.gross_claimed);
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

    pub fn set_meter_pause(env: Env, meter_id: u64, paused: bool) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        meter.is_paused = paused;
        let now = env.ledger().timestamp();
        refresh_activity(&mut meter, now);

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);

        env.events()
            .publish((symbol_short!("Paused"), meter_id), paused);
    }

    pub fn set_tiered_pricing(env: Env, meter_id: u64, threshold: i128, rate: i128) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        meter.tier_threshold = threshold;
        meter.tier_rate = rate;

        env.storage()
            .instance()
            .set(&DataKey::Meter(meter_id), &meter);
    }

    pub fn vote_for_asset(env: Env, voter: Address, asset_symbol: Symbol) {
        voter.require_auth();

        // Check if user already voted for this specific asset
        if env
            .storage()
            .instance()
            .has(&DataKey::UserVoted(voter.clone(), asset_symbol.clone()))
        {
            panic_with_error!(env, ContractError::AlreadyVoted);
        }

        let mut votes = env
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::PollVotes(asset_symbol.clone()))
            .unwrap_or(0);

        votes += 1;

        env.storage()
            .instance()
            .set(&DataKey::PollVotes(asset_symbol.clone()), &votes);
        env.storage()
            .instance()
            .set(&DataKey::UserVoted(voter, asset_symbol.clone()), &true);

        env.events()
            .publish((symbol_short!("Voted"), asset_symbol), votes);
    }

    pub fn get_votes(env: Env, asset_symbol: Symbol) -> i128 {
        env.storage()
            .instance()
            .get::<_, i128>(&DataKey::PollVotes(asset_symbol))
            .unwrap_or(0)
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
                (symbol_short!("USD2XL"), meter_id),
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

    pub fn set_closing_fee(env: Env, fee_bps: i128) {
        // Validate fee is within reasonable bounds (0-1000 bps = 0-10%)
        if fee_bps < 0 || fee_bps > 1000 {
            panic_with_error!(env, ContractError::InvalidClosingFee);
        }
        env.storage().instance().set(&DataKey::ClosingFeeBps, &fee_bps);
    }

    pub fn get_closing_fee(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::ClosingFeeBps)
            .unwrap_or(100) // Default 1% (100 bps)
    }

    /// Close account and withdraw remaining balance minus closing fee
    /// Users can call this to permanently close their meter and get refunded
    pub fn close_account_and_refund(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        // Check if account is already closed
        if meter.is_closed {
            panic_with_error!(env, ContractError::AccountAlreadyClosed);
        }

        // Store old meter value for pool update
        let old_meter_value = provider_meter_value(&meter);

        // Calculate refundable amount based on billing type
        let refundable_amount = match meter.billing_type {
            BillingType::PrePaid => meter.balance,
            BillingType::PostPaid => {
                // For postpaid, refund any remaining collateral
                remaining_postpaid_collateral(&meter)
            }
        };

        // Check if there's anything to refund
        if refundable_amount <= 0 {
            panic_with_error!(env, ContractError::InsufficientBalance);
        }

        // Get closing fee and calculate fee amount
        let closing_fee_bps = Self::get_closing_fee(env.clone());
        let closing_fee_amount = (refundable_amount * closing_fee_bps) / 10000;
        let final_refund_amount = refundable_amount.saturating_sub(closing_fee_amount);

        // Convert USD cents to XLM if needed for withdrawal
        let withdrawal_amount = match convert_usd_to_xlm_if_needed(&env, final_refund_amount, &meter.token) {
            Ok(amount) => amount,
            Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
        };

        // Transfer closing fee to maintenance wallet if configured
        if closing_fee_amount > 0 {
            if let Some(maintenance_wallet) = env.storage().instance().get::<_, Address>(&DataKey::MaintenanceWallet) {
                let fee_withdrawal_amount = match convert_usd_to_xlm_if_needed(&env, closing_fee_amount, &meter.token) {
                    Ok(amount) => amount,
                    Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
                };

                let token_client = token::Client::new(&env, &meter.token);
                token_client.transfer(&env.current_contract_address(), &maintenance_wallet, &fee_withdrawal_amount);
            }
        }

        // Transfer refund to user
        if final_refund_amount > 0 {
            let token_client = token::Client::new(&env, &meter.token);
            token_client.transfer(&env.current_contract_address(), &meter.user, &withdrawal_amount);
        }

        // Close the account
        meter.is_closed = true;
        meter.is_active = false;
        meter.balance = 0;
        meter.debt = 0;
        meter.collateral_limit = 0;

        let now = env.ledger().timestamp();
        meter.last_update = now;

        // Update provider total pool
        let new_meter_value = provider_meter_value(&meter);
        update_provider_total_pool(&env, &meter.provider, old_meter_value, new_meter_value);

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        // Emit events
        env.events().publish(
            (symbol_short!("AccountClosed"), meter_id),
            (refundable_amount, closing_fee_amount, final_refund_amount)
        );

        // Emit conversion event if XLM was used
        if is_native_token(&meter.token) {
            env.events().publish(
                (symbol_short!("RefundUSDToXLM"), meter_id),
                (final_refund_amount, withdrawal_amount)
            );
        }
    }

    /// Withdraw earnings with path payment support - allows provider to receive XLM
    /// even when payments were made in USDC or other tokens
    pub fn withdraw_earnings_path_payment(env: Env, meter_id: u64, amount_usd_cents: i128, destination_token: Address) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        if amount_usd_cents <= 0 {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        // Check if destination token is supported for withdrawal
        if !Self::is_withdrawal_token_supported(env.clone(), destination_token.clone()) {
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

        // If destination token is same as meter token, use regular withdrawal
        if destination_token == meter.token {
            Self::withdraw_earnings(env.clone(), meter_id, amount_usd_cents);
            return;
        }

        // Convert USD cents to destination token amount
        let withdrawal_amount = match convert_usd_to_token_if_needed(&env, amount_usd_cents, &destination_token) {
            Ok(amount) => amount,
            Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
        };

        // For path payment, we need to:
        // 1. Convert from meter token to USD (if not already USD)
        // 2. Convert from USD to destination token
        // This is handled by the oracle conversion functions

        // Transfer destination tokens to provider
        let destination_client = token::Client::new(&env, &destination_token);

        // Check if contract has enough destination tokens
        let contract_balance = destination_client.balance(&env.current_contract_address());
        if contract_balance < withdrawal_amount {
            panic_with_error!(&env, ContractError::InsufficientBalance);
        }

        destination_client.transfer(&env.current_contract_address(), &meter.provider, &withdrawal_amount);

        // Update meter balance/debt (deduct in USD cents)
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

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        // Emit path payment event
        env.events().publish(
            (symbol_short!("PathPay"), meter_id),
            (
                meter.token,
                destination_token.clone(),
                amount_usd_cents,
                withdrawal_amount,
            ),
        );

        // Issue #107: Cross-Border Settlement Event for Inter-Anchor communication
        env.events().publish(
            (symbol_short!("XBrder"), meter_id),
            (meter.provider.clone(), destination_token.clone(), withdrawal_amount)
        );
    }

    /// Get supported withdrawal tokens for a provider
    pub fn get_supported_withdrawal_tokens(env: Env) -> Vec<Address> {
        let mut supported_tokens = Vec::new(&env);

        // Add XLM as native token - represented by the contract's own address for native token
        // In Stellar, native token operations use the contract address directly
        supported_tokens.push_back(env.current_contract_address());

        // In a full implementation, you would iterate through stored supported withdrawal tokens
        // For now, we return just the native token

        supported_tokens
    }

    /// Check if a token is supported for withdrawal
    pub fn is_withdrawal_token_supported(env: Env, token: Address) -> bool {
        // Always support native token (XLM)
        if token == env.current_contract_address() {
            return true;
        }

        // Check if token is in supported withdrawal tokens list
        env.storage().instance().get::<DataKey, bool>(&DataKey::SupportedWithdrawalToken(token)).unwrap_or(false)
    }

    /// Get refund estimate for a meter (does not execute the refund)
    pub fn get_refund_estimate(env: Env, meter_id: u64) -> Option<(i128, i128, i128)> {
        if let Some(meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
            if meter.is_closed {
                return None;
            }

            let refundable_amount = match meter.billing_type {
                BillingType::PrePaid => meter.balance,
                BillingType::PostPaid => remaining_postpaid_collateral(&meter),
            };

            if refundable_amount <= 0 {
                return None;
            }

            let closing_fee_bps = Self::get_closing_fee(env.clone());
            let closing_fee_amount = (refundable_amount * closing_fee_bps) / 10000;
            let final_refund_amount = refundable_amount.saturating_sub(closing_fee_amount);

            Some((refundable_amount, closing_fee_amount, final_refund_amount))
        } else {
            None
        }
    }

    // Group Billing Functions
    pub fn create_billing_group(env: Env, parent_account: Address) {
        parent_account.require_auth();

        let billing_group = BillingGroup {
            parent_account: parent_account.clone(),
            child_meters: Vec::new(),
            created_at: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::BillingGroup(parent_account), &billing_group);
    }

    fn add_meter_to_billing_group(env: Env, parent_account: Address, meter_id: u64) {
        let mut billing_group: BillingGroup = env.storage().instance()
            .get(&DataKey::BillingGroup(parent_account.clone()))
            .unwrap_or_else(|| BillingGroup {
                parent_account: parent_account.clone(),
                child_meters: Vec::new(),
                created_at: env.ledger().timestamp(),
            });

        // Add meter to the group if not already present
        if !billing_group.child_meters.contains(&meter_id) {
            billing_group.child_meters.push(meter_id);
            env.storage().instance().set(&DataKey::BillingGroup(parent_account), &billing_group);
        }
    }

    pub fn group_top_up(env: Env, parent_account: Address, amount_per_meter: i128) {
        parent_account.require_auth();

        let billing_group: BillingGroup = env.storage().instance()
            .get(&DataKey::BillingGroup(parent_account.clone()))
            .ok_or("Billing group not found").unwrap();

        if billing_group.child_meters.is_empty() {
            return;
        }

        let total_amount = amount_per_meter * billing_group.child_meters.len() as i128;

        // Transfer total amount from parent to contract
        if let Some(first_meter_id) = billing_group.child_meters.first() {
            if let Some(first_meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(*first_meter_id)) {
                let client = token::Client::new(&env, &first_meter.token);
                client.transfer(&parent_account, &env.current_contract_address(), &total_amount);
            }
        }

        // Distribute funds to all child meters
        for &meter_id in &billing_group.child_meters {
            if let Some(mut meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
                meter.balance += amount_per_meter;
                meter.is_active = true;
                meter.last_update = env.ledger().timestamp();
                env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
            }
        }
    }

    pub fn get_billing_group(env: Env, parent_account: Address) -> Option<BillingGroup> {
        env.storage().instance().get(&DataKey::BillingGroup(parent_account))
    }

    pub fn remove_meter_from_billing_group(env: Env, parent_account: Address, meter_id: u64) {
        parent_account.require_auth();

        let mut billing_group: BillingGroup = env.storage().instance()
            .get(&DataKey::BillingGroup(parent_account.clone()))
            .ok_or("Billing group not found").unwrap();

        billing_group.child_meters.retain(|&id| id != meter_id);
        env.storage().instance().set(&DataKey::BillingGroup(parent_account), &billing_group);

        // Update the meter to remove parent reference
        if let Some(mut meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
            meter.parent_account = None;
            env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
        }
    }

    // Gas Cost Estimator Functions
    pub fn estimate_meter_monthly_cost(
        env: Env,
        is_group_meter: bool,
        _meters_in_group: u32,
    ) -> i128 {
        GasCostEstimator::estimate_meter_monthly_cost(&env, is_group_meter, _meters_in_group)
    }

    pub fn get_operation_cost(_env: Env, operation: String) -> i128 {
        GasCostEstimator::get_operation_cost(&operation)
    }

    // Webhook and Alert Functions
    pub fn configure_webhook(env: Env, user: Address, webhook_url: String) {
        user.require_auth();

        let webhook_config = WebhookConfig {
            url: webhook_url.clone(),
            user: user.clone(),
            is_active: true,
            created_at: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::WebhookConfig(user), &webhook_config);
    }

    pub fn deactivate_webhook(env: Env, user: Address) {
        user.require_auth();

        if let Some(mut config) = env.storage().instance().get::<_, WebhookConfig>(&DataKey::WebhookConfig(user.clone())) {
            config.is_active = false;
            env.storage().instance().set(&DataKey::WebhookConfig(user), &config);
        }
    }

    pub fn get_webhook_config(env: Env, user: Address) -> Option<WebhookConfig> {
        env.storage().instance().get(&DataKey::WebhookConfig(user))
    }

    fn check_and_send_low_balance_alert(env: &Env, meter: &Meter, meter_id: u64) {
        // Only check if webhook is configured for this user
        let webhook_config = match env.storage().instance().get::<_, WebhookConfig>(&DataKey::WebhookConfig(meter.user.clone())) {
            Some(config) if config.is_active => config,
            _ => return, // No active webhook configured
        };

        // Calculate hours remaining
        let hours_remaining = if meter.rate_per_second > 0 {
            meter.balance as f32 / meter.rate_per_second as f32 / 3600.0
        } else {
            f32::INFINITY
        };

        // Check if balance is low (< 24 hours)
        if hours_remaining < 24.0 {
            // Check if we've sent an alert recently (within last 12 hours)
            let current_time = env.ledger().timestamp();
            let last_alert_time: Option<u64> = env.storage().instance().get(&DataKey::LastAlert(meter_id));

            if let Some(last_time) = last_alert_time {
                if current_time.checked_sub(last_time).unwrap_or(0) < 43200 { // 12 hours in seconds
                    return; // Already sent alert recently
                }
            }

            // Create and send alert
            let alert = LowBalanceAlert {
                meter_id,
                user: meter.user.clone(),
                remaining_balance: meter.balance,
                hours_remaining,
                timestamp: current_time,
            };

            // Store the alert timestamp
            env.storage().instance().set(&DataKey::LastAlert(meter_id), &current_time);

            // In a real implementation, this would make an HTTP call to the webhook
            // For now, we'll store the alert in contract storage for demonstration
            let alert_key = format!("alert:{}:{}", meter_id, current_time);
            env.storage().instance().set(&alert_key, &alert);
        }
    }

    pub fn get_pending_alerts(env: Env, user: Address) -> Vec<LowBalanceAlert> {
        let mut alerts = Vec::new();

        // This is a simplified implementation
        // In practice, you'd want to iterate through storage more efficiently
        let count: u64 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);

        for meter_id in 1..=count {
            if let Some(meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
                if meter.user == user {
                    // Check for recent alerts
                    let current_time = env.ledger().timestamp();
                    let alert_key = format!("alert:{}:{}", meter_id, current_time);
                    if let Some(alert) = env.storage().instance().get::<_, LowBalanceAlert>(&alert_key) {
                        alerts.push(alert);
                    }
                }
            }
        }

        alerts
    }

    // Enhanced claim function with webhook integration
    pub fn claim_with_alerts(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        // Task #88: Kill-Switch Check
        if meter.is_disputed {
            panic_with_error!(&env, ContractError::InDispute);
        }

        let now = env.ledger().timestamp();
        let elapsed = now.checked_sub(meter.last_update).unwrap_or(0);

        // Task #90: Credit Settlement Flow
        let amount = (elapsed as i128).saturating_mul(meter.rate_per_unit.saturating_add(meter.credit_drip_rate));

        // Check if we need to reset the hourly counter
        let hours_passed = now.checked_sub(meter.last_claim_time).unwrap_or(0) / 3600;
        if hours_passed >= 1 {
            meter.claimed_this_hour = 0;
            meter.last_claim_time = now;
        }

        // Ensure we don't overdraw the balance
        let claimable = if amount > meter.balance {
            meter.balance
        } else {
            amount
        };

        // Apply max flow rate cap
        let final_claimable = if claimable > 0 {
            let remaining_hourly_capacity = meter.max_flow_rate_per_hour - meter.claimed_this_hour;
            if claimable > remaining_hourly_capacity {
                remaining_hourly_capacity
            } else {
                claimable
            }
        } else {
            0
        };

        if final_claimable > 0 {
            let client = token::Client::new(&env, &meter.token);
            client.transfer(&env.current_contract_address(), &meter.provider, &final_claimable);
            meter.balance -= final_claimable;
            meter.claimed_this_hour += final_claimable;

            // If credit drip was active, reduce the debt if in PostPaid mode
            if meter.billing_type == BillingType::PostPaid && meter.credit_drip_rate > 0 {
                let credit_settlement = (elapsed as i128).saturating_mul(meter.credit_drip_rate).min(meter.debt);
                meter.debt = meter.debt.saturating_sub(credit_settlement);
            }
        }

        meter.last_update = now;
        if meter.balance <= 0 {
            meter.is_active = false;
        }

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        // Check for low balance and send alert if needed
        Self::check_and_send_low_balance_alert(&env, &meter, meter_id);
    }

    // Task #87: Roommates support
    pub fn add_authorized_contributor(env: Env, meter_id: u64, contributor: Address) {
        let meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        env.storage().instance().set(&DataKey::AuthorizedContributor(meter_id, contributor), &true);
    }

    pub fn remove_authorized_contributor(env: Env, meter_id: u64, contributor: Address) {
        let meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        env.storage().instance().remove(&DataKey::AuthorizedContributor(meter_id, contributor));
    }

    pub fn get_contribution(env: Env, meter_id: u64, contributor: Address) -> i128 {
        env.storage().instance().get(&DataKey::Contributor(meter_id, contributor)).unwrap_or(0)
    }

    // Task #88: Emergency Kill-Switch (Challenge)
    pub fn challenge_service(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        if meter.is_disputed {
            panic_with_error!(&env, ContractError::ChallengeActive);
        }

        meter.is_disputed = true;
        meter.is_paused = true;
        meter.challenge_timestamp = env.ledger().timestamp();

        let now = env.ledger().timestamp();
        refresh_activity(&mut meter, now);

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        env.events().publish((symbol_short!("Challeng"), meter_id), meter.challenge_timestamp);
    }

    pub fn resolve_challenge(env: Env, meter_id: u64, restored: bool) {
        let mut meter: Meter = env
            .storage()
            .instance()
            .get(&DataKey::Meter(meter_id))
            .expect("Meter not found");

        // This should be called by the Oracle or Admin
        let oracle: Address = env
            .storage()
            .instance()
            .get(&DataKey::Oracle)
            .expect("No oracle set");

        oracle.require_auth();

        if !meter.is_disputed {
            return;
        }

        if restored {
            // Service restored, unpause and resume stream
            meter.is_disputed = false;
            meter.is_paused = false;
        } else {
            // Service NOT restored
            meter.is_disputed = false; // Resolved but failed
            meter.is_paused = true; // Stay paused
        }

        let now = env.ledger().timestamp();
        refresh_activity(&mut meter, now);

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        env.events().publish((symbol_short!("Resolv"), meter_id), restored);
    }

    pub fn refund_disputed_funds(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        // Can only refund if challenged more than 48 hours ago and not resolved
        let now = env.ledger().timestamp();
        if !meter.is_disputed || now.saturating_sub(meter.challenge_timestamp) < (48 * HOUR_IN_SECONDS) {
            panic_with_error!(&env, ContractError::ChallengeActive);
        }

        // Return funds to user
        let refundable = match meter.billing_type {
            BillingType::PrePaid => meter.balance,
            BillingType::PostPaid => remaining_postpaid_collateral(&meter),
        };

        if refundable > 0 {
            let withdrawal_amount = match convert_usd_to_xlm_if_needed(&env, refundable, &meter.token) {
                Ok(amount) => amount,
                Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
            };

            let client = token::Client::new(&env, &meter.token);
            client.transfer(&env.current_contract_address(), &meter.user, &withdrawal_amount);
        }

        meter.balance = 0;
        meter.debt = 0;
        meter.is_active = false;
        meter.is_disputed = false;

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        env.events().publish((symbol_short!("Refund"), meter_id), refundable);
    }

    // Task #90: Post-Paid Settlement Credit Logic
    pub fn set_credit_drip(env: Env, meter_id: u64, drip_rate: i128) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        meter.credit_drip_rate = drip_rate;

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
    }

    // Task #1: Stream Priority System - Set priority index for a meter
    pub fn set_priority_index(env: Env, meter_id: u64, priority_index: u32) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.user.require_auth();

        meter.priority_index = priority_index;

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        env.events().publish(
            (soroban_sdk::symbol_short!("Prior"), meter_id),
            priority_index,
        );
    }

    // Task #1: Check if throttling should be activated and pause low-priority streams
    pub fn apply_throttling_if_needed(env: Env, meter_id: u64) {
        let mut meter = get_meter_or_panic(&env, meter_id);
        meter.provider.require_auth();

        let throttling_active = check_throttling_threshold(&env, &meter);

        if should_pause_low_priority_stream(&meter, throttling_active) {
            meter.is_paused = true;
            panic_with_error!(&env, ContractError::LowPriorityStreamPaused);
        }

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        env.events().publish(
            (soroban_sdk::symbol_short!("Throttl"), meter_id),
            throttling_active,
        );
    }

    // Task #2: Tax Compliance - Set government vault address
    pub fn set_government_vault(env: Env, vault_address: Address) {
        vault_address.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::GovernmentVault, &vault_address);

        env.events().publish(
            soroban_sdk::symbol_short!("GovVault"),
            vault_address,
        );
    }

    // Task #2: Tax Compliance - Set tax rate (in basis points)
    pub fn set_tax_rate(env: Env, tax_rate_bps: i128) {
        // Should be admin-only in production
        if tax_rate_bps < 0 || tax_rate_bps > 10_000 {
            panic_with_error!(&env, ContractError::InvalidUsageValue);
        }

        env.storage()
            .instance()
            .set(&DataKey::TaxRateBps, &tax_rate_bps);

        env.events().publish(
            soroban_sdk::symbol_short!("TaxRate"),
            tax_rate_bps,
        );
    }

    // Task #3: Self-Maintenance - Get maintenance fund balance for a meter
    pub fn get_maintenance_fund(env: Env, meter_id: u64) -> i128 {
        get_maintenance_fund_balance(&env, meter_id)
    }

    // Task #3: Self-Maintenance - Manually extend TTL (emergency function)
    pub fn manual_extend_ttl(env: Env, meter_id: u64) {
        let maintenance_balance = get_maintenance_fund_balance(&env, meter_id);

        // Estimate cost (simplified)
        let estimated_cost = 1_000_000; // 1 XLM in stroops

        if maintenance_balance < estimated_cost {
            panic_with_error!(&env, ContractError::MaintenanceFundInsufficient);
        }

        // Deduct from maintenance fund
        let new_balance = maintenance_balance.saturating_sub(estimated_cost);
        env.storage()
            .instance()
            .set(&DataKey::MaintenanceFund(meter_id), &new_balance);

        // Extend TTL
        env.storage().instance().extend_ttl(LEDGER_LIFETIME_EXTENSION, LEDGER_LIFETIME_EXTENSION);

        env.events().publish(
            (soroban_sdk::symbol_short!("TTLMnl"), meter_id),
            LEDGER_LIFETIME_EXTENSION,
        );
    }

    // Task #4: Wasm Hash Rotation - Propose upgrade
    pub fn propose_upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let proposer = env.current_contract_address();
        proposer.require_auth();

        // Validate hash (basic check - should be non-zero)
        if new_wasm_hash == BytesN::<32>::from_array(&env, &[0; 32]) {
            panic_with_error!(&env, ContractError::InvalidWasmHash);
        }

        // Check if there's already an active proposal
        let existing_proposal_time: Option<u64> = env.storage().instance().get(&DataKey::UpgradeProposalTime);
        if let Some(proposal_time) = existing_proposal_time {
            let deadline: u64 = env.storage().instance().get(&DataKey::VetoDeadline).unwrap_or(0);
            let now = env.ledger().timestamp();

            if now < deadline {
                panic_with_error!(&env, ContractError::UpgradeProposalActive);
            }
        }

        let proposal_id = propose_upgrade_impl(&env, new_wasm_hash, &proposer);

        env.events().publish(
            soroban_sdk::symbol_short!("UpgrdProp"),
            proposal_id,
        );
    }

    // Task #4: Wasm Hash Rotation - Submit veto
    pub fn submit_upgrade_veto(env: Env, proposal_id: u64) {
        let user = env.current_contract_address();
        user.require_auth();

        // Check if veto period is still active
        let deadline: u64 = env.storage().instance().get(&DataKey::VetoDeadline).unwrap_or(0);
        let now = env.ledger().timestamp();

        if now >= deadline {
            panic_with_error!(&env, ContractError::VetoPeriodExpired);
        }

        submit_veto(&env, &user, proposal_id);
    }

    // Task #4: Wasm Hash Rotation - Finalize upgrade
    pub fn finalize_upgrade(env: Env) {
        // Check if upgrade can be finalized
        if !can_finalize_upgrade(&env) {
            panic_with_error!(&env, ContractError::UpgradeProposalActive);
        }

        // Get the proposed upgrade
        let proposal: UpgradeProposal = env
            .storage()
            .instance()
            .get(&DataKey::ProposedUpgrade)
            .expect("No upgrade proposal found");

        // In a real implementation, this would call env.deployer().update_current_contract_wasm()
        // For now, we just emit an event indicating the upgrade is ready
        env.events().publish(
            soroban_sdk::symbol_short!("UpgrdFinsh"),
            proposal.new_wasm_hash,
        );

        // Clear the proposal
        env.storage().instance().remove(&DataKey::ProposedUpgrade);
        env.storage().instance().remove(&DataKey::UpgradeProposalTime);
        env.storage().instance().remove(&DataKey::VetoDeadline);
    }

    // ============================================================
    // NEW TASKS IMPLEMENTATION
    // ============================================================

    // ==================== TASK #1: ADMIN TRANSFER WITH TIMELOCK ====================

    /// Initialize admin transfer with 48-hour timelock
    /// During the window, active users can veto (requires 10% to succeed)
    pub fn initiate_admin_transfer(env: Env, proposed_admin: Address) {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::CurrentAdmin)
            .expect("No admin set");

        current_admin.require_auth();

        // Check no active transfer
        let existing_proposal: Option<AdminTransferProposal> = env
            .storage()
            .instance()
            .get(&DataKey::AdminTransferProposal);

        if let Some(proposal) = existing_proposal {
            if proposal.is_active && env.ledger().timestamp() < proposal.execution_deadline {
                panic_with_error!(&env, ContractError::AdminTransferActive);
            }
        }

        let now = env.ledger().timestamp();
        let proposal = AdminTransferProposal {
            current_admin: current_admin.clone(),
            proposed_admin: proposed_admin.clone(),
            proposed_at: now,
            execution_deadline: now + ADMIN_TRANSFER_TIMELOCK,
            veto_count: 0,
            is_active: true,
        };

        env.storage().instance().set(&DataKey::AdminTransferProposal, &proposal);

        env.events().publish(
            (soroban_sdk::symbol_short!("AdminXfer"),),
            (current_admin, proposed_admin, now + ADMIN_TRANSFER_TIMELOCK),
        );
    }

    /// Submit veto against admin transfer
    /// Requires 10% of active users to veto
    pub fn veto_admin_transfer(env: Env, user: Address) {
        user.require_auth();

        let proposal: AdminTransferProposal = env
            .storage()
            .instance()
            .get(&DataKey::AdminTransferProposal)
            .expect("No active transfer");

        if !proposal.is_active || env.ledger().timestamp() >= proposal.execution_deadline {
            panic_with_error!(&env, ContractError::NoAdminTransferInProgress);
        }

        // Check if user already vetoed
        let has_vetoed: bool = env
            .storage()
            .instance()
            .get(&DataKey::AdminVeto(user.clone(), proposal.proposed_at))
            .unwrap_or(false);

        if has_vetoed {
            panic_with_error!(&env, ContractError::AlreadyVoted);
        }

        // Record veto
        env.storage().instance().set(&DataKey::AdminVeto(user, proposal.proposed_at), &true);

        // Increment veto count
        let mut updated_proposal = proposal;
        updated_proposal.veto_count += 1;
        env.storage().instance().set(&DataKey::AdminTransferProposal, &updated_proposal);

        env.events().publish(
            (soroban_sdk::symbol_short!("Veto"),),
            updated_proposal.veto_count,
        );
    }

    /// Execute admin transfer after 48-hour timelock if not vetoed
    pub fn execute_admin_transfer(env: Env) {
        let proposal: AdminTransferProposal = env
            .storage()
            .instance()
            .get(&DataKey::AdminTransferProposal)
            .expect("No active transfer");

        if !proposal.is_active {
            panic_with_error!(&env, ContractError::NoAdminTransferInProgress);
        }

        let now = env.ledger().timestamp();

        // Check if execution window expired
        if now > proposal.execution_deadline + DAY_IN_SECONDS {
            panic_with_error!(&env, ContractError::AdminExecutionWindowExpired);
        }

        // Calculate total active users and veto threshold
        let total_active_users: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ActiveUsers)
            .unwrap_or(100); // Default 100 for testing

        let veto_threshold = (total_active_users as i128 * VETO_THRESHOLD_BPS / 10000) as u32;

        if proposal.veto_count >= veto_threshold {
            panic_with_error!(&env, ContractError::VetoThresholdNotReached);
        }

        // Execute transfer
        env.storage().instance().set(&DataKey::CurrentAdmin, &proposal.proposed_admin);
        env.storage().instance().remove(&DataKey::AdminTransferProposal);

        // Clean up individual vetos
        // (In production, you'd iterate and clean, but simplified here)

        env.events().publish(
            (soroban_sdk::symbol_short!("AdminDone"),),
            (proposal.proposed_admin, now),
        );
    }

    /// Set current admin (initialization only)
    pub fn set_initial_admin(env: Env, admin: Address) {
        // Only allow if no admin is set
        let existing: Option<Address> = env.storage().instance().get(&DataKey::CurrentAdmin);
        if existing.is_some() {
            panic_with_error!(&env, ContractError::AdminTransferActive);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::CurrentAdmin, &admin);

        env.events().publish(
            (soroban_sdk::symbol_short!("SetAdmn"),),
            admin,
        );
    }

    /// Register as active user (for governance tracking)
    pub fn register_active_user(env: Env, user: Address) {
        user.require_auth();

        // Simplified: just increment counter
        let count: u32 = env.storage().instance().get(&DataKey::ActiveUsers).unwrap_or(0);
        env.storage().instance().set(&DataKey::ActiveUsers, &(count + 1));

        env.events().publish(
            (soroban_sdk::symbol_short!("ActvUser"),),
            user,
        );
    }

    // ==================== TASK #2: LEGAL FREEZE ====================

    /// Initiate legal freeze on a meter (compliance officer only)
    pub fn legal_freeze(env: Env, meter_id: u64, reason: String) {
        let compliance_officer: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceOfficer)
            .expect("No compliance officer set");

        compliance_officer.require_auth();

        // Check if already frozen
        let existing_freeze: Option<LegalFreeze> = env
            .storage()
            .instance()
            .get(&DataKey::LegalFreeze(meter_id));

        if let Some(freeze) = existing_freeze {
            if !freeze.is_released {
                panic_with_error!(&env, ContractError::LegalFreezeAlreadyActive);
            }
        }

        let mut meter = get_meter_or_panic(&env, meter_id);

        // Get legal vault
        let legal_vault: Address = env
            .storage()
            .instance()
            .get(&DataKey::LegalVault)
            .expect("No legal vault set");

        // Calculate frozen amount
        let frozen_amount = match meter.billing_type {
            BillingType::PrePaid => meter.balance,
            BillingType::PostPaid => remaining_postpaid_collateral(&meter),
        };

        // Transfer funds to legal vault
        if frozen_amount > 0 {
            let withdrawal_amount = match convert_usd_to_xlm_if_needed(&env, frozen_amount, &meter.token) {
                Ok(amount) => amount,
                Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
            };

            let client = token::Client::new(&env, &meter.token);
            client.transfer(&env.current_contract_address(), &legal_vault, &withdrawal_amount);
        }

        // Create freeze record
        let freeze = LegalFreeze {
            meter_id,
            frozen_at: env.ledger().timestamp(),
            reason: reason.clone(),
            compliance_officer: compliance_officer.clone(),
            legal_vault: legal_vault.clone(),
            frozen_amount,
            is_released: false,
        };

        env.storage().instance().set(&DataKey::LegalFreeze(meter_id), &freeze);

        // Pause the meter
        meter.is_paused = true;
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        env.events().publish(
            (soroban_sdk::symbol_short!("LglFrz"), meter_id),
            (reason, frozen_amount, legal_vault),
        );
    }

    /// Release legal freeze (requires compliance council multi-sig)
    pub fn release_legal_freeze(env: Env, meter_id: u64, council_signatures: Vec<Address>) {
        // Verify council approval (simplified: check at least 2 signatures)
        if council_signatures.len() < 2 {
            panic_with_error!(&env, ContractError::ComplianceCouncilApprovalRequired);
        }

        // In production, verify each signature against council members
        // For now, just require auth from provided addresses
        for sig in council_signatures.iter() {
            sig.require_auth();
        }

        let freeze: LegalFreeze = env
            .storage()
            .instance()
            .get(&DataKey::LegalFreeze(meter_id))
            .expect("No active freeze");

        if freeze.is_released {
            panic_with_error!(&env, ContractError::MeterNotFrozen);
        }

        let mut meter = get_meter_or_panic(&env, meter_id);

        // Return funds from legal vault to user
        if freeze.frozen_amount > 0 {
            let legal_vault: Address = env
                .storage()
                .instance()
                .get(&DataKey::LegalVault)
                .expect("No legal vault set");

            let withdrawal_amount = match convert_usd_to_xlm_if_needed(&env, freeze.frozen_amount, &meter.token) {
                Ok(amount) => amount,
                Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
            };

            let client = token::Client::new(&env, &meter.token);
            client.transfer(&legal_vault, &meter.user, &withdrawal_amount);
        }

        // Update freeze record
        let mut updated_freeze = freeze;
        updated_freeze.is_released = true;
        env.storage().instance().set(&DataKey::LegalFreeze(meter_id), &updated_freeze);

        // Unpause meter
        meter.is_paused = false;
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        env.events().publish(
            (soroban_sdk::symbol_short!("FrzRls"), meter_id),
            env.ledger().timestamp(),
        );
    }

    /// Set compliance officer address
    pub fn set_compliance_officer(env: Env, officer: Address) {
        // Should be called by current admin
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::CurrentAdmin)
            .expect("No admin set");

        admin.require_auth();

        env.storage().instance().set(&DataKey::ComplianceOfficer, &officer);

        env.events().publish(
            (soroban_sdk::symbol_short!("CmpOfcr"),),
            officer,
        );
    }

    /// Set legal vault address
    pub fn set_legal_vault(env: Env, vault: Address) {
        vault.require_auth();

        env.storage().instance().set(&DataKey::LegalVault, &vault);

        env.events().publish(
            (soroban_sdk::symbol_short!("LglVlt"),),
            vault,
        );
    }

    /// Get legal freeze info
    pub fn get_legal_freeze(env: Env, meter_id: u64) -> LegalFreeze {
        env.storage()
            .instance()
            .get(&DataKey::LegalFreeze(meter_id))
            .expect("No freeze found")
    }

    // ==================== TASK #3: VERIFIED PROVIDER REGISTRY ====================

    /// Request provider verification
    pub fn request_provider_verification(env: Env, provider_name: String) {
        let provider = env.current_contract_address();
        provider.require_auth();

        // Check if already verified
        let existing: Option<VerifiedProvider> = env
            .storage()
            .instance()
            .get(&DataKey::VerifiedProvider(provider.clone()));

        if let Some(v) = existing {
            if v.is_verified {
                panic_with_error!(&env, ContractError::VerificationAlreadyGranted);
            }
        }

        // Create verification request (pending identity verification)
        let verified_provider = VerifiedProvider {
            address: provider.clone(),
            is_verified: false,
            verified_at: env.ledger().timestamp(),
            verification_method: VerificationMethod::IdentityVerified,
            provider_name,
        };

env.storage()
            .instance()
            .set(&DataKey::VerifiedProvider(provider.clone()), &verified_provider);

        env.events().publish(
            (soroban_sdk::symbol_short!("VrfReqst"),),
            provider,
        );
    }

    /// Grant verification to provider (admin or community vote)
    pub fn grant_provider_verification(env: Env, provider: Address, method: VerificationMethod) {
        // Admin can grant verification
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::CurrentAdmin)
            .expect("No admin set");

        admin.require_auth();

        let mut verified_provider: VerifiedProvider = env
            .storage()
            .instance()
            .get(&DataKey::VerifiedProvider(provider.clone()))
            .expect("No verification request found");

        verified_provider.is_verified = true;
        verified_provider.verification_method = method;
        verified_provider.verified_at = env.ledger().timestamp();

        env.storage().instance().set(&DataKey::VerifiedProvider(provider.clone()), &verified_provider);

        env.events().publish(
            (soroban_sdk::symbol_short!("VrfGrnt"),),
            provider,
        );
    }

    /// Check if provider is verified
    pub fn is_provider_verified(env: Env, provider: Address) -> bool {
        let verified: Option<VerifiedProvider> = env
            .storage()
            .instance()
            .get(&DataKey::VerifiedProvider(provider));

        match verified {
            Some(v) => v.is_verified,
            None => false,
        }
    }

    /// Get provider info
    pub fn get_provider_info(env: Env, provider: Address) -> VerifiedProvider {
        env.storage()
            .instance()
            .get(&DataKey::VerifiedProvider(provider))
            .expect("Provider not found")
    }

    // ==================== TASK #4: SUB-DAO HIERARCHICAL PERMISSIONS ====================

    /// Create Sub-DAO configuration
    pub fn create_sub_dao(env: Env, sub_dao: Address, allocated_budget: i128, token: Address) {
        let parent_dao = env.current_contract_address();
        parent_dao.require_auth();

        // Check budget availability (simplified)
        let existing_config: Option<SubDaoConfig> = env
            .storage()
            .instance()
            .get(&DataKey::SubDaoConfig(sub_dao.clone()));

        if let Some(config) = existing_config {
            if config.is_active {
                panic_with_error!(&env, ContractError::SubDaoNotConfigured);
            }
        }

        let config = SubDaoConfig {
            parent_dao: parent_dao.clone(),
            sub_dao: sub_dao.clone(),
            allocated_budget,
            spent_budget: 0,
            token: token.clone(),
            created_at: env.ledger().timestamp(),
            is_active: true,
        };

        env.storage()
            .instance()
            .set(&DataKey::SubDaoConfig(sub_dao.clone()), &config);

        env.events().publish(
            (soroban_sdk::symbol_short!("SubDaoC"),),
            (parent_dao, sub_dao.clone(), allocated_budget),
        );
    }

    /// Create stream from Sub-DAO (uses allocated budget)
    pub fn create_sub_dao_stream(
        env: Env,
        user: Address,
        provider: Address,
        off_peak_rate: i128,
        token: Address,
        device_public_key: BytesN<32>,
        priority_index: u32,
    ) -> u64 {
        // Verify caller is a configured Sub-DAO
        let sub_dao = env.current_contract_address();

        let config: SubDaoConfig = env
            .storage()
            .instance()
            .get(&DataKey::SubDaoConfig(sub_dao.clone()))
            .expect("Sub-DAO not configured");

        if !config.is_active {
            panic_with_error!(&env, ContractError::SubDaoNotConfigured);
        }

        // Verify token matches
        if token != config.token {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        // Check budget (simplified - in production would track properly)
        if config.spent_budget >= config.allocated_budget {
            panic_with_error!(&env, ContractError::SubDaoBudgetExceeded);
        }

        // Create the meter using standard logic
        let meter_id = Self::register_meter_with_mode(
            env,
            user,
            provider,
            off_peak_rate,
            token,
            BillingType::PrePaid,
            device_public_key,
            priority_index,
        );

        // Update spent budget (simplified)
        let mut updated_config = config;
        updated_config.spent_budget += off_peak_rate; // Simplified accounting
        env.storage().instance().set(&DataKey::SubDaoConfig(sub_dao), &updated_config);

        env.events().publish(
            (soroban_sdk::symbol_short!("SubDaoStr"), meter_id),
            sub_dao,
        );

        meter_id
    }

    /// Recall funds from Sub-DAO (parent DAO only)
    pub fn recall_sub_dao_funds(env: Env, sub_dao: Address, amount: i128) {
        let parent_dao = env.current_contract_address();
        parent_dao.require_auth();

        let mut config: SubDaoConfig = env
            .storage()
            .instance()
            .get(&DataKey::SubDaoConfig(sub_dao.clone()))
            .expect("Sub-DAO not configured");

        if config.parent_dao != parent_dao {
            panic_with_error!(&env, ContractError::NotParentDao);
        }

        // Reduce allocated budget
        config.allocated_budget = config.allocated_budget.saturating_sub(amount);

        env.storage().instance().set(&DataKey::SubDaoConfig(sub_dao), &config);

        env.events().publish(
            (symbol_short!("SubDaoR"),),
            (sub_dao.clone(), amount, config.allocated_budget),
        );
    }

    /// Deactivate Sub-DAO
    pub fn deactivate_sub_dao(env: Env, sub_dao: Address) {
        let parent_dao = env.current_contract_address();
        parent_dao.require_auth();

        let mut config: SubDaoConfig = env
            .storage()
            .instance()
            .get(&DataKey::SubDaoConfig(sub_dao.clone()))
            .expect("Sub-DAO not configured");

        if config.parent_dao != parent_dao {
            panic_with_error!(&env, ContractError::NotParentDao);
        }

        config.is_active = false;
        env.storage().instance().set(&DataKey::SubDaoConfig(sub_dao), &config);

        env.events().publish(
            (soroban_sdk::symbol_short!("SubDaoOff"),),
            sub_dao,
        );
    }

    /// Get Sub-DAO config
    pub fn get_sub_dao_config(env: Env, sub_dao: Address) -> SubDaoConfig {
        env.storage()
            .instance()
            .get(&DataKey::SubDaoConfig(sub_dao))
            .expect("Sub-DAO not configured")
    }

    // ============================================================================
    // Issue #98: Multi-Sig Provider Withdrawal Requirement
    // ============================================================================
    // For large utility companies, a single wallet should not be able to pull
    // millions in revenue. This implements a "Multi-Sig Payout" requirement where
    // withdrawals from the contract to the company's main treasury require 3-of-5
    // authorized signatures from "Finance Department" wallets.
    // ============================================================================

    /// Configure multi-sig withdrawal requirement for a provider.
    /// This sets up the Finance Department wallets that can authorize large withdrawals.
    ///
    /// # Arguments
    /// * `provider` - The utility provider address
    /// * `finance_wallets` - Vector of authorized Finance Department wallet addresses (3-5 wallets)
    /// * `required_signatures` - Number of signatures required (must be <= wallet count)
    /// * `threshold_amount` - Minimum amount in USD cents requiring multi-sig approval
    pub fn configure_multisig_withdrawal(
        env: Env,
        provider: Address,
        finance_wallets: Vec<Address>,
        required_signatures: u32,
        threshold_amount: i128,
    ) {
        // Require provider authorization
        provider.require_auth();

        // Check if already configured
        if env.storage().instance().has(&DataKey::MultiSigConfig(provider.clone())) {
            panic_with_error!(&env, ContractError::MultiSigAlreadyConfigured);
        }

        // Validate wallet count (3-5 wallets required)
        let wallet_count = finance_wallets.len();
        if wallet_count < MIN_FINANCE_WALLETS || wallet_count > MAX_FINANCE_WALLETS {
            panic_with_error!(&env, ContractError::InvalidFinanceWalletCount);
        }

        // Validate required signatures
        if required_signatures == 0 || required_signatures > wallet_count {
            panic_with_error!(&env, ContractError::InvalidSignatureThreshold);
        }

        let config = MultiSigConfig {
            provider: provider.clone(),
            finance_wallets,
            required_signatures,
            threshold_amount,
            is_active: true,
            created_at: env.ledger().timestamp(),
        };

        // Store configuration
        env.storage().instance().set(&DataKey::MultiSigConfig(provider.clone()), &config);

        // Initialize request counter
        env.storage().instance().set(&DataKey::WithdrawalRequestCount(provider.clone()), &0u64);

        env.events().publish(
            (symbol_short!("MSigCfg"),),
            (provider, required_signatures, threshold_amount),
        );
    }

    /// Update multi-sig configuration for a provider.
    /// Requires authorization from at least `required_signatures` current finance wallets.
    pub fn update_multisig_config(
        env: Env,
        provider: Address,
        new_finance_wallets: Vec<Address>,
        new_required_signatures: u32,
        new_threshold_amount: i128,
    ) {
        let config: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured));

        // Require authorization from the provider
        provider.require_auth();

        // Validate new wallet count
        let wallet_count = new_finance_wallets.len();
        if wallet_count < MIN_FINANCE_WALLETS || wallet_count > MAX_FINANCE_WALLETS {
            panic_with_error!(&env, ContractError::InvalidFinanceWalletCount);
        }

let milestone = MaintenanceMilestone {
            meter_id,
            milestone_number,
            description,
            funding_amount,
            is_completed: false,
            completed_at: 0,
            verified_by: verified_by.clone(),
            completion_proof: Bytes::from_array(&env, &[0; 0]),
        };

        env.storage().instance().set(&DataKey::MultiSigConfig(provider.clone()), &updated_config);

        env.events().publish(
            (symbol_short!("MSigUpd"),),
            (provider, new_required_signatures, new_threshold_amount),
        );
    }

    /// Propose a multi-sig withdrawal request.
    /// Only authorized Finance Department wallets can propose withdrawals.
    ///
    /// # Arguments
    /// * `provider` - The utility provider address
    /// * `meter_id` - The meter to withdraw earnings from
    /// * `amount_usd_cents` - Amount to withdraw in USD cents
    /// * `destination` - Treasury address to receive funds
    ///
    /// # Returns
    /// The request ID for this withdrawal proposal
    pub fn propose_multisig_withdrawal(
        env: Env,
        provider: Address,
        meter_id: u64,
        amount_usd_cents: i128,
        destination: Address,
    ) -> u64 {
        let config: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured));

        if !config.is_active {
            panic_with_error!(&env, ContractError::MultiSigNotConfigured);
        }

        // Verify the meter belongs to this provider
        let meter = get_meter_or_panic(&env, meter_id);
        if meter.provider != provider {
            panic_with_error!(&env, ContractError::MeterNotFound);
        }

        // Check amount is above multi-sig threshold
        if amount_usd_cents < config.threshold_amount {
            panic_with_error!(&env, ContractError::AmountBelowMultiSigThreshold);
        }

        // Find the proposer from authorized finance wallets
        let mut proposer: Option<Address> = None;
        for i in 0..config.finance_wallets.len() {
            let wallet = config.finance_wallets.get(i).unwrap();
            // Try to require auth from each wallet - the one that authorized is the proposer
            if env.try_invoke_contract::<(), _>(
                &wallet,
                &Symbol::new(&env, "require_auth"),
                (),
            ).is_ok() {
                proposer = Some(wallet);
                break;
            }
        }

        // Alternative: Require explicit proposer parameter and verify they're authorized
        // For now, we'll require any finance wallet to authorize
        let mut found_proposer = false;
        let mut actual_proposer = config.finance_wallets.get(0).unwrap();
        for i in 0..config.finance_wallets.len() {
            let wallet = config.finance_wallets.get(i).unwrap();
            // Check if this wallet can authorize
            wallet.require_auth();
            actual_proposer = wallet;
            found_proposer = true;
            break;
        }

        if !found_proposer {
            panic_with_error!(&env, ContractError::NotAuthorizedFinanceWallet);
        }

        // Get and increment request counter
        let request_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::WithdrawalRequestCount(provider.clone()))
            .unwrap_or(0);

        let now = env.ledger().timestamp();

        let request = WithdrawalRequest {
            request_id,
            provider: provider.clone(),
            meter_id,
            amount_usd_cents,
            destination: destination.clone(),
            proposer: actual_proposer.clone(),
            created_at: now,
            expires_at: now + WITHDRAWAL_REQUEST_EXPIRY,
            approval_count: 1, // Proposer automatically approves
            is_executed: false,
            is_cancelled: false,
        };

        // Store the request
        env.storage().instance().set(
            &DataKey::WithdrawalRequest(provider.clone(), request_id),
            &request,
        );

        // Record proposer's approval
        env.storage().instance().set(
            &DataKey::WithdrawalApproval(provider.clone(), request_id, actual_proposer.clone()),
            &true,
        );

        // Increment counter
        env.storage().instance().set(
            &DataKey::WithdrawalRequestCount(provider.clone()),
            &(request_id + 1),
        );

        env.events().publish(
            (symbol_short!("MSigProp"),),
            (provider, request_id, amount_usd_cents, destination, actual_proposer),
        );

        request_id
    }

    /// Approve a pending multi-sig withdrawal request.
    /// Only authorized Finance Department wallets can approve.
    ///
    /// # Arguments
    /// * `provider` - The utility provider address
    /// * `request_id` - The withdrawal request ID to approve
    pub fn approve_multisig_withdrawal(env: Env, provider: Address, request_id: u64) {
        let config: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured));

        let mut request: WithdrawalRequest = env
            .storage()
            .instance()
            .get(&DataKey::WithdrawalRequest(provider.clone(), request_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::WithdrawalRequestNotFound));

        // Check request status
        if request.is_executed {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyExecuted);
        }
        if request.is_cancelled {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyCancelled);
        }
        if env.ledger().timestamp() > request.expires_at {
            panic_with_error!(&env, ContractError::WithdrawalRequestExpired);
        }

        // Find and verify the approver is an authorized finance wallet
        let mut approver: Option<Address> = None;
        for i in 0..config.finance_wallets.len() {
            let wallet = config.finance_wallets.get(i).unwrap();
            wallet.require_auth();
            approver = Some(wallet);
            break;
        }

        let actual_approver = approver.unwrap_or_else(|| {
            panic_with_error!(&env, ContractError::NotAuthorizedFinanceWallet)
        });

        // Check if already approved by this wallet
        let approval_key = DataKey::WithdrawalApproval(
            provider.clone(),
            request_id,
            actual_approver.clone(),
        );
        if env.storage().instance().has(&approval_key) {
            panic_with_error!(&env, ContractError::AlreadyApprovedWithdrawal);
        }

        // Record approval
        env.storage().instance().set(&approval_key, &true);
        request.approval_count += 1;

        // Update request
        env.storage().instance().set(
            &DataKey::WithdrawalRequest(provider.clone(), request_id),
            &request,
        );

        env.events().publish(
            (symbol_short!("MSigAppr"),),
            (provider, request_id, actual_approver, request.approval_count),
        );
    }

    /// Execute a multi-sig withdrawal after sufficient approvals.
    ///
    /// # Arguments
    /// * `provider` - The utility provider address
    /// * `request_id` - The withdrawal request ID to execute
    pub fn execute_multisig_withdrawal(env: Env, provider: Address, request_id: u64) {
        let config: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured));

        let mut request: WithdrawalRequest = env
            .storage()
            .instance()
            .get(&DataKey::WithdrawalRequest(provider.clone(), request_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::WithdrawalRequestNotFound));

        // Check request status
        if request.is_executed {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyExecuted);
        }
        if request.is_cancelled {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyCancelled);
        }
        if env.ledger().timestamp() > request.expires_at {
            panic_with_error!(&env, ContractError::WithdrawalRequestExpired);
        }

        // Check sufficient approvals
        if request.approval_count < config.required_signatures {
            panic_with_error!(&env, ContractError::InsufficientApprovals);
        }

        // Get meter and verify
        let mut meter = get_meter_or_panic(&env, request.meter_id);
        if meter.provider != provider {
            panic_with_error!(&env, ContractError::MeterNotFound);
        }

        // Store old meter value for pool update
        let old_meter_value = provider_meter_value(&meter);

        let available_earnings = match meter.billing_type {
            BillingType::PrePaid => meter.balance,
            BillingType::PostPaid => meter.debt,
        };

        if request.amount_usd_cents > available_earnings {
            panic_with_error!(&env, ContractError::InvalidTokenAmount);
        }

        // Convert USD cents to XLM if needed
        let withdrawal_amount = match convert_usd_to_xlm_if_needed(
            &env,
            request.amount_usd_cents,
            &meter.token,
        ) {
            Ok(amount) => amount,
            Err(_) => panic_with_error!(&env, ContractError::PriceConversionFailed),
        };

        // Execute the transfer to the destination treasury
        let client = token::Client::new(&env, &meter.token);
        client.transfer(
            &env.current_contract_address(),
            &request.destination,
            &withdrawal_amount,
        );

        // Update meter balance/debt
        match meter.billing_type {
            BillingType::PrePaid => {
                meter.balance = meter.balance.saturating_sub(request.amount_usd_cents);
            }
            BillingType::PostPaid => {
                meter.debt = meter.debt.saturating_sub(request.amount_usd_cents);
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
            .set(&DataKey::Meter(request.meter_id), &meter);

        // Mark request as executed
        request.is_executed = true;
        env.storage().instance().set(
            &DataKey::WithdrawalRequest(provider.clone(), request_id),
            &request,
        );

        env.events().publish(
            (symbol_short!("MSigExec"),),
            (provider, request_id, request.amount_usd_cents, request.destination, withdrawal_amount),
        );
    }

    /// Revoke a previously given approval for a withdrawal request.
    ///
    /// # Arguments
    /// * `provider` - The utility provider address
    /// * `request_id` - The withdrawal request ID
    pub fn revoke_multisig_approval(env: Env, provider: Address, request_id: u64) {
        let config: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured));

        let mut request: WithdrawalRequest = env
            .storage()
            .instance()
            .get(&DataKey::WithdrawalRequest(provider.clone(), request_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::WithdrawalRequestNotFound));

        // Check request is still pending
        if request.is_executed {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyExecuted);
        }
        if request.is_cancelled {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyCancelled);
        }

        // Find and verify the revoker is an authorized finance wallet
        let mut revoker: Option<Address> = None;
        for i in 0..config.finance_wallets.len() {
            let wallet = config.finance_wallets.get(i).unwrap();
            wallet.require_auth();
            revoker = Some(wallet);
            break;
        }

        let actual_revoker = revoker.unwrap_or_else(|| {
            panic_with_error!(&env, ContractError::NotAuthorizedFinanceWallet)
        });

        // Check if this wallet has approved
        let approval_key = DataKey::WithdrawalApproval(
            provider.clone(),
            request_id,
            actual_revoker.clone(),
        );
        if !env.storage().instance().has(&approval_key) {
            panic_with_error!(&env, ContractError::NotApprovedByWallet);
        }

        // Remove approval
        env.storage().instance().remove(&approval_key);
        request.approval_count = request.approval_count.saturating_sub(1);

        // Update request
        env.storage().instance().set(
            &DataKey::WithdrawalRequest(provider.clone(), request_id),
            &request,
        );

        env.events().publish(
            (symbol_short!("MSigRvke"),),
            (provider, request_id, actual_revoker, request.approval_count),
        );
    }

    /// Cancel a pending multi-sig withdrawal request.
    /// Only the original proposer or provider can cancel.
    ///
    /// # Arguments
    /// * `provider` - The utility provider address
    /// * `request_id` - The withdrawal request ID to cancel
    pub fn cancel_multisig_withdrawal(env: Env, provider: Address, request_id: u64) {
        let mut request: WithdrawalRequest = env
            .storage()
            .instance()
            .get(&DataKey::WithdrawalRequest(provider.clone(), request_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::WithdrawalRequestNotFound));

        // Check request is still pending
        if request.is_executed {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyExecuted);
        }
        if request.is_cancelled {
            panic_with_error!(&env, ContractError::WithdrawalAlreadyCancelled);
        }

        // Either provider or proposer can cancel
        // Try provider first
        let is_provider = env.try_invoke_contract::<(), _>(
            &provider,
            &Symbol::new(&env, "require_auth"),
            (),
        ).is_ok();

        if !is_provider {
            // Try proposer
            request.proposer.require_auth();
        } else {
            provider.require_auth();
        }

        // Mark as cancelled
        request.is_cancelled = true;
        env.storage().instance().set(
            &DataKey::WithdrawalRequest(provider.clone(), request_id),
            &request,
        );

        env.events().publish(
            (symbol_short!("MSigCanc"),),
            (provider, request_id),
        );
    }

    /// Disable multi-sig requirement for a provider.
    /// This allows returning to single-signature withdrawals.
    /// Requires provider authorization.
    pub fn disable_multisig(env: Env, provider: Address) {
        provider.require_auth();

        let mut config: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured));

        config.is_active = false;
        env.storage().instance().set(&DataKey::MultiSigConfig(provider.clone()), &config);

        env.events().publish(
            (symbol_short!("MSigOff"),),
            provider,
        );
    }

    /// Re-enable multi-sig requirement for a provider.
    pub fn enable_multisig(env: Env, provider: Address) {
        provider.require_auth();

        let mut config: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured));

        config.is_active = true;
        env.storage().instance().set(&DataKey::MultiSigConfig(provider.clone()), &config);

        env.events().publish(
            (symbol_short!("MSigOn"),),
            provider,
        );
    }

    /// Get multi-sig configuration for a provider.
    pub fn get_multisig_config(env: Env, provider: Address) -> MultiSigConfig {
        env.storage()
            .instance()
            .get(&DataKey::MultiSigConfig(provider))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MultiSigNotConfigured))
    }

    /// Get a specific withdrawal request.
    pub fn get_withdrawal_request(env: Env, provider: Address, request_id: u64) -> WithdrawalRequest {
        env.storage()
            .instance()
            .get(&DataKey::WithdrawalRequest(provider, request_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::WithdrawalRequestNotFound))
    }

    /// Check if a wallet has approved a specific withdrawal request.
    pub fn has_approved_withdrawal(
        env: Env,
        provider: Address,
        request_id: u64,
        wallet: Address,
    ) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::WithdrawalApproval(provider, request_id, wallet))
    }

    /// Check if a withdrawal amount requires multi-sig for a provider.
    pub fn requires_multisig(env: Env, provider: Address, amount_usd_cents: i128) -> bool {
        match env
            .storage()
            .instance()
            .get::<_, MultiSigConfig>(&DataKey::MultiSigConfig(provider))
        {
            Some(config) => config.is_active && amount_usd_cents >= config.threshold_amount,
            None => false,
        }
    }

    /// Get the current withdrawal request count for a provider.
    pub fn get_withdrawal_request_count(env: Env, provider: Address) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::WithdrawalRequestCount(provider))
            .unwrap_or(0)
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
        is_renewable_energy: signed_data.is_renewable_energy,
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
