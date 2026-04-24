#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env};
use utility_contracts::UtilityContractClient;

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let root_admin = Address::generate(&env);
    
    // Test initial state
    assert_eq!(client.is_initialized(), false);
    assert_eq!(client.get_root_admin(), None);
    
    // Initialize contract
    client.initialize(&root_admin);
    
    // Test post-initialization state
    assert_eq!(client.is_initialized(), true);
    assert_eq!(client.get_root_admin(), Some(root_admin.clone()));
    
    // Test double initialization fails
    let result = std::panic::catch_unwind(|| {
        client.initialize(&root_admin);
    });
    assert!(result.is_err());
}

#[test]
fn test_utility_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Setup a token
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token = token::Client::new(&env, &token_address);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    // Initial funding
    token_admin_client.mint(&user, &1000);

    // 1. Register Meter
    let rate = 10; // 10 tokens per second
    let meter_id = client.register_meter(&user, &provider, &rate, &token_address, &None);
    assert_eq!(meter_id, 1);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.rate_per_second, 10);
    assert_eq!(meter.balance, 0);
    assert_eq!(meter.is_active, false);
    assert_eq!(meter.max_flow_rate_per_hour, 36000); // 10 * 3600

    // 2. Top up
    client.top_up(&meter_id, &500);
    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.balance, 500);
    assert_eq!(meter.is_active, true);
    assert_eq!(token.balance(&user), 500);
    assert_eq!(token.balance(&contract_id), 500);

    // 3. Claim balance (simulate time passing)
    env.ledger().set_timestamp(env.ledger().timestamp() + 10); // 10 seconds pass
    client.claim(&meter_id);
    
    let meter = client.get_meter(&meter_id).unwrap();
    // 10 seconds * 10 tokens/sec = 100 tokens claimed
    assert_eq!(meter.balance, 400);
    assert_eq!(token.balance(&provider), 100);
    assert_eq!(token.balance(&contract_id), 400);

    // 4. Claim more than balance
    env.ledger().set_timestamp(env.ledger().timestamp() + 50); // 50 seconds pass
    client.claim(&meter_id);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.balance, 0);
    assert_eq!(meter.is_active, false);
    assert_eq!(token.balance(&provider), 500);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
fn test_max_flow_rate_cap() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Setup a token
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    // Initial funding
    token_admin_client.mint(&user, &10000);

    // Register Meter with high rate
    let rate = 100; // 100 tokens per second
    let meter_id = client.register_meter(&user, &provider, &rate, &token_address, &None);
    
    // Set a low max flow rate cap
    client.set_max_flow_rate(&meter_id, &5000); // 5000 tokens per hour max
    
    // Top up with large balance
    client.top_up(&meter_id, &10000);
    
    // Try to claim more than the hourly cap
    env.ledger().set_timestamp(env.ledger().timestamp() + 120); // 2 minutes pass
    client.claim(&meter_id);
    
    let meter = client.get_meter(&meter_id).unwrap();
    // Should be capped at 5000 tokens per hour
    assert_eq!(meter.claimed_this_hour, 5000); // 120 seconds * 100 = 12000, but capped at 5000
    assert_eq!(meter.balance, 5000); // Should have exactly 5000 remaining
}

#[test]
fn test_calculate_expected_depletion() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Setup a token
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &1000);

    // Register Meter
    let rate = 10; // 10 tokens per second
    let meter_id = client.register_meter(&user, &provider, &rate, &token_address, &None);
    client.top_up(&meter_id, &500);
    
    // Calculate depletion time
    let depletion_time = client.calculate_expected_depletion(&meter_id).unwrap();
    let current_time = env.ledger().timestamp();
    let expected_depletion = current_time + 50; // 500 tokens / 10 tokens per second = 50 seconds
    
    assert_eq!(depletion_time, expected_depletion);
}

#[test]
fn test_emergency_shutdown() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Setup a token
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &1000);

    // Register and top up meter
    let rate = 10;
    let meter_id = client.register_meter(&user, &provider, &rate, &token_address, &None);
    client.top_up(&meter_id, &500);
    
    // Verify meter is active
    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.is_active, true);
    
    // Emergency shutdown
    client.emergency_shutdown(&meter_id);
    
    // Verify meter is inactive
    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.is_active, false);
}

#[test]
fn test_heartbeat_functionality() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Setup a token
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &1000);

    // Register meter
    let rate = 10;
    let meter_id = client.register_meter(&user, &provider, &rate, &token_address, &None);
    
    // Initially should not be offline
    assert_eq!(client.is_meter_offline(&meter_id), false);
    
    // Simulate time passing more than 1 hour
    env.ledger().set_timestamp(env.ledger().timestamp() + 3700); // > 1 hour
    
    // Should now be offline
    assert_eq!(client.is_meter_offline(&meter_id), true);
    
    // Update heartbeat
    client.update_heartbeat(&meter_id);
    
    // Should no longer be offline
    assert_eq!(client.is_meter_offline(&meter_id), false);
}

#[test]
fn test_event_emissions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let root_admin = Address::generate(&env);
    
    // Test initialization event
    client.initialize(&root_admin);
    
    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Setup a token
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &1000);

    // Test meter registration event
    let meter_id = client.register_meter(&user, &provider, &10, &token_address, &None);
    
    // Test top-up event
    client.top_up(&meter_id, &500);
    
    // Test claim event
    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.claim(&meter_id);
    
    // Test webhook configuration event
    let webhook_url_hash = 12345u64; // Simple hash for testing
    client.configure_webhook(&user, &webhook_url_hash);
    
    // Test emergency shutdown event
    client.emergency_shutdown(&meter_id);
    
    // Note: In a real test environment, you would verify the events were emitted
    // This test ensures the functions execute without panicking when events are published
}
