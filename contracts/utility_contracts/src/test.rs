#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env};

#[test]
fn test_prepaid_meter_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let oracle = Address::generate(&env);
    client.set_oracle(&oracle);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token = token::Client::new(&env, &token_address);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    // Initial funding - provide enough for minimum balance tests
    token_admin_client.mint(&user, &1000); // 1000 tokens

    let meter_id = client.register_meter(&user, &provider, &10, &token_address);
    assert_eq!(meter_id, 1);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.billing_type, BillingType::PrePaid);
    assert_eq!(meter.rate_per_second, 10);
    assert_eq!(meter.balance, 0);
    assert_eq!(meter.is_active, false);
    assert_eq!(meter.usage_data.total_watt_hours, 0);
    assert_eq!(meter.usage_data.current_cycle_watt_hours, 0);
    assert_eq!(meter.usage_data.peak_usage_watt_hours, 0);
    assert_eq!(meter.usage_data.precision_factor, 1000);
    assert_eq!(meter.max_flow_rate_per_hour, 36000); // 10 * 3600

    // 2. Top up with minimum balance
    client.top_up(&meter_id, &500); // 500 tokens - meets minimum
    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.balance, 500);
    assert_eq!(meter.is_active, true);
    assert_eq!(token.balance(&user), 500); // 1000 - 500 = 500 remaining
    assert_eq!(token.balance(&contract_id), 500);

    // 3. Report usage (billing by units)
    let units_consumed = 15; // 15 kWh
    client.deduct_units(&meter_id, &units_consumed);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.balance, 350); // 500 - 150 = 350
    assert_eq!(meter.is_active, false); // Below minimum (350 < 500)
    assert_eq!(token.balance(&provider), 150); // 150 tokens claimed
    assert_eq!(token.balance(&contract_id), 350);

    client.update_usage(&meter_id, &1500);
    let usage_data = client.get_usage_data(&meter_id).unwrap();
    assert_eq!(usage_data.total_watt_hours, 1_500_000);
    assert_eq!(usage_data.current_cycle_watt_hours, 1_500_000);
    assert_eq!(usage_data.peak_usage_watt_hours, 1_500_000);

    client.reset_cycle_usage(&meter_id);
    let usage_data = client.get_usage_data(&meter_id).unwrap();
    assert_eq!(usage_data.total_watt_hours, 1_500_000);
    assert_eq!(usage_data.current_cycle_watt_hours, 0);
    assert_eq!(usage_data.peak_usage_watt_hours, 1_500_000);

    client.update_usage(&meter_id, &2000);
    let usage_data = client.get_usage_data(&meter_id).unwrap();
    assert_eq!(usage_data.total_watt_hours, 3_500_000);
    assert_eq!(usage_data.current_cycle_watt_hours, 2_000_000);
    assert_eq!(usage_data.peak_usage_watt_hours, 2_000_000);

    // 8. Test display helper function
    let display_total = UtilityContract::get_watt_hours_display(usage_data.total_watt_hours, usage_data.precision_factor);
    assert_eq!(display_total, 3500); // 3500000 / 1000 = 3500 (3.5 kWh)

    // 9. Test minimum balance functionality
    let min_balance = client.get_minimum_balance_to_flow();
    assert_eq!(min_balance, 500); // 500 tokens minimum

    // Test small top-up that doesn't meet minimum
    let meter_id_2 = client.register_meter(&user, &provider, &rate, &token_address);
    client.top_up(&meter_id_2, &100); // 100 tokens - below minimum
    let meter_2 = client.get_meter(&meter_id_2).unwrap();
    assert_eq!(meter_2.balance, 100);
    assert_eq!(meter_2.is_active, false); // Should not be active

    // Test top-up that meets minimum
    client.top_up(&meter_id_2, &400); // Add 400 tokens more = 500 total
    let meter_2 = client.get_meter(&meter_id_2).unwrap();
    assert_eq!(meter_2.balance, 500);
    assert_eq!(meter_2.is_active, true); // Should now be active

    // Test claim that drops below minimum
    env.ledger().set_timestamp(env.ledger().timestamp() + 10); // 10 seconds pass
    client.claim(&meter_id_2); // This should claim 100 tokens (10 * 10)
    let meter_2 = client.get_meter(&meter_id_2).unwrap();
    assert_eq!(meter_2.balance, 400); // 500 - 100 = 400
    assert_eq!(meter_2.is_active, false); // Should be deactivated
}

#[test]
fn test_max_flow_rate_cap() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let oracle = Address::generate(&env);
    client.set_oracle(&oracle);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &10_000);

    let meter_id = client.register_meter(&user, &provider, &100, &token_address);
    client.set_max_flow_rate(&meter_id, &5000);
    client.top_up(&meter_id, &10_000);
    client.deduct_units(&meter_id, &120);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.claimed_this_hour, 5000);
    assert_eq!(meter.balance, 5000);
}

#[test]
fn test_calculate_expected_depletion() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &1000);

    let meter_id = client.register_meter(&user, &provider, &10, &token_address);
    client.top_up(&meter_id, &500);

    let depletion_time = client.calculate_expected_depletion(&meter_id).unwrap();
    let current_time = env.ledger().timestamp();
    assert_eq!(depletion_time, current_time + 50);
}

#[test]
fn test_emergency_shutdown() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &1000);

    let meter_id = client.register_meter(&user, &provider, &10, &token_address);
    client.top_up(&meter_id, &500);

    let meter = client.get_meter(&meter_id).unwrap();
    assert!(meter.is_active);

    client.emergency_shutdown(&meter_id);

    let meter = client.get_meter(&meter_id).unwrap();
    assert!(!meter.is_active);
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
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    token_admin_client.mint(&user, &1000);

    // Register meter
    let rate = 10;
    let meter_id = client.register_meter(&user, &provider, &rate, &token_address);
    
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
fn test_carbon_credit_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Setup default token
    let default_token_admin = Address::generate(&env);
    let default_token_address = env.register_stellar_asset_contract(default_token_admin.clone());
    
    // Setup Carbon Credit Token (e.g., AQUA/Eco-Token)
    let eco_token_admin = Address::generate(&env);
    let eco_token_address = env.register_stellar_asset_contract(eco_token_admin.clone());
    let eco_token = token::Client::new(&env, &eco_token_address);
    let eco_token_admin_client = token::StellarAssetClient::new(&env, &eco_token_address);

    // Initial funding of Carbon Credits
    eco_token_admin_client.mint(&user, &2000); // 2000 Eco-Tokens

    // 1. Register Meter with default token
    let rate = 10;
    let meter_id = client.register_meter(&user, &provider, &rate, &default_token_address);

    // 2. Add Carbon Credit token as supported
    client.add_supported_token(&eco_token_address);

    // 3. Top up using Carbon Credits
    client.top_up_with_token(&meter_id, &1000, &eco_token_address);

    // 4. Verify the meter balance increased
    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.balance, 1000);
    assert_eq!(meter.is_active, true);

    // 5. Verify the Carbon Credits were BURNED (balance should be 1000 remaining)
    assert_eq!(eco_token.balance(&user), 1000);
    
    // The contract itself should have 0 eco_tokens because they were correctly burned
    assert_eq!(eco_token.balance(&contract_id), 0);
}

#[test]
#[should_panic]
fn test_unsupported_token_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(UtilityContract, ());
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    
    let default_token_admin = Address::generate(&env);
    let default_token_address = env.register_stellar_asset_contract(default_token_admin.clone());
    
    let bad_token_admin = Address::generate(&env);
    let bad_token_address = env.register_stellar_asset_contract(bad_token_admin.clone());
    let bad_token_admin_client = token::StellarAssetClient::new(&env, &bad_token_address);
    bad_token_admin_client.mint(&user, &2000);

    let rate = 10;
    let meter_id = client.register_meter(&user, &provider, &rate, &default_token_address);

    // Should panic because bad_token_address is not supported
    client.top_up_with_token(&meter_id, &1000, &bad_token_address);
}
