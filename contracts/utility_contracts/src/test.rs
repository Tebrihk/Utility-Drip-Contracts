#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env};

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
    let meter_id = client.register_meter(&user, &provider, &rate, &token_address);
    assert_eq!(meter_id, 1);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.rate_per_second, 10);
    assert_eq!(meter.balance, 0);
    assert_eq!(meter.is_active, false);
    assert_eq!(meter.usage_data.total_watt_hours, 0);
    assert_eq!(meter.usage_data.current_cycle_watt_hours, 0);
    assert_eq!(meter.usage_data.peak_usage_watt_hours, 0);
    assert_eq!(meter.usage_data.precision_factor, 1000);

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

    // 5. Test usage tracking
    client.update_usage(&meter_id, &1500); // 1.5 kWh
    let usage_data = client.get_usage_data(&meter_id).unwrap();
    assert_eq!(usage_data.total_watt_hours, 1500000); // 1500 * 1000 precision
    assert_eq!(usage_data.current_cycle_watt_hours, 1500000);
    assert_eq!(usage_data.peak_usage_watt_hours, 1500000);

    // 6. Test cycle reset
    client.reset_cycle_usage(&meter_id);
    let usage_data = client.get_usage_data(&meter_id).unwrap();
    assert_eq!(usage_data.total_watt_hours, 1500000); // Total remains
    assert_eq!(usage_data.current_cycle_watt_hours, 0); // Current cycle reset
    assert_eq!(usage_data.peak_usage_watt_hours, 1500000); // Peak remains

    // 7. Test peak usage update
    client.update_usage(&meter_id, &2000); // 2.0 kWh
    let usage_data = client.get_usage_data(&meter_id).unwrap();
    assert_eq!(usage_data.total_watt_hours, 3500000); // 1500 + 2000
    assert_eq!(usage_data.current_cycle_watt_hours, 2000000); // New cycle
    assert_eq!(usage_data.peak_usage_watt_hours, 2000000); // Updated peak

    // 8. Test display helper function
    let display_total = UtilityContract::get_watt_hours_display(usage_data.total_watt_hours, usage_data.precision_factor);
    assert_eq!(display_total, 3500); // 3500000 / 1000 = 3500 (3.5 kWh)
}
