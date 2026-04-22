// Comprehensive tests for Issue #119: Milestone-Based Maintenance Fund Release
// This test suite validates the step-logic and sequential milestone verification

use soroban_sdk::{
    contractimport, symbol_short, Address, Bytes, BytesN, Env, String, Vec,
};

#[contractimport]
__!("../../target/wasm32-unknown-unknown/release/utility_contracts.wasm");

type UtilityContract = UtilityContractClient<'static>;

#[test]
fn test_maintenance_fund_creation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup test addresses
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Create a test meter first
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let meter_id = client.register_meter(
        &user,
        &provider,
        &1000i128, // off_peak_rate
        &token,
        &device_key,
    );
    
    // Test 1: Create maintenance fund with valid parameters
    let total_amount = 10000i128;
    let milestone_count = 3u32;
    
    client.create_maintenance_fund(
        &meter_id,
        &total_amount,
        &milestone_count,
    );
    
    // Verify fund was created correctly
    let fund = client.get_maintenance_fund(&meter_id);
    assert_eq!(fund.meter_id, meter_id);
    assert_eq!(fund.total_allocated, total_amount);
    assert_eq!(fund.total_released, 0);
    assert_eq!(fund.current_milestone, 0);
    assert_eq!(fund.total_milestones, milestone_count);
    assert!(fund.is_active);
    
    println!("Test 1: Maintenance fund creation successful");
}

#[test]
fn test_milestone_addition_and_verification() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup test addresses
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Create a test meter
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let meter_id = client.register_meter(&user, &provider, &1000i128, &token, &device_key);
    
    // Create maintenance fund
    let total_amount = 9000i128;
    let milestone_count = 3u32;
    client.create_maintenance_fund(&meter_id, &total_amount, &milestone_count);
    
    // Test 1: Add milestones with different funding amounts
    let milestone1_amount = 3000i128;
    let milestone2_amount = 3000i128;
    let milestone3_amount = 3000i128;
    
    client.add_milestone(
        &meter_id,
        &1u32,
        &String::from_str(&env, "Install generator foundation"),
        &milestone1_amount,
    );
    
    client.add_milestone(
        &meter_id,
        &2u32,
        &String::from_str(&env, "Install generator and connect to grid"),
        &milestone2_amount,
    );
    
    client.add_milestone(
        &meter_id,
        &3u32,
        &String::from_str(&env, "Final testing and commissioning"),
        &milestone3_amount,
    );
    
    // Verify milestones were added correctly
    let milestone1 = client.get_milestone(&meter_id, &1u32);
    assert_eq!(milestone1.milestone_number, 1);
    assert_eq!(milestone1.funding_amount, milestone1_amount);
    assert!(!milestone1.is_completed);
    
    let milestone2 = client.get_milestone(&meter_id, &2u32);
    assert_eq!(milestone2.milestone_number, 2);
    assert_eq!(milestone2.funding_amount, milestone2_amount);
    assert!(!milestone2.is_completed);
    
    println!("Test 1: Milestone addition successful");
}

