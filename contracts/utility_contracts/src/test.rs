#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, token, Address, BytesN, Env, Vec};

#[contracttype]
#[derive(Clone)]
enum OracleDataKey {
    Price,
}

#[contract]
struct MockOracle;

#[contractimpl]
impl MockOracle {
    fn set_price(env: Env, price: i128, decimals: u32, last_updated: u64) {
        let data = PriceData {
            price,
            decimals,
            last_updated,
        };
        env.storage().instance().set(&OracleDataKey::Price, &data);
    }

    fn get_price(env: Env) -> PriceData {
        env.storage()
            .instance()
            .get(&OracleDataKey::Price)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::OracleNotSet))
    }

    fn xlm_to_usd_cents(env: Env, xlm_amount: i128) -> i128 {
        let price = Self::get_price(env).price;
        xlm_amount.saturating_mul(price)
    }

    fn usd_cents_to_xlm(env: Env, usd_cents: i128) -> i128 {
        let price = Self::get_price(env).price;
        if price <= 0 {
            0
        } else {
            usd_cents / price
        }
    }
}

fn setup_meter_and_oracle(env: &Env, stale: bool) -> (UtilityContractClient<'_>, Address, u64, Address) {
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(env, &contract_id);

    let user = Address::generate(env);
    let provider = Address::generate(env);
    let token_address = create_token(env);
    let meter_id = client.register_meter(&user, &provider, &10, &token_address, &device_key(env, 3));

    let oracle_id = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(env, &oracle_id);
    client.set_oracle(&oracle_id);

    let now = env.ledger().timestamp();
    let heartbeat = if stale {
        now.saturating_sub(72 * 60 * 60 + 1)
    } else {
        now.saturating_sub(60)
    };
    oracle_client.set_price(&100, &2, &heartbeat);

    (client, token_address, meter_id, user)
}

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

// ==================== INTER-PROTOCOL DEBT SERVICE TESTS ====================

/// Minimal mock SoroSusu contract used only in tests.
/// Stores a single boolean per user: whether they are in default.
#[cfg(test)]
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

        pub fn is_trusted_saver(_env: Env, _user: Address) -> bool {
            false
        }

        pub fn get_susu_score(_env: Env, _user: Address) -> u32 {
            0
        }

        /// Records a debt payment — in tests we just store the cumulative amount.
        pub fn record_debt_payment(env: Env, user: Address, amount: i128) {
            let key = (user.clone(), soroban_sdk::symbol_short!("paid"));
            let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage()
                .instance()
                .set(&key, &current.saturating_add(amount));
        }
    }
}

#[test]
fn test_service_sorosusu_debt_diverts_5_percent_of_maintenance_fund() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy utility contract
    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    // Deploy mock SoroSusu
    let susu_id = env.register_contract(None, mock_sorosusu::MockSoroSusu);
    let susu_client = mock_sorosusu::MockSoroSusuClient::new(&env, &susu_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_address = create_token(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token_address);

    token_admin.mint(&user, &100_000);

    // Register and fund a meter so the maintenance fund accumulates
    client.set_tax_rate(&0);
    let meter_id = client.register_meter(&user, &provider, &10, &token_address, &device_key(&env, 42));
    client.top_up(&meter_id, &100_000);

    // Manually seed the maintenance fund (simulates accumulated 0.01% allocations)
    // We do this by advancing time and claiming so allocate_to_maintenance_fund runs,
    // but for simplicity we verify the function logic directly via a known fund balance.
    // Seed: inject 10_000 into the maintenance fund via a claim cycle.
    env.ledger().set_timestamp(env.ledger().timestamp() + 1_000);
    client.claim(&meter_id);

    let fund_before = client.get_maintenance_fund(&meter_id);
    // fund_before should be > 0 (1 bps of claimed amount)
    assert!(fund_before > 0, "maintenance fund should be non-zero after claim");

    // Wire up SoroSusu contract
    set_sorosusu_contract(env.clone(), susu_id.clone());

    // Mark user as in default on SoroSusu
    susu_client.set_default(&user, &true);

    // Trigger debt service
    client.service_sorosusu_debt(&meter_id);

    let fund_after = client.get_maintenance_fund(&meter_id);
    let expected_divert = (fund_before * 500) / 10_000; // 5%
    assert_eq!(
        fund_before - fund_after,
        expected_divert,
        "exactly 5% should be diverted from the maintenance fund"
    );

    // Debt service record should be persisted
    let record = client.get_debt_service_record(&meter_id).unwrap();
    assert_eq!(record.meter_id, meter_id);
    assert_eq!(record.user, user);
    assert_eq!(record.amount_diverted, expected_divert);
}

#[test]
fn test_service_sorosusu_debt_noop_when_user_not_in_default() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let susu_id = env.register_contract(None, mock_sorosusu::MockSoroSusu);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_address = create_token(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token_address);
    token_admin.mint(&user, &100_000);

    client.set_tax_rate(&0);
    let meter_id = client.register_meter(&user, &provider, &10, &token_address, &device_key(&env, 43));
    client.top_up(&meter_id, &100_000);

    env.ledger().set_timestamp(env.ledger().timestamp() + 1_000);
    client.claim(&meter_id);

    let fund_before = client.get_maintenance_fund(&meter_id);

    set_sorosusu_contract(env.clone(), susu_id.clone());
    // user is NOT in default — no diversion should happen

    client.service_sorosusu_debt(&meter_id);

    let fund_after = client.get_maintenance_fund(&meter_id);
    assert_eq!(fund_before, fund_after, "fund should be unchanged when user is not in default");
    assert!(client.get_debt_service_record(&meter_id).is_none());
}

#[test]
fn test_service_sorosusu_debt_noop_when_sorosusu_not_configured() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let provider = Address::generate(&env);
    let token_address = create_token(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token_address);
    token_admin.mint(&user, &100_000);

    client.set_tax_rate(&0);
    let meter_id = client.register_meter(&user, &provider, &10, &token_address, &device_key(&env, 44));
    client.top_up(&meter_id, &100_000);

    env.ledger().set_timestamp(env.ledger().timestamp() + 1_000);
    client.claim(&meter_id);

    let fund_before = client.get_maintenance_fund(&meter_id);

    // SoroSusu contract is NOT configured — should be a silent no-op
    client.service_sorosusu_debt(&meter_id);

    assert_eq!(fund_before, client.get_maintenance_fund(&meter_id));
}
