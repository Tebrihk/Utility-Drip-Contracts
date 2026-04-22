// Comprehensive tests for Inter-Susu Reputation Migration functionality
// Issue #127: Support for Inter-Susu_Reputation_Migration_for_Renters

use soroban_sdk::{contractimpl, Address, BytesN, Env, Symbol, symbol_short};
use utility_contracts::{ReputationRecord, ReputationMigration, ContractError, DataKey};

#[test]
fn test_reputation_export_burns_old_record() {
    let env = Env::default();
    let user = Address::random(&env);
    let old_contract = env.register_contract(None, utility_contracts::Contract);
    
    // Create initial reputation record
    let initial_reputation = ReputationRecord {
        user: user.clone(),
        reliability_score: 85,
        total_payments: 12,
        on_time_payments: 11,
        total_usage: 5000,
        created_at: 1640995200, // Jan 1, 2022
        last_updated: 1643673600, // Feb 1, 2022
        is_active: true,
    };
    
    // Store initial reputation
    env.storage().instance().set(&DataKey::UserReputation(user.clone()), &initial_reputation);
    
    // Export reputation (should burn old record)
    let exported_reputation = utility_contracts::Contract::export_reputation(&env, user.clone());
    
    // Verify exported data matches original
    assert_eq!(exported_reputation.user, initial_reputation.user);
    assert_eq!(exported_reputation.reliability_score, initial_reputation.reliability_score);
    assert_eq!(exported_reputation.total_payments, initial_reputation.total_payments);
    assert_eq!(exported_reputation.on_time_payments, initial_reputation.on_time_payments);
    assert_eq!(exported_reputation.total_usage, initial_reputation.total_usage);
    
    // Verify old record is now inactive (burned)
    let burned_reputation = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    assert_eq!(burned_reputation.is_active, false);
    
    // Verify event was emitted
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].topics[0], symbol_short!("RepExport"));
    assert_eq!(events[0].topics[1], user);
    assert_eq!(events[0].data[0], exported_reputation.reliability_score);
}

#[test]
fn test_reputation_import_mints_new_record() {
    let env = Env::default();
    let user = Address::random(&env);
    let old_contract = Address::random(&env);
    let new_contract = env.register_contract(None, utility_contracts::Contract);
    
    // Create reputation record to import
    let reputation_to_import = ReputationRecord {
        user: user.clone(),
        reliability_score: 75,
        total_payments: 8,
        on_time_payments: 7,
        total_usage: 3000,
        created_at: 1640995200,
        last_updated: 1641081600,
        is_active: false, // Should be inactive from old contract
    };
    
    // Create unique nullifier
    let nullifier = BytesN::from_array(&env, &[1; 32]);
    let migration_signature = BytesN::from_array(&env, &[2; 64]);
    
    // Import reputation (should mint new record)
    utility_contracts::Contract::import_reputation(
        &env,
        old_contract.clone(),
        user.clone(),
        reputation_to_import.clone(),
        migration_signature,
        nullifier.clone(),
    );
    
    // Verify new record is active
    let active_reputation = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    assert_eq!(active_reputation.is_active, true);
    assert_eq!(active_reputation.reliability_score, reputation_to_import.reliability_score);
    assert_eq!(active_reputation.total_payments, reputation_to_import.total_payments);
    assert_eq!(active_reputation.on_time_payments, reputation_to_import.on_time_payments);
    assert_eq!(active_reputation.total_usage, reputation_to_import.total_usage);
    assert!(active_reputation.last_updated > reputation_to_import.last_updated); // Should be updated
    
    // Verify migration record was stored
    let migration = env.storage().instance()
        .get::<DataKey, ReputationMigration>(&DataKey::ReputationMigration(nullifier.clone()))
        .unwrap();
    assert_eq!(migration.old_contract, old_contract);
    assert_eq!(migration.new_contract, new_contract.address());
    assert_eq!(migration.user, user);
    assert_eq!(migration.nullifier, nullifier);
    
    // Verify migrated reputation flag was set
    assert!(env.storage().instance().has(&DataKey::MigratedReputation(user.clone(), old_contract.clone())));
    
    // Verify nullifier map was set
    assert!(env.storage().instance().has(&DataKey::NullifierMap(nullifier.clone())));
    
    // Verify event was emitted
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].topics[0], symbol_short!("RepImport"));
    assert_eq!(events[0].topics[1], user);
    assert_eq!(events[0].data[0], active_reputation.reliability_score);
    assert_eq!(events[0].data[1], old_contract);
}