#[test]
fn test_sequential_milestone_completion() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup test addresses
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let maintenance_wallet = Address::generate(&env);
    
    // Set maintenance wallet
    client.set_maintenance_config(&maintenance_wallet, &0i128);
    
    // Create a test meter
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let meter_id = client.register_meter(&user, &provider, &1000i128, &token, &device_key);
    
    // Create maintenance fund
    let total_amount = 6000i128;
    let milestone_count = 3u32;
    client.create_maintenance_fund(&meter_id, &total_amount, &milestone_count);
    
    // Add milestones
    client.add_milestone(
        &meter_id,
        &1u32,
        &String::from_str(&env, "Foundation work"),
        &2000i128,
    );
    
    client.add_milestone(
        &meter_id,
        &2u32,
        &String::from_str(&env, "Generator installation"),
        &2000i128,
    );
    
    client.add_milestone(
        &meter_id,
        &3u32,
        &String::from_str(&env, "Commissioning"),
        &2000i128,
    );
    
    // Test 1: Complete milestone 1 (should succeed)
    let completion_proof = Bytes::from_slice(&env, &[1, 2, 3, 4]);
    client.complete_milestone(
        &meter_id,
        &1u32,
        &completion_proof.clone(),
        &admin,
    );
    
    // Verify milestone 1 is completed
    let milestone1 = client.get_milestone(&meter_id, &1u32);
    assert!(milestone1.is_completed);
    assert_eq!(milestone1.verified_by, admin);
    
    // Verify fund status
    let fund = client.get_maintenance_fund(&meter_id);
    assert_eq!(fund.total_released, 2000i128);
    assert_eq!(fund.current_milestone, 1u32);
    
    println!("Test 1: Milestone 1 completion successful");
    
    // Test 2: Try to complete milestone 3 before milestone 2 (should fail)
    let result = env.try_invoke::<_, ()>(
        &contract_id,
        &UtilityContract::complete_milestone(
            &meter_id,
            &3u32,
            &completion_proof,
            &admin,
        ),
    );
    
    assert!(result.is_err());
    println!("Test 2: Sequential completion validation works");
    
    // Test 3: Complete milestone 2 (should succeed)
    client.complete_milestone(
        &meter_id,
        &2u32,
        &completion_proof.clone(),
        &admin,
    );
    
    // Verify milestone 2 is completed
    let milestone2 = client.get_milestone(&meter_id, &2u32);
    assert!(milestone2.is_completed);
    
    // Verify fund status
    let fund = client.get_maintenance_fund(&meter_id);
    assert_eq!(fund.total_released, 4000i128);
    assert_eq!(fund.current_milestone, 2u32);
    
    println!("Test 3: Milestone 2 completion successful");
    
    // Test 4: Complete final milestone
    client.complete_milestone(
        &meter_id,
        &3u32,
        &completion_proof,
        &admin,
    );
    
    // Verify all milestones are completed
    let milestone3 = client.get_milestone(&meter_id, &3u32);
    assert!(milestone3.is_completed);
    
    // Verify fund is fully released
    let fund = client.get_maintenance_fund(&meter_id);
    assert_eq!(fund.total_released, 6000i128);
    assert_eq!(fund.current_milestone, 3u32);
    
    println!("Test 4: All milestones completed successfully");
}

#[test]
fn test_milestone_error_conditions() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup test addresses
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    
    // Create a test meter
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let meter_id = client.register_meter(&user, &provider, &1000i128, &token, &device_key);
    
    // Create maintenance fund
    let total_amount = 3000i128;
    let milestone_count = 2u32;
    client.create_maintenance_fund(&meter_id, &total_amount, &milestone_count);
    
    // Add a milestone
    client.add_milestone(
        &meter_id,
        &1u32,
        &String::from_str(&env, "Test milestone"),
        &1500i128,
    );
    
    // Test 1: Try to complete milestone with unauthorized user (should fail)
    let completion_proof = Bytes::from_slice(&env, &[1, 2, 3, 4]);
    let result = env.try_invoke::<_, ()>(
        &contract_id,
        &UtilityContract::complete_milestone(
            &meter_id,
            &1u32,
            &completion_proof,
            &unauthorized_user,
        ),
    );
    
    assert!(result.is_err());
    println!("Test 1: Unauthorized completion blocked");
    
    // Test 2: Try to complete same milestone twice (should fail)
    client.complete_milestone(
        &meter_id,
        &1u32,
        &completion_proof.clone(),
        &admin,
    );
    
    let result = env.try_invoke::<_, ()>(
        &contract_id,
        &UtilityContract::complete_milestone(
            &meter_id,
            &1u32,
            &completion_proof,
            &admin,
        ),
    );
    
    assert!(result.is_err());
    println!("Test 2: Duplicate completion blocked");
    
    // Test 3: Try to add milestone beyond count (should fail)
    let result = env.try_invoke::<_, ()>(
        &contract_id,
        &UtilityContract::add_milestone(
            &meter_id,
            &3u32, // Beyond the 2 milestones allocated
            &String::from_str(&env, "Invalid milestone"),
            &1000i128,
        ),
    );
    
    assert!(result.is_err());
    println!("Test 3: Invalid milestone number blocked");
}

#[test]
fn test_fund_insufficient_protection() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup test addresses
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Create a test meter
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let meter_id = client.register_meter(&user, &provider, &1000i128, &token, &device_key);
    
    // Create maintenance fund with insufficient total for milestones
    let total_amount = 1000i128; // Less than what we'll try to release
    let milestone_count = 2u32;
    client.create_maintenance_fund(&meter_id, &total_amount, &milestone_count);
    
    // Add milestone with amount exceeding total fund
    client.add_milestone(
        &meter_id,
        &1u32,
        &String::from_str(&env, "Oversized milestone"),
        &2000i128, // More than total fund
    );
    
    // Test: Try to complete milestone with insufficient funds (should fail)
    let completion_proof = Bytes::from_slice(&env, &[1, 2, 3, 4]);
    let result = env.try_invoke::<_, ()>(
        &contract_id,
        &UtilityContract::complete_milestone(
            &meter_id,
            &1u32,
            &completion_proof,
            &admin,
        ),
    );
    
    assert!(result.is_err());
    println!("Test: Insufficient funds protection works");
}

#[test]
fn test_real_world_neighborhood_generator_scenario() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup realistic scenario
    let community_admin = Address::generate(&env);
    let maintenance_company = Address::generate(&env);
    let neighborhood_association = Address::generate(&env);
    let token = Address::generate(&env);
    let maintenance_wallet = Address::generate(&env);
    
    // Set maintenance wallet
    client.set_maintenance_config(&maintenance_wallet, &0i128);
    
    // Create meter for neighborhood generator
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let generator_meter_id = client.register_meter(
        &neighborhood_association,
        &maintenance_company,
        &1000i128,
        &token,
        &device_key,
    );
    
    // Create maintenance fund for generator overhaul ($50,000 total)
    let total_budget = 50000i128; // Representing $50,000 in cents
    let phases = 5u32;
    client.create_maintenance_fund(&generator_meter_id, &total_budget, &phases);
    
    // Phase 1: Site Preparation and Foundation ($10,000)
    client.add_milestone(
        &generator_meter_id,
        &1u32,
        &String::from_str(&env, "Site preparation, excavation, and concrete foundation work"),
        &10000i128,
    );
    
    // Phase 2: Generator Installation ($15,000)
    client.add_milestone(
        &generator_meter_id,
        &2u32,
        &String::from_str(&env, "Delivery and installation of generator unit"),
        &15000i128,
    );
    
    // Phase 3: Electrical Work ($12,000)
    client.add_milestone(
        &generator_meter_id,
        &3u32,
        &String::from_str(&env, "Electrical wiring, panel upgrades, and connection to grid"),
        &12000i128,
    );
    
    // Phase 4: Fuel System Installation ($8,000)
    client.add_milestone(
        &generator_meter_id,
        &4u32,
        &String::from_str(&env, "Fuel tank installation and plumbing"),
        &8000i128,
    );
    
    // Phase 5: Testing and Commissioning ($5,000)
    client.add_milestone(
        &generator_meter_id,
        &5u32,
        &String::from_str(&env, "System testing, calibration, and final commissioning"),
        &5000i128,
    );
    
    // Simulate sequential completion with admin verification
    let phases_completed = vec![
        (1u32, "Foundation completed and inspected"),
        (2u32, "Generator installed and secured"),
        (3u32, "Electrical work passed inspection"),
        (4u32, "Fuel system fully installed"),
        (5u32, "System commissioned and operational"),
    ];
    
    for (phase_num, description) in phases_completed {
        let proof = Bytes::from_slice(&env, description.as_bytes());
        client.complete_milestone(
            &generator_meter_id,
            &phase_num,
            &proof,
            &community_admin,
        );
        
        let milestone = client.get_milestone(&generator_meter_id, &phase_num);
        assert!(milestone.is_completed, "Phase {} should be completed", phase_num);
        
        let fund = client.get_maintenance_fund(&generator_meter_id);
        println!("Phase {} completed: ${:.2} released", 
                phase_num, 
                fund.total_released as f64 / 100.0);
    }
    
    // Final verification
    let final_fund = client.get_maintenance_fund(&generator_meter_id);
    assert_eq!(final_fund.total_released, 50000i128);
    assert_eq!(final_fund.current_milestone, 5u32);
    
    println!("Real-world scenario simulation completed successfully!");
    println!("All 5 phases completed sequentially with proper verification");
    println!("Total funds released: ${:.2}", final_fund.total_released as f64 / 100.0);
}

#[test]
fn test_step_logic_edge_cases() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup test addresses
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Create meter
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let meter_id = client.register_meter(&user, &provider, &1000i128, &token, &device_key);
    
    // Create fund with single milestone
    let total_amount = 5000i128;
    let milestone_count = 1u32;
    client.create_maintenance_fund(&meter_id, &total_amount, &milestone_count);
    
    // Add single milestone
    client.add_milestone(
        &meter_id,
        &1u32,
        &String::from_str(&env, "Single milestone test"),
        &5000i128,
    );
    
    // Test: Single milestone should complete without sequential checks
    let completion_proof = Bytes::from_slice(&env, &[1, 2, 3, 4]);
    client.complete_milestone(
        &meter_id,
        &1u32,
        &completion_proof,
        &admin,
    );
    
    let milestone = client.get_milestone(&meter_id, &1u32);
    assert!(milestone.is_completed);
    
    let fund = client.get_maintenance_fund(&meter_id);
    assert_eq!(fund.total_released, 5000i128);
    assert_eq!(fund.current_milestone, 1u32);
    
    println!("Single milestone edge case handled correctly");
}

#[test]
fn test_milestone_data_integrity() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContract::new(&env, &contract_id);
    
    // Setup test addresses
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Create meter
    let device_key = BytesN::from_array(&env, &[1; 32]);
    let meter_id = client.register_meter(&user, &provider, &1000i128, &token, &device_key);
    
    // Create fund
    let total_amount = 3000i128;
    let milestone_count = 2u32;
    client.create_maintenance_fund(&meter_id, &total_amount, &milestone_count);
    
    // Add milestone with detailed information
    let description = String::from_str(&env, "Complete electrical rewiring and safety inspection");
    let funding_amount = 1500i128;
    let milestone_number = 1u32;
    
    client.add_milestone(
        &meter_id,
        &milestone_number,
        &description.clone(),
        &funding_amount,
    );
    
    // Verify data integrity before completion
    let milestone_before = client.get_milestone(&meter_id, &milestone_number);
    assert_eq!(milestone_before.description, description);
    assert_eq!(milestone_before.funding_amount, funding_amount);
    assert!(!milestone_before.is_completed);
    assert_eq!(milestone_before.completed_at, 0);
    assert_eq!(milestone_before.verified_by, Address::from_contract_id(&BytesN::from_array(&[0; 32])));
    
    // Complete milestone
    let completion_proof = Bytes::from_slice(&env, &[1, 2, 3, 4, 5]);
    let completion_time = env.ledger().timestamp();
    
    client.complete_milestone(
        &meter_id,
        &milestone_number,
        &completion_proof,
        &admin,
    );
    
    // Verify data integrity after completion
    let milestone_after = client.get_milestone(&meter_id, &milestone_number);
    assert_eq!(milestone_after.description, description);
    assert_eq!(milestone_after.funding_amount, funding_amount);
    assert!(milestone_after.is_completed);
    assert_eq!(milestone_after.completed_at, completion_time);
    assert_eq!(milestone_after.verified_by, admin);
    assert_eq!(milestone_after.completion_proof, completion_proof);
    
    println!("Data integrity verification passed");
}

fn main() {
    println!("Running Milestone-Based Maintenance Fund Release Tests...");
    println!("Issue #119: Step-Logic with Sequential Verification");
    println!("========================================================");
    
    test_maintenance_fund_creation();
    test_milestone_addition_and_verification();
    test_sequential_milestone_completion();
    test_milestone_error_conditions();
    test_fund_insufficient_protection();
    test_real_world_neighborhood_generator_scenario();
    test_step_logic_edge_cases();
    test_milestone_data_integrity();
    
    println!("========================================================");
    println!("All milestone-based maintenance fund tests passed! ");
    println!("Step-logic correctly enforces sequential milestone completion");
    println!("Admin verification prevents unauthorized milestone completion");
    println!("Fund protection prevents over-release of maintenance funds");
    println!("Real-world neighborhood generator scenario validated");
}
