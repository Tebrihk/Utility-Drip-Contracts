#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, BytesN, Env, Vec};

fn device_key(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn create_token(env: &Env) -> Address {
    let admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(admin).address()
}

#[test]
fn test_provider_total_pool_tracks_topups_and_claims() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let user_one = Address::generate(&env);
    let user_two = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_address = create_token(&env);
    let token = token::Client::new(&env, &token_address);
    let token_admin = token::StellarAssetClient::new(&env, &token_address);

    token_admin.mint(&user_one, &10_000);
    token_admin.mint(&user_two, &10_000);

    client.set_tax_rate(&0);

    let meter_one = client.register_meter(
        &user_one,
        &provider,
        &10,
        &token_address,
        &device_key(&env, 1),
    );
    let meter_two = client.register_meter(
        &user_two,
        &provider,
        &10,
        &token_address,
        &device_key(&env, 2),
    );

    client.top_up(&meter_one, &5_000);
    client.top_up(&meter_two, &5_000);

    assert_eq!(client.get_provider_total_pool(&provider), 10_000);

    env.ledger().set_timestamp(env.ledger().timestamp() + 5);
    client.claim(&meter_one);

    let window = client.get_provider_window(&provider).unwrap();
    assert_eq!(window.daily_withdrawn, 50);
    assert_eq!(client.get_provider_total_pool(&provider), 9_950);
    assert_eq!(token.balance(&provider), 50);

    env.ledger().set_timestamp(env.ledger().timestamp() + 5);
    client.claim(&meter_two);

    let window = client.get_provider_window(&provider).unwrap();
    assert_eq!(window.daily_withdrawn, 100);
    assert_eq!(client.get_provider_total_pool(&provider), 9_900);
    assert_eq!(token.balance(&provider), 100);
}

#[test]
fn test_batch_withdraw_all_claims_active_provider_streams_for_one_token() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let user_one = Address::generate(&env);
    let user_two = Address::generate(&env);
    let user_three = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_address = create_token(&env);
    let other_token = create_token(&env);
    let token = token::Client::new(&env, &token_address);
    let token_admin = token::StellarAssetClient::new(&env, &token_address);
    let other_token_admin = token::StellarAssetClient::new(&env, &other_token);

    token_admin.mint(&user_one, &10_000);
    token_admin.mint(&user_two, &10_000);
    other_token_admin.mint(&user_three, &10_000);

    let meter_one = client.register_meter(
        &user_one,
        &provider,
        &10,
        &token_address,
        &device_key(&env, 1),
    );
    let meter_two = client.register_meter(
        &user_two,
        &provider,
        &10,
        &token_address,
        &device_key(&env, 2),
    );
    let other_meter = client.register_meter(
        &user_three,
        &provider,
        &10,
        &other_token,
        &device_key(&env, 3),
    );

    client.top_up(&meter_one, &5_000);
    client.top_up(&meter_two, &5_000);
    client.top_up(&other_meter, &5_000);

    env.ledger().set_timestamp(env.ledger().timestamp() + 5);
    let result = client.batch_withdraw_all(&provider, &token_address);

    assert_eq!(result.token, token_address);
    assert_eq!(result.streams_scanned, 2);
    assert_eq!(result.streams_withdrawn, 2);
    assert_eq!(result.total_gross_claimed, 100);
    assert_eq!(result.total_provider_payout, 95);
    assert_eq!(result.total_tax_withheld, 5);
    assert_eq!(result.total_protocol_fee, 0);

    assert_eq!(token.balance(&provider), 95);
    assert_eq!(client.get_provider_total_pool(&provider), 14_900);

    let meter_one = client.get_meter(&meter_one).unwrap();
    let meter_two = client.get_meter(&meter_two).unwrap();
    let other_meter = client.get_meter(&other_meter).unwrap();

    assert_eq!(meter_one.balance, 4_950);
    assert_eq!(meter_two.balance, 4_950);
    assert_eq!(other_meter.balance, 5_000);
}

#[test]
fn test_batch_register_meters_creates_all_records() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let token_address = create_token(&env);

    let mut meter_infos = Vec::new(&env);
    meter_infos.push_back(MeterInfo {
        user: Address::generate(&env),
        provider: provider.clone(),
        off_peak_rate: 100,
        token: token_address.clone(),
        billing_type: BillingType::PrePaid,
        device_public_key: device_key(&env, 10),
    });
    meter_infos.push_back(MeterInfo {
        user: Address::generate(&env),
        provider: provider.clone(),
        off_peak_rate: 200,
        token: token_address.clone(),
        billing_type: BillingType::PostPaid,
        device_public_key: device_key(&env, 11),
    });

    let batch = client.batch_register_meters(&meter_infos);
    assert_eq!(batch.start_id, 1);
    assert_eq!(batch.end_id, 2);
    assert_eq!(batch.count, 2);

    let first = client.get_meter(&1).unwrap();
    let second = client.get_meter(&2).unwrap();
    assert_eq!(first.off_peak_rate, 100);
    assert_eq!(first.priority_index, 0);
    assert_eq!(second.off_peak_rate, 200);
    assert_eq!(second.billing_type, BillingType::PostPaid);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_batch_register_meters_empty_vector_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);
    let empty = Vec::<MeterInfo>::new(&env);

    client.batch_register_meters(&empty);
}