#[test]
#[should_panic(expected = "ReputationNotFound")]
fn test_export_reputation_not_found() {
    let env = Env::default();
    let user = Address::random(&env);
    
    // Try to export non-existent reputation
    utility_contracts::Contract::export_reputation(&env, user);
}

#[test]
#[should_panic(expected = "ReputationAlreadyMigrated")]
fn test_import_already_migrated() {
    let env = Env::default();
    let user = Address::random(&env);
    let old_contract = Address::random(&env);
    
    // Set up migrated reputation flag
    env.storage().instance().set(&DataKey::MigratedReputation(user.clone(), old_contract.clone()), &true);
    
    let reputation_to_import = ReputationRecord {
        user: user.clone(),
        reliability_score: 80,
        total_payments: 10,
        on_time_payments: 9,
        total_usage: 4000,
        created_at: 1640995200,
        last_updated: 1641081600,
        is_active: false,
    };
    
    let nullifier = BytesN::from_array(&env, &[3; 32]);
    let migration_signature = BytesN::from_array(&env, &[4; 64]);
    
    // Try to import already migrated reputation
    utility_contracts::Contract::import_reputation(
        &env,
        old_contract,
        user,
        reputation_to_import,
        migration_signature,
        nullifier,
    );
}

#[test]
#[should_panic(expected = "NullifierAlreadyUsed")]
fn test_import_nullifier_already_used() {
    let env = Env::default();
    let user = Address::random(&env);
    let old_contract = Address::random(&env);
    
    // Set up used nullifier
    let nullifier = BytesN::from_array(&env, &[5; 32]);
    env.storage().instance().set(&DataKey::NullifierMap(nullifier.clone()), &true);
    
    let reputation_to_import = ReputationRecord {
        user: user.clone(),
        reliability_score: 90,
        total_payments: 15,
        on_time_payments: 15,
        total_usage: 6000,
        created_at: 1640995200,
        last_updated: 1641081600,
        is_active: false,
    };
    
    let migration_signature = BytesN::from_array(&env, &[6; 64]);
    
    // Try to import with used nullifier
    utility_contracts::Contract::import_reputation(
        &env,
        old_contract,
        user,
        reputation_to_import,
        migration_signature,
        nullifier,
    );
}

#[test]
fn test_get_reputation() {
    let env = Env::default();
    let user = Address::random(&env);
    
    let reputation = ReputationRecord {
        user: user.clone(),
        reliability_score: 95,
        total_payments: 20,
        on_time_payments: 20,
        total_usage: 8000,
        created_at: 1640995200,
        last_updated: 1641081600,
        is_active: true,
    };
    
    env.storage().instance().set(&DataKey::UserReputation(user.clone()), &reputation);
    
    let retrieved_reputation = utility_contracts::Contract::get_reputation(&env, user.clone());
    assert_eq!(retrieved_reputation.user, reputation.user);
    assert_eq!(retrieved_reputation.reliability_score, reputation.reliability_score);
    assert_eq!(retrieved_reputation.total_payments, reputation.total_payments);
    assert_eq!(retrieved_reputation.on_time_payments, reputation.on_time_payments);
    assert_eq!(retrieved_reputation.total_usage, reputation.total_usage);
}

#[test]
#[should_panic(expected = "ReputationNotFound")]
fn test_get_reputation_not_found() {
    let env = Env::default();
    let user = Address::random(&env);
    
    // Try to get non-existent reputation
    utility_contracts::Contract::get_reputation(&env, user);
}

#[test]
fn test_update_reputation_score_on_time_payment() {
    let env = Env::default();
    let user = Address::random(&env);
    let contract_address = env.register_contract(None, utility_contracts::Contract);
    
    // Create initial reputation
    let initial_reputation = ReputationRecord {
        user: user.clone(),
        reliability_score: 70,
        total_payments: 5,
        on_time_payments: 4,
        total_usage: 2000,
        created_at: 1640995200,
        last_updated: 1641081600,
        is_active: true,
    };
    
    env.storage().instance().set(&DataKey::UserReputation(user.clone()), &initial_reputation);
    
    // Update with on-time payment
    utility_contracts::Contract::update_reputation_score(&env, user.clone(), 1000, true);
    
    let updated_reputation = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    
    assert_eq!(updated_reputation.total_payments, 6);
    assert_eq!(updated_reputation.on_time_payments, 5);
    assert_eq!(updated_reputation.total_usage, 3000);
    assert!(updated_reputation.last_updated > initial_reputation.last_updated);
    
    // Score should improve (weighted average)
    let payment_ratio = (5 * 100) / 6; // 83
    let expected_score = ((70 * 3) + payment_ratio) / 4; // ~73
    assert_eq!(updated_reputation.reliability_score, expected_score);
}

