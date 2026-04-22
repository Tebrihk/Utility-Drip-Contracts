use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Map, Vec, BytesN, Symbol, symbol_short};

use crate::{
    BillingType, ContractError, DataKey, Meter, UsageData, 
    get_effective_rate, remaining_postpaid_collateral, XLM_PRECISION
};

// Insurance Pool Constants
const INSURANCE_POOL_FEE_BPS: i128 = 50; // 0.5% of each claim goes to insurance pool
const MIN_PREMIUM_PAYMENT: i128 = 100 * XLM_PRECISION; // 100 XLM minimum premium
const MAX_PREMIUM_PAYMENT: i128 = 10000 * XLM_PRECISION; // 10,000 XLM maximum premium
const VOTING_PERIOD_SECONDS: u64 = 7 * 24 * 60 * 60; // 7 days
const QUORUM_THRESHOLD_BPS: i128 = 2000; // 20% of pool members must vote
const APPROVAL_THRESHOLD_BPS: i128 = 5100; // 51% approval required
const CLAIM_COOLDOWN_SECONDS: u64 = 30 * 24 * 60 * 60; // 30 days between claims
const MAX_CLAIM_AMOUNT_BPS: i128 = 1000; // Max 10% of pool per claim
const RISK_ASSESSMENT_PERIOD: u64 = 90 * 24 * 60 * 60; // 90 days for risk assessment

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurancePoolMember {
    pub user: Address,
    pub premium_paid: i128,
    pub join_timestamp: u64,
    pub last_claim_timestamp: u64,
    pub claim_count: u32,
    pub risk_score: u32, // 0-1000, lower is better
    pub voting_power: i128, // Based on premium paid and tenure
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurancePool {
    pub total_funds: i128,
    pub total_members: u32,
    pub total_voting_power: i128,
    pub created_at: u64,
    pub governance_admin: Address,
    pub base_premium_rate_bps: i128, // Base premium as % of monthly usage
    pub risk_multiplier_max: i128, // Maximum risk multiplier (e.g., 300 = 3x)
    pub is_active: bool,
    pub emergency_pause: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceProposal {
    pub proposal_id: u64,
    pub proposer: Address,
    pub proposal_type: ProposalType,
    pub description: Symbol,
    pub new_value: i128,
    pub created_at: u64,
    pub voting_deadline: u64,
    pub votes_for: i128,
    pub votes_against: i128,
    pub total_votes: i128,
    pub is_executed: bool,
    pub is_cancelled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalType {
    ChangePremiumRate,
    ChangeRiskMultiplier,
    ChangeMaxClaimAmount,
    AddMember,
    RemoveMember,
    EmergencyPause,
    ChangeGovernanceAdmin,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsuranceClaim {
    pub claim_id: u64,
    pub claimant: Address,
    pub meter_id: u64,
    pub requested_amount: i128,
    pub reason: Symbol,
    pub created_at: u64,
    pub auto_approved: bool,
    pub is_processed: bool,
    pub approved_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskAssessment {
    pub user: Address,
    pub payment_history_score: u32, // 0-250 points
    pub usage_stability_score: u32, // 0-250 points  
    pub device_security_score: u32, // 0-250 points
    pub tenure_score: u32, // 0-250 points
    pub total_score: u32, // Sum of above, 0-1000
    pub last_updated: u64,
}

impl InsurancePoolMember {
    pub fn calculate_voting_power(&self, now: u64) -> i128 {
        let tenure_months = (now.saturating_sub(self.join_timestamp)) / (30 * 24 * 60 * 60);
        let tenure_bonus = (tenure_months as i128).min(12) * 10; // Max 120 bonus points for 1 year
        
        let base_power = self.premium_paid / XLM_PRECISION; // 1 voting power per XLM
        let risk_penalty = (self.risk_score as i128) / 10; // Reduce power based on risk
        
        (base_power + tenure_bonus).saturating_sub(risk_penalty).max(1)
    }
}

impl RiskAssessment {
    pub fn calculate_premium_multiplier(&self) -> i128 {
        // Convert 0-1000 score to 0.5x - 3.0x multiplier
        // Lower score = lower multiplier (better risk)
        let base_multiplier = 50; // 0.5x in basis points (50/100 = 0.5)
        let max_additional = 250; // Additional 2.5x possible (total 3.0x)
        
        let risk_factor = (self.total_score as i128 * max_additional) / 1000;
        base_multiplier + risk_factor
    }
}

// Storage key extensions for insurance pool
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InsuranceDataKey {
    Pool,
    Member(Address),
    Proposal(u64),
    Vote(Address, u64), // user, proposal_id
    Claim(u64),
    RiskAssessment(Address),
    NextProposalId,
    NextClaimId,
    ProposalCount,
    ClaimCount,
}

pub fn get_insurance_pool(env: &Env) -> Result<InsurancePool, ContractError> {
    env.storage()
        .instance()
        .get(&DataKey::InsurancePool)
        .ok_or(ContractError::InsurancePoolNotFound)
}

pub fn get_pool_member(env: &Env, user: &Address) -> Result<InsurancePoolMember, ContractError> {
    env.storage()
        .instance()
        .get(&DataKey::InsurancePoolMember(user.clone()))
        .ok_or(ContractError::NotPoolMember)
}

pub fn calculate_risk_score(env: &Env, user: &Address, meter_id: u64) -> u32 {
    let meter: Meter = env.storage()
        .instance()
        .get(&DataKey::Meter(meter_id))
        .unwrap_or_else(|| panic!("Meter not found"));
    
    let now = env.ledger().timestamp();
    
    // Payment history score (0-250)
    let payment_score = if meter.billing_type == BillingType::PrePaid {
        // For prepaid, check balance maintenance
        if meter.balance > meter.rate_per_unit * 86400 { // 1 day buffer
            250
        } else if meter.balance > meter.rate_per_unit * 3600 { // 1 hour buffer  
            150
        } else {
            50
        }
    } else {
        // For postpaid, check debt levels
        let collateral_ratio = if meter.collateral_limit > 0 {
            (remaining_postpaid_collateral(&meter) * 100) / meter.collateral_limit
        } else {
            0
        };
        
        if collateral_ratio > 80 { 250 }
        else if collateral_ratio > 50 { 150 }
        else { 50 }
    };
    
    // Usage stability score (0-250)
    let usage_score = if meter.usage_data.current_cycle_watt_hours > 0 {
        let peak_ratio = (meter.usage_data.peak_usage_watt_hours * 100) 
            / meter.usage_data.current_cycle_watt_hours;
        
        if peak_ratio < 150 { 250 } // Stable usage
        else if peak_ratio < 300 { 150 } // Moderate spikes
        else { 50 } // High volatility
    } else {
        100 // No usage data
    };
    
    // Device security score (0-250)
    let security_score = if meter.is_paired && meter.device_public_key.len() == 32 {
        let heartbeat_age = now.saturating_sub(meter.heartbeat);
        if heartbeat_age < 3600 { 250 } // Recent heartbeat
        else if heartbeat_age < 86400 { 150 } // Daily heartbeat
        else { 50 } // Stale heartbeat
    } else {
        25 // Not properly paired
    };
    
    // Tenure score (0-250) - would need meter creation timestamp
    let tenure_score = 200; // Placeholder - would calculate based on account age
    
    payment_score + usage_score + security_score + tenure_score
}

pub fn calculate_premium_amount(
    env: &Env, 
    user: &Address, 
    meter_id: u64
) -> Result<i128, ContractError> {
    let pool = get_insurance_pool(env)?;
    let meter: Meter = env.storage()
        .instance()
        .get(&DataKey::Meter(meter_id))
        .ok_or(ContractError::MeterNotFound)?;
    
    // Base premium calculation
    let monthly_usage_value = meter.usage_data.monthly_volume;
    let base_premium = (monthly_usage_value * pool.base_premium_rate_bps) / 10000;
    
    // Risk assessment
    let risk_score = calculate_risk_score(env, user, meter_id);
    let risk_assessment = RiskAssessment {
        user: user.clone(),
        payment_history_score: risk_score / 4, // Simplified distribution
        usage_stability_score: risk_score / 4,
        device_security_score: risk_score / 4,
        tenure_score: risk_score / 4,
        total_score: risk_score,
        last_updated: env.ledger().timestamp(),
    };
    
    let risk_multiplier = risk_assessment.calculate_premium_multiplier();
    let adjusted_premium = (base_premium * risk_multiplier) / 100;
    
    // Ensure within bounds
    let final_premium = adjusted_premium
        .max(MIN_PREMIUM_PAYMENT)
        .min(MAX_PREMIUM_PAYMENT);
    
    // Store risk assessment
    env.storage().instance().set(
        &DataKey::InsuranceRiskAssessment(user.clone()),
        &risk_assessment
    );
    
    Ok(final_premium)
}
// Insurance Pool Implementation Functions

pub fn create_insurance_pool(
    env: &Env,
    governance_admin: Address,
    base_premium_rate_bps: i128,
) -> Result<(), ContractError> {
    governance_admin.require_auth();
    
    if env.storage().instance().has(&DataKey::InsurancePool) {
        return Err(ContractError::InsurancePoolAlreadyExists);
    }
    
    let pool = InsurancePool {
        total_funds: 0,
        total_members: 0,
        total_voting_power: 0,
        created_at: env.ledger().timestamp(),
        governance_admin,
        base_premium_rate_bps: base_premium_rate_bps.max(10).min(1000), // 0.1% - 10%
        risk_multiplier_max: 300, // 3x maximum
        is_active: true,
        emergency_pause: false,
    };
    
    env.storage().instance().set(&DataKey::InsurancePool, &pool);
    env.storage().instance().set(&DataKey::InsuranceNextProposalId, &1u64);
    env.storage().instance().set(&DataKey::InsuranceNextClaimId, &1u64);
    
    env.events().publish(
        (symbol_short!("PoolInit"), &pool.governance_admin),
        &pool.base_premium_rate_bps
    );
    
    Ok(())
}

pub fn join_insurance_pool(
    env: &Env,
    user: Address,
    meter_id: u64,
    premium_amount: i128,
) -> Result<(), ContractError> {
    user.require_auth();
    
    let mut pool = get_insurance_pool(env)?;
    if !pool.is_active || pool.emergency_pause {
        return Err(ContractError::InsurancePoolInactive);
    }
    
    // Check if already a member
    if env.storage().instance().has(&DataKey::InsurancePoolMember(user.clone())) {
        return Err(ContractError::AlreadyPoolMember);
    }
    
    // Validate meter ownership
    let meter: Meter = env.storage()
        .instance()
        .get(&DataKey::Meter(meter_id))
        .ok_or(ContractError::MeterNotFound)?;
    
    if meter.user != user {
        return Err(ContractError::Unauthorized);
    }
    
    // Calculate required premium
    let required_premium = calculate_premium_amount(env, &user, meter_id)?;
    if premium_amount < required_premium {
        return Err(ContractError::InsufficientPremium);
    }
    
    let now = env.ledger().timestamp();
    let risk_score = calculate_risk_score(env, &user, meter_id);
    
    let member = InsurancePoolMember {
        user: user.clone(),
        premium_paid: premium_amount,
        join_timestamp: now,
        last_claim_timestamp: 0,
        claim_count: 0,
        risk_score,
        voting_power: 0, // Will be calculated dynamically
        is_active: true,
    };
    
    // Update pool totals
    pool.total_funds = pool.total_funds.saturating_add(premium_amount);
    pool.total_members = pool.total_members.saturating_add(1);
    pool.total_voting_power = pool.total_voting_power.saturating_add(
        member.calculate_voting_power(now)
    );
    
    // Store updates
    env.storage().instance().set(&DataKey::InsurancePool, &pool);
    env.storage().instance().set(&DataKey::InsurancePoolMember(user.clone()), &member);
    
    env.events().publish(
        (symbol_short!("PoolJoin"), &user),
        (meter_id, premium_amount, risk_score)
    );
    
    Ok(())
}

pub fn submit_insurance_claim(
    env: &Env,
    claimant: Address,
    meter_id: u64,
    requested_amount: i128,
    reason: Symbol,
) -> Result<u64, ContractError> {
    claimant.require_auth();
    
    let pool = get_insurance_pool(env)?;
    if !pool.is_active || pool.emergency_pause {
        return Err(ContractError::InsurancePoolInactive);
    }
    
    let member = get_pool_member(env, &claimant)?;
    if !member.is_active {
        return Err(ContractError::MemberInactive);
    }
    
    let now = env.ledger().timestamp();
    
    // Check cooldown period
    if member.last_claim_timestamp > 0 {
        let time_since_last_claim = now.saturating_sub(member.last_claim_timestamp);
        if time_since_last_claim < CLAIM_COOLDOWN_SECONDS {
            return Err(ContractError::ClaimCooldownActive);
        }
    }
    
    // Validate claim amount
    let max_claim = (pool.total_funds * MAX_CLAIM_AMOUNT_BPS) / 10000;
    if requested_amount > max_claim {
        return Err(ContractError::ClaimAmountTooHigh);
    }
    
    // Get next claim ID
    let claim_id: u64 = env.storage()
        .instance()
        .get(&DataKey::InsuranceNextClaimId)
        .unwrap_or(1);
    
    // Auto-approve small claims from low-risk members
    let auto_approve_threshold = pool.total_funds / 100; // 1% of pool
    let auto_approved = requested_amount <= auto_approve_threshold && member.risk_score <= 300;
    
    let claim = InsuranceClaim {
        claim_id,
        claimant: claimant.clone(),
        meter_id,
        requested_amount,
        reason,
        created_at: now,
        auto_approved,
        is_processed: auto_approved,
        approved_amount: if auto_approved { requested_amount } else { 0 },
    };
    
    env.storage().instance().set(&DataKey::InsuranceClaim(claim_id), &claim);
    env.storage().instance().set(&DataKey::InsuranceNextClaimId, &(claim_id + 1));
    
    if auto_approved {
        process_approved_claim(env, claim_id)?;
    }
    
    env.events().publish(
        (symbol_short!("ClaimSub"), &claimant),
        (claim_id, meter_id, requested_amount, auto_approved)
    );
    
    Ok(claim_id)
}

pub fn process_approved_claim(env: &Env, claim_id: u64) -> Result<(), ContractError> {
    let mut claim: InsuranceClaim = env.storage()
        .instance()
        .get(&DataKey::InsuranceClaim(claim_id))
        .ok_or(ContractError::ClaimNotFound)?;
    
    if claim.is_processed {
        return Err(ContractError::ClaimAlreadyProcessed);
    }
    
    let mut pool = get_insurance_pool(env)?;
    let mut member = get_pool_member(env, &claim.claimant)?;
    
    // Verify sufficient funds
    if pool.total_funds < claim.approved_amount {
        return Err(ContractError::InsufficientPoolFunds);
    }
    
    // Update pool and member
    pool.total_funds = pool.total_funds.saturating_sub(claim.approved_amount);
    member.last_claim_timestamp = env.ledger().timestamp();
    member.claim_count = member.claim_count.saturating_add(1);
    
    // Mark claim as processed
    claim.is_processed = true;
    
    // Store updates
    env.storage().instance().set(&DataKey::InsurancePool, &pool);
    env.storage().instance().set(&DataKey::InsurancePoolMember(claim.claimant.clone()), &member);
    env.storage().instance().set(&DataKey::InsuranceClaim(claim_id), &claim);
    
    // Transfer funds to claimant's meter
    let mut meter: Meter = env.storage()
        .instance()
        .get(&DataKey::Meter(claim.meter_id))
        .ok_or(ContractError::MeterNotFound)?;
    
    match meter.billing_type {
        BillingType::PrePaid => {
            meter.balance = meter.balance.saturating_add(claim.approved_amount);
        }
        BillingType::PostPaid => {
            // For postpaid, reduce debt or increase collateral
            if meter.debt > 0 {
                let debt_payment = claim.approved_amount.min(meter.debt);
                meter.debt = meter.debt.saturating_sub(debt_payment);
                let remaining = claim.approved_amount.saturating_sub(debt_payment);
                meter.collateral_limit = meter.collateral_limit.saturating_add(remaining);
            } else {
                meter.collateral_limit = meter.collateral_limit.saturating_add(claim.approved_amount);
            }
        }
    }
    
    env.storage().instance().set(&DataKey::Meter(claim.meter_id), &meter);
    
    env.events().publish(
        (symbol_short!("ClaimPaid"), &claim.claimant),
        (claim_id, claim.approved_amount)
    );
    
    Ok(())
}

pub fn create_governance_proposal(
    env: &Env,
    proposer: Address,
    proposal_type: ProposalType,
    description: Symbol,
    new_value: i128,
) -> Result<u64, ContractError> {
    proposer.require_auth();
    
    let pool = get_insurance_pool(env)?;
    let member = get_pool_member(env, &proposer)?;
    
    if !member.is_active {
        return Err(ContractError::MemberInactive);
    }
    
    // Check minimum voting power to propose
    let min_voting_power = pool.total_voting_power / 20; // 5% of total voting power
    let proposer_power = member.calculate_voting_power(env.ledger().timestamp());
    
    if proposer_power < min_voting_power {
        return Err(ContractError::InsufficientVotingPower);
    }
    
    let proposal_id: u64 = env.storage()
        .instance()
        .get(&DataKey::InsuranceNextProposalId)
        .unwrap_or(1);
    
    let now = env.ledger().timestamp();
    let proposal = GovernanceProposal {
        proposal_id,
        proposer: proposer.clone(),
        proposal_type,
        description,
        new_value,
        created_at: now,
        voting_deadline: now.saturating_add(VOTING_PERIOD_SECONDS),
        votes_for: 0,
        votes_against: 0,
        total_votes: 0,
        is_executed: false,
        is_cancelled: false,
    };
    
    env.storage().instance().set(&DataKey::InsuranceProposal(proposal_id), &proposal);
    env.storage().instance().set(&DataKey::InsuranceNextProposalId, &(proposal_id + 1));
    
    env.events().publish(
        (symbol_short!("PropCrtd"), &proposer),
        (proposal_id, new_value)
    );
    
    Ok(proposal_id)
}

pub fn vote_on_proposal(
    env: &Env,
    voter: Address,
    proposal_id: u64,
    vote_for: bool,
) -> Result<(), ContractError> {
    voter.require_auth();
    
    let member = get_pool_member(env, &voter)?;
    if !member.is_active {
        return Err(ContractError::MemberInactive);
    }
    
    let mut proposal: GovernanceProposal = env.storage()
        .instance()
        .get(&DataKey::InsuranceProposal(proposal_id))
        .ok_or(ContractError::ProposalNotFound)?;
    
    let now = env.ledger().timestamp();
    if now > proposal.voting_deadline {
        return Err(ContractError::VotingPeriodExpired);
    }
    
    if proposal.is_executed || proposal.is_cancelled {
        return Err(ContractError::ProposalNotActive);
    }
    
    // Check if already voted
    let vote_key = DataKey::InsuranceVote(voter.clone(), proposal_id);
    if env.storage().instance().has(&vote_key) {
        return Err(ContractError::AlreadyVoted);
    }
    
    let voting_power = member.calculate_voting_power(now);
    
    if vote_for {
        proposal.votes_for = proposal.votes_for.saturating_add(voting_power);
    } else {
        proposal.votes_against = proposal.votes_against.saturating_add(voting_power);
    }
    proposal.total_votes = proposal.total_votes.saturating_add(voting_power);
    
    // Record vote
    env.storage().instance().set(&vote_key, &vote_for);
    env.storage().instance().set(&DataKey::InsuranceProposal(proposal_id), &proposal);
    
    env.events().publish(
        (symbol_short!("Vote"), &voter),
        (proposal_id, vote_for, voting_power)
    );
    
    Ok(())
}

pub fn execute_proposal(env: &Env, proposal_id: u64) -> Result<(), ContractError> {
    let mut proposal: GovernanceProposal = env.storage()
        .instance()
        .get(&DataKey::InsuranceProposal(proposal_id))
        .ok_or(ContractError::ProposalNotFound)?;
    
    let now = env.ledger().timestamp();
    if now <= proposal.voting_deadline {
        return Err(ContractError::VotingPeriodActive);
    }
    
    if proposal.is_executed || proposal.is_cancelled {
        return Err(ContractError::ProposalNotActive);
    }
    
    let pool = get_insurance_pool(env)?;
    
    // Check quorum
    let quorum_required = (pool.total_voting_power * QUORUM_THRESHOLD_BPS) / 10000;
    if proposal.total_votes < quorum_required {
        proposal.is_cancelled = true;
        env.storage().instance().set(&DataKey::InsuranceProposal(proposal_id), &proposal);
        return Err(ContractError::QuorumNotMet);
    }
    
    // Check approval
    let approval_required = (proposal.total_votes * APPROVAL_THRESHOLD_BPS) / 10000;
    if proposal.votes_for < approval_required {
        proposal.is_cancelled = true;
        env.storage().instance().set(&DataKey::InsuranceProposal(proposal_id), &proposal);
        return Err(ContractError::ProposalRejected);
    }
    
    // Execute proposal
    execute_proposal_action(env, &proposal)?;
    
    proposal.is_executed = true;
    env.storage().instance().set(&DataKey::InsuranceProposal(proposal_id), &proposal);
    
    env.events().publish(
        (symbol_short!("PropExec"), &proposal.proposer),
        (proposal_id, proposal.new_value)
    );
    
    Ok(())
}

fn execute_proposal_action(env: &Env, proposal: &GovernanceProposal) -> Result<(), ContractError> {
    let mut pool = get_insurance_pool(env)?;
    
    match proposal.proposal_type {
        ProposalType::ChangePremiumRate => {
            pool.base_premium_rate_bps = proposal.new_value.max(10).min(1000);
        }
        ProposalType::ChangeRiskMultiplier => {
            pool.risk_multiplier_max = proposal.new_value.max(100).min(500);
        }
        ProposalType::EmergencyPause => {
            pool.emergency_pause = proposal.new_value > 0;
        }
        ProposalType::ChangeGovernanceAdmin => {
            // Would need to decode address from new_value - simplified for now
            return Err(ContractError::NotImplemented);
        }
        _ => {
            return Err(ContractError::InvalidProposalType);
        }
    }
    
    env.storage().instance().set(&DataKey::InsurancePool, &pool);
    Ok(())
}

pub fn allocate_claim_fees_to_pool(env: &Env, claim_amount: i128) -> i128 {
    if let Ok(mut pool) = get_insurance_pool(env) {
        let pool_allocation = (claim_amount * INSURANCE_POOL_FEE_BPS) / 10000;
        pool.total_funds = pool.total_funds.saturating_add(pool_allocation);
        env.storage().instance().set(&DataKey::InsurancePool, &pool);
        
        env.events().publish(
            (symbol_short!("PoolFund"), pool_allocation),
            claim_amount
        );
        
        pool_allocation
    } else {
        0
    }
}