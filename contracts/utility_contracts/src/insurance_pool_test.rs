#[cfg(test)]
mod insurance_pool_tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Symbol, symbol_short};

    fn create_test_env() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        
        (env, admin, user1, user2)
    }

    fn setup_test_meter(env: &Env, user: &Address, provider: &Address) -> u64 {
        let token = Address::generate(env);
        let device_key = BytesN::from_array(env, &[1u8; 32]);
        
        // Add supported token
        env.storage().instance().set(&DataKey::SupportedToken(token.clone()), &true);
        
        // Register meter
        let meter_id = UtilityContract::register_meter(
            env.clone(),
            user.clone(),
            provider.clone(),
            1000, // off_peak_rate
            token,
            BillingType::PrePaid,
            device_key,
            0, // priority_index
        );
        
        // Top up the meter
        UtilityContract::top_up(env.clone(), user.clone(), meter_id, 100_000_000).unwrap();
        
        meter_id
    }

    #[test]
    fn test_create_insurance_pool() {
        let (env, admin, _user1, _user2) = create_test_env();
        
        // Create insurance pool
        let result = UtilityContract::create_insurance_pool(
            env.clone(),
            admin.clone(),
            100, // 1% base premium rate
        );
        
        assert!(result.is_ok());
        
        // Verify pool was created
        let pool = UtilityContract::get_insurance_pool(env.clone()).unwrap();
        assert_eq!(pool.governance_admin, admin);
        assert_eq!(pool.base_premium_rate_bps, 100);
        assert_eq!(pool.total_members, 0);
        assert_eq!(pool.total_funds, 0);
        assert!(pool.is_active);
        assert!(!pool.emergency_pause);
    }

    #[test]
    fn test_join_insurance_pool() {
        let (env, admin, user1, _user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Create insurance pool
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        
        // Setup meter for user1
        let meter_id = setup_test_meter(&env, &user1, &provider);
        
        // Calculate premium
        let premium = UtilityContract::calculate_premium_amount(
            env.clone(),
            user1.clone(),
            meter_id,
        ).unwrap();
        
        // Join pool
        let result = UtilityContract::join_insurance_pool(
            env.clone(),
            user1.clone(),
            meter_id,
            premium,
        );
        
        assert!(result.is_ok());
        
        // Verify membership
        let member = UtilityContract::get_pool_member(env.clone(), user1.clone()).unwrap();
        assert_eq!(member.user, user1);
        assert_eq!(member.premium_paid, premium);
        assert!(member.is_active);
        assert_eq!(member.claim_count, 0);
        
        // Verify pool updated
        let pool = UtilityContract::get_insurance_pool(env.clone()).unwrap();
        assert_eq!(pool.total_members, 1);
        assert_eq!(pool.total_funds, premium);
    }

    #[test]
    fn test_submit_insurance_claim() {
        let (env, admin, user1, _user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool and member
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        let meter_id = setup_test_meter(&env, &user1, &provider);
        let premium = UtilityContract::calculate_premium_amount(env.clone(), user1.clone(), meter_id).unwrap();
        UtilityContract::join_insurance_pool(env.clone(), user1.clone(), meter_id, premium).unwrap();
        
        // Submit claim
        let claim_amount = 1_000_000; // 1 XLM
        let reason = symbol_short!("EmergFund");
        
        let claim_id = UtilityContract::submit_insurance_claim(
            env.clone(),
            user1.clone(),
            meter_id,
            claim_amount,
            reason,
        ).unwrap();
        
        assert_eq!(claim_id, 1);
        
        // Verify claim was created
        let claim: InsuranceClaim = env.storage()
            .instance()
            .get(&DataKey::InsuranceClaim(claim_id))
            .unwrap();
        
        assert_eq!(claim.claimant, user1);
        assert_eq!(claim.meter_id, meter_id);
        assert_eq!(claim.requested_amount, claim_amount);
        assert_eq!(claim.reason, reason);
    }

    #[test]
    fn test_governance_proposal_creation() {
        let (env, admin, user1, user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool with multiple members
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        
        let meter_id1 = setup_test_meter(&env, &user1, &provider);
        let premium1 = UtilityContract::calculate_premium_amount(env.clone(), user1.clone(), meter_id1).unwrap();
        UtilityContract::join_insurance_pool(env.clone(), user1.clone(), meter_id1, premium1).unwrap();
        
        let meter_id2 = setup_test_meter(&env, &user2, &provider);
        let premium2 = UtilityContract::calculate_premium_amount(env.clone(), user2.clone(), meter_id2).unwrap();
        UtilityContract::join_insurance_pool(env.clone(), user2.clone(), meter_id2, premium2).unwrap();
        
        // Create proposal to change premium rate
        let proposal_id = UtilityContract::create_governance_proposal(
            env.clone(),
            user1.clone(),
            ProposalType::ChangePremiumRate,
            symbol_short!("NewRate"),
            150, // 1.5% new rate
        ).unwrap();
        
        assert_eq!(proposal_id, 1);
        
        // Verify proposal
        let proposal: GovernanceProposal = env.storage()
            .instance()
            .get(&DataKey::InsuranceProposal(proposal_id))
            .unwrap();
        
        assert_eq!(proposal.proposer, user1);
        assert_eq!(proposal.new_value, 150);
        assert!(!proposal.is_executed);
        assert!(!proposal.is_cancelled);
    }

    #[test]
    fn test_voting_on_proposal() {
        let (env, admin, user1, user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool with members
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        
        let meter_id1 = setup_test_meter(&env, &user1, &provider);
        let premium1 = UtilityContract::calculate_premium_amount(env.clone(), user1.clone(), meter_id1).unwrap();
        UtilityContract::join_insurance_pool(env.clone(), user1.clone(), meter_id1, premium1).unwrap();
        
        let meter_id2 = setup_test_meter(&env, &user2, &provider);
        let premium2 = UtilityContract::calculate_premium_amount(env.clone(), user2.clone(), meter_id2).unwrap();
        UtilityContract::join_insurance_pool(env.clone(), user2.clone(), meter_id2, premium2).unwrap();
        
        // Create proposal
        let proposal_id = UtilityContract::create_governance_proposal(
            env.clone(),
            user1.clone(),
            ProposalType::ChangePremiumRate,
            symbol_short!("NewRate"),
            150,
        ).unwrap();
        
        // Vote on proposal
        let vote_result1 = UtilityContract::vote_on_proposal(
            env.clone(),
            user1.clone(),
            proposal_id,
            true, // vote for
        );
        assert!(vote_result1.is_ok());
        
        let vote_result2 = UtilityContract::vote_on_proposal(
            env.clone(),
            user2.clone(),
            proposal_id,
            false, // vote against
        );
        assert!(vote_result2.is_ok());
        
        // Verify votes were recorded
        let proposal: GovernanceProposal = env.storage()
            .instance()
            .get(&DataKey::InsuranceProposal(proposal_id))
            .unwrap();
        
        assert!(proposal.votes_for > 0);
        assert!(proposal.votes_against > 0);
        assert_eq!(proposal.total_votes, proposal.votes_for + proposal.votes_against);
    }

    #[test]
    fn test_risk_assessment_calculation() {
        let (env, admin, user1, _user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        let meter_id = setup_test_meter(&env, &user1, &provider);
        
        // Calculate risk score
        let risk_score = calculate_risk_score(&env, &user1, meter_id);
        
        // Risk score should be reasonable (0-1000 range)
        assert!(risk_score <= 1000);
        assert!(risk_score > 0);
        
        // Calculate premium based on risk
        let premium = UtilityContract::calculate_premium_amount(
            env.clone(),
            user1.clone(),
            meter_id,
        ).unwrap();
        
        // Premium should be within bounds
        assert!(premium >= MIN_PREMIUM_PAYMENT);
        assert!(premium <= MAX_PREMIUM_PAYMENT);
    }

    #[test]
    fn test_auto_approved_small_claims() {
        let (env, admin, user1, _user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool with sufficient funds
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        let meter_id = setup_test_meter(&env, &user1, &provider);
        let premium = 10_000_000_000i128; // Large premium to ensure sufficient pool funds
        UtilityContract::join_insurance_pool(env.clone(), user1.clone(), meter_id, premium).unwrap();
        
        // Submit small claim (should be auto-approved)
        let small_claim_amount = premium / 200; // 0.5% of pool
        let claim_id = UtilityContract::submit_insurance_claim(
            env.clone(),
            user1.clone(),
            meter_id,
            small_claim_amount,
            symbol_short!("SmallEmrg"),
        ).unwrap();
        
        // Verify claim was auto-approved and processed
        let claim: InsuranceClaim = env.storage()
            .instance()
            .get(&DataKey::InsuranceClaim(claim_id))
            .unwrap();
        
        assert!(claim.auto_approved);
        assert!(claim.is_processed);
        assert_eq!(claim.approved_amount, small_claim_amount);
        
        // Verify funds were transferred to meter
        let meter: Meter = env.storage()
            .instance()
            .get(&DataKey::Meter(meter_id))
            .unwrap();
        
        // Balance should have increased by claim amount
        assert!(meter.balance >= small_claim_amount);
    }

    #[test]
    fn test_insurance_pool_fee_allocation() {
        let (env, admin, user1, _user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        let initial_pool_funds = 1_000_000_000i128;
        
        // Manually set pool funds for testing
        let mut pool = get_insurance_pool(&env).unwrap();
        pool.total_funds = initial_pool_funds;
        env.storage().instance().set(&DataKey::InsurancePool, &pool);
        
        // Test fee allocation
        let claim_amount = 10_000_000i128; // 10 XLM
        let allocated_fee = allocate_claim_fees_to_pool(&env, claim_amount);
        
        // Verify fee was calculated correctly (0.5% of claim)
        let expected_fee = (claim_amount * INSURANCE_POOL_FEE_BPS) / 10000;
        assert_eq!(allocated_fee, expected_fee);
        
        // Verify pool funds increased
        let updated_pool = get_insurance_pool(&env).unwrap();
        assert_eq!(updated_pool.total_funds, initial_pool_funds + expected_fee);
    }

    #[test]
    fn test_cooldown_period_enforcement() {
        let (env, admin, user1, _user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool and member
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        let meter_id = setup_test_meter(&env, &user1, &provider);
        let premium = 10_000_000_000i128;
        UtilityContract::join_insurance_pool(env.clone(), user1.clone(), meter_id, premium).unwrap();
        
        // Submit first claim
        let claim_amount = premium / 200;
        let claim_id1 = UtilityContract::submit_insurance_claim(
            env.clone(),
            user1.clone(),
            meter_id,
            claim_amount,
            symbol_short!("First"),
        ).unwrap();
        
        // Try to submit second claim immediately (should fail due to cooldown)
        let result = UtilityContract::submit_insurance_claim(
            env.clone(),
            user1.clone(),
            meter_id,
            claim_amount,
            symbol_short!("Second"),
        );
        
        assert_eq!(result.unwrap_err(), ContractError::ClaimCooldownActive);
    }

    #[test]
    fn test_emergency_pause_functionality() {
        let (env, admin, user1, user2) = create_test_env();
        let provider = Address::generate(&env);
        
        // Setup pool with members
        UtilityContract::create_insurance_pool(env.clone(), admin, 100).unwrap();
        
        let meter_id1 = setup_test_meter(&env, &user1, &provider);
        let premium1 = UtilityContract::calculate_premium_amount(env.clone(), user1.clone(), meter_id1).unwrap();
        UtilityContract::join_insurance_pool(env.clone(), user1.clone(), meter_id1, premium1).unwrap();
        
        let meter_id2 = setup_test_meter(&env, &user2, &provider);
        let premium2 = UtilityContract::calculate_premium_amount(env.clone(), user2.clone(), meter_id2).unwrap();
        UtilityContract::join_insurance_pool(env.clone(), user2.clone(), meter_id2, premium2).unwrap();
        
        // Create emergency pause proposal
        let proposal_id = UtilityContract::create_governance_proposal(
            env.clone(),
            user1.clone(),
            ProposalType::EmergencyPause,
            symbol_short!("EmrgPause"),
            1, // Enable pause
        ).unwrap();
        
        // Vote to approve pause
        UtilityContract::vote_on_proposal(env.clone(), user1.clone(), proposal_id, true).unwrap();
        UtilityContract::vote_on_proposal(env.clone(), user2.clone(), proposal_id, true).unwrap();
        
        // Fast forward time to end voting period
        env.ledger().with_mut(|li| {
            li.timestamp = li.timestamp + VOTING_PERIOD_SECONDS + 1;
        });
        
        // Execute proposal
        UtilityContract::execute_proposal(env.clone(), proposal_id).unwrap();
        
        // Verify pool is paused
        let pool = UtilityContract::get_insurance_pool(env.clone()).unwrap();
        assert!(pool.emergency_pause);
        
        // Try to submit claim (should fail due to pause)
        let result = UtilityContract::submit_insurance_claim(
            env.clone(),
            user1.clone(),
            meter_id1,
            1_000_000,
            symbol_short!("TestClaim"),
        );
        
        assert_eq!(result.unwrap_err(), ContractError::InsurancePoolInactive);
    }
}