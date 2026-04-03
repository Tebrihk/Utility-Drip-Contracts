#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, BytesN, Env, Vec};

// --- Helpers ---
fn device_key(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn create_token(env: &Env) -> Address {
    let admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(admin).address()
}

// ==================== MOCK CONTRACTS ====================

mod mock_sorosusu {
    use soroban_sdk::{contract, contractimpl, Address, Env};

    #[contract]
    pub struct MockSoroSusu;

    #[contractimpl]
    impl MockSoroSusu {
        pub fn set_default(env: Env, user: Address, in_default: bool) {
            env.storage().instance().set(&user, &in_default);
        }

        pub fn is_in_default(env: Env, user: Address) -> bool {
            env.storage().instance().get(&user).unwrap_or(false)
        }

        pub fn is_trusted_saver(_env: Env, _user: Address) -> bool { false }
        pub fn get_susu_score(_env: Env, _user: Address) -> u32 { 0 }

        pub fn record_debt_payment(env: Env, user: Address, amount: i128) {
            let key = (user.clone(), soroban_sdk::symbol_short!("paid"));
            let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &current.saturating_add(amount));
        }
    }
}

// ==================== CORE UTILITY TESTS ====================

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

    let meter_one = client.register_meter(&user_one, &provider, &10, &token_address, &device_key(&env, 1));
    let meter_two = client.register_meter(&user_two, &provider, &10, &token_address, &device_key(&env, 2));

    client.top_up(&meter_one, &5_000);
    client.top_up(&meter_two, &5_000);

    // Verify initial pool
    assert_eq!(client.get_provider_total_pool(&provider), 10_000);

    env.ledger().set_timestamp(100);
    client.claim(&meter_one);

    let window = client.get_provider_window(&provider).unwrap();
    assert!(window.daily_withdrawn > 0);
}

#[test]
fn test_batch_withdraw_all_claims_active_provider_streams() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let user_one = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_address = create_token(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token_address);

    token_admin.mint(&user_one, &10_000);
    client.set_tax_rate(&500); // 5% tax

    let meter_one = client.register_meter(&user_one, &provider, &10, &token_address, &device_key(&env, 1));
    client.top_up(&meter_one, &5_000);

    env.ledger().set_timestamp(10);
    let result = client.batch_withdraw_all(&provider, &token_address);

    assert_eq!(result.streams_withdrawn, 1);
    assert!(result.total_tax_withheld > 0);
}

// ==================== INTER-PROTOCOL DEBT SERVICE TESTS ====================

#[test]
fn test_service_sorosusu_debt_diverts_funds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let susu_id = env.register_contract(None, mock_sorosusu::MockSoroSusu);
    let susu_client = mock_sorosusu::MockSoroSusuClient::new(&env, &susu_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_address = create_token(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token_address);

    token_admin.mint(&user, &100_000);
    client.set_tax_rate(&0);
    client.set_sorosusu_contract(&susu_id);

    let meter_id = client.register_meter(&user, &provider, &10, &token_address, &device_key(&env, 42));
    client.top_up(&meter_id, &100_000);

    // Generate maintenance fund via claim
    env.ledger().set_timestamp(1_000);
    client.claim(&meter_id);

    let fund_before = client.get_maintenance_fund(&meter_id);
    susu_client.set_default(&user, &true);

    client.service_sorosusu_debt(&meter_id);

    let fund_after = client.get_maintenance_fund(&meter_id);
    assert!(fund_after < fund_before);
}

// ==================== PROVIDER RELIABILITY TESTS ====================

#[test]
fn test_reliability_score_logic() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);
    let provider = Address::generate(&env);

    // Report 99 online windows out of 100
    for _ in 0..99u32 {
        client.report_provider_uptime(&provider, &true);
    }
    client.report_provider_uptime(&provider, &false);

    let score = client.get_reliability_score(&provider).unwrap();
    assert_eq!(score.score_bps, 9900);
    assert_eq!(score.badge, ReliabilityBadge::Gold);
}

#[test]
fn test_reliability_score_reset_impact() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);
    let provider = Address::generate(&env);

    // 10 online, then 10 offline
    for _ in 0..10u32 { client.report_provider_uptime(&provider, &true); }
    for _ in 0..10u32 { client.report_provider_uptime(&provider, &false); }

    let score = client.get_reliability_score(&provider).unwrap();
    assert_eq!(score.windows_total, 20);
    assert_eq!(score.score_bps, 5000);
    assert_eq!(score.badge, ReliabilityBadge::None);
}