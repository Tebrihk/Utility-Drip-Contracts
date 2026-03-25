use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

#[test]
fn test_extreme_usage_values() {
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
    let token_admin_client =
        soroban_sdk::token::StellarAssetClient::new(&env, &token_address);
    token_admin_client.mint(&user, &1_000_000_000_000i128);

    let device_public_key = BytesN::from_array(&env, &[1u8; 32]);
    let meter_id =
        client.register_meter(&user, &provider, &100, &token_address, &device_public_key);

    client.top_up(&meter_id, &1_000_000_000_000i128);

    // Test large (but valid) usage updates
    let extreme_values: [i128; 3] = [
        1_000_000_000i128,
        10_000_000_000i128,
        100_000_000_000i128,
    ];

    for &usage in extreme_values.iter() {
        client.update_usage(&meter_id, &usage);
        let usage_data = client.get_usage_data(&meter_id);
        assert!(usage_data.is_some());
        let data = usage_data.unwrap();
        assert!(data.total_watt_hours >= 0);
        assert!(data.current_cycle_watt_hours >= 0);
        assert!(data.peak_usage_watt_hours >= 0);
    }
}

#[test]
fn test_precision_factor_extremes() {
    let extreme_precision_factors: [i128; 5] = [
        1,
        1000,
        1_000_000,
        1_000_000_000,
        i128::MAX / 1000,
    ];

    let test_usage = 1_000_000_000i128;

    for &precision in extreme_precision_factors.iter() {
        let precise_consumption = test_usage.saturating_mul(precision);
        assert!(precise_consumption >= 0);

        if precision != 0 {
            let display = test_usage / precision;
            assert!(display >= 0);
        }
    }
}

#[test]
fn test_arithmetic_edge_cases() {
    let edge_cases: [i128; 7] = [
        i128::MAX,
        i128::MIN,
        i128::MAX - 1,
        i128::MIN + 1,
        0,
        -1,
        1,
    ];

    for &value in edge_cases.iter() {
        let _a = value.saturating_add(1);
        let _b = value.saturating_mul(1000);
        let _c = value.saturating_sub(1);

        if value != 0 {
            let _d = 1000i128 / value;
        }
    }
}

#[test]
fn test_cumulative_extreme_usage() {
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
    let token_admin_client =
        soroban_sdk::token::StellarAssetClient::new(&env, &token_address);
    token_admin_client.mint(&user, &i128::MAX);

    let device_public_key = BytesN::from_array(&env, &[1u8; 32]);
    let meter_id =
        client.register_meter(&user, &provider, &100, &token_address, &device_public_key);

    client.top_up(&meter_id, &1_000_000_000_000i128);

    let extreme_usage = 1_000_000_000i128;

    for i in 0u64..10 {
        let cumulative_usage = extreme_usage.saturating_mul((i + 1) as i128);
        client.update_usage(&meter_id, &cumulative_usage);

        let usage_data = client.get_usage_data(&meter_id);
        assert!(usage_data.is_some());

        let data = usage_data.unwrap();
        assert!(data.total_watt_hours >= 0);
        assert!(data.current_cycle_watt_hours >= 0);
        assert!(data.peak_usage_watt_hours >= 0);
    }
}