#[test]
fn test_update_reputation_score_late_payment() {
    let env = Env::default();
    let user = Address::random(&env);
    let contract_address = env.register_contract(None, utility_contracts::Contract);
    
    // Create initial reputation
    let initial_reputation = ReputationRecord {
        user: user.clone(),
        reliability_score: 80,
        total_payments: 10,
        on_time_payments: 9,
        total_usage: 4000,
        created_at: 1640995200,
        last_updated: 1641081600,
        is_active: true,
    };
    
    env.storage().instance().set(&DataKey::UserReputation(user.clone()), &initial_reputation);
    
    // Update with late payment
    utility_contracts::Contract::update_reputation_score(&env, user.clone(), 1500, false);
    
    let updated_reputation = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    
    assert_eq!(updated_reputation.total_payments, 11);
    assert_eq!(updated_reputation.on_time_payments, 9); // No change
    assert_eq!(updated_reputation.total_usage, 5500);
    assert!(updated_reputation.last_updated > initial_reputation.last_updated);
    
    // Score should decrease (weighted average)
    let payment_ratio = (9 * 100) / 11; // 81
    let expected_score = ((80 * 3) + payment_ratio) / 4; // ~80
    assert_eq!(updated_reputation.reliability_score, expected_score);
}

#[test]
fn test_update_reputation_score_creates_new_record() {
    let env = Env::default();
    let user = Address::random(&env);
    let contract_address = env.register_contract(None, utility_contracts::Contract);
    
    // Update reputation for non-existent record
    utility_contracts::Contract::update_reputation_score(&env, user.clone(), 500, true);
    
    let new_reputation = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    
    assert_eq!(new_reputation.user, user);
    assert_eq!(new_reputation.reliability_score, 50); // Starting score
    assert_eq!(new_reputation.total_payments, 1);
    assert_eq!(new_reputation.on_time_payments, 1);
    assert_eq!(new_reputation.total_usage, 500);
    assert!(new_reputation.is_active);
}

#[test]
fn test_complete_migration_flow() {
    let env = Env::default();
    let user = Address::random(&env);
    let old_contract_address = Address::random(&env);
    let new_contract_address = env.register_contract(None, utility_contracts::Contract);
    
    // Step 1: Create reputation in old contract (simulated)
    let old_reputation = ReputationRecord {
        user: user.clone(),
        reliability_score: 88,
        total_payments: 25,
        on_time_payments: 23,
        total_usage: 10000,
        created_at: 1640995200,
        last_updated: 1641081600,
        is_active: true,
    };
    
    // Simulate storing in old contract
    env.storage().instance().set(&DataKey::UserReputation(user.clone()), &old_reputation);
    
    // Step 2: Export from old contract
    let exported_reputation = utility_contracts::Contract::export_reputation(&env, user.clone());
    
    // Verify old record is burned
    let burned_record = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    assert_eq!(burned_record.is_active, false);
    
    // Step 3: Import to new contract
    let nullifier = BytesN::from_array(&env, &[7; 32]);
    let migration_signature = BytesN::from_array(&env, &[8; 64]);
    
    utility_contracts::Contract::import_reputation(
        &env,
        old_contract_address,
        user.clone(),
        exported_reputation,
        migration_signature,
        nullifier,
    );
    
    // Step 4: Verify migration complete
    let new_reputation = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    
    assert_eq!(new_reputation.is_active, true);
    assert_eq!(new_reputation.reliability_score, 88);
    assert_eq!(new_reputation.total_payments, 25);
    assert_eq!(new_reputation.on_time_payments, 23);
    assert_eq!(new_reputation.total_usage, 10000);
    
    // Verify migration record
    let migration = env.storage().instance()
        .get::<DataKey, ReputationMigration>(&DataKey::ReputationMigration(nullifier))
        .unwrap();
    assert_eq!(migration.old_contract, old_contract_address);
    assert_eq!(migration.new_contract, new_contract_address);
    assert_eq!(migration.user, user);
    
    // Step 5: Continue using reputation in new contract
    utility_contracts::Contract::update_reputation_score(&env, user.clone(), 1200, true);
    
    let final_reputation = env.storage().instance()
        .get::<DataKey, ReputationRecord>(&DataKey::UserReputation(user.clone()))
        .unwrap();
    
    assert_eq!(final_reputation.total_payments, 26);
    assert_eq!(final_reputation.on_time_payments, 24);
    assert_eq!(final_reputation.total_usage, 11200);
}
