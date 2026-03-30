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

#[test]
fn test_trust_mode_boundary_72_hours() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let contract_id = env.register_contract(None, UtilityContract);
    let client = UtilityContractClient::new(&env, &contract_id);

    let oracle_id = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_id);
    client.set_oracle(&oracle_id);

    let now = env.ledger().timestamp();
    oracle_client.set_price(&100, &2, &(now - 72 * 60 * 60));
    assert!(!client.is_trust_mode());

    oracle_client.set_price(&100, &2, &(now - 72 * 60 * 60 - 1));
    assert!(client.is_trust_mode());
}

#[test]
#[should_panic(expected = "Error(Contract, #52)")]
fn test_healthy_oracle_blocks_emergency_proposal() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, member) = setup_meter_and_oracle(&env, false);
    client.register_active_user(&member);

    client.propose_emergency_flow_rate(&member, &meter_id, &9999);
}

#[test]
#[should_panic(expected = "Error(Contract, #56)")]
fn test_unanimous_approval_required_before_execute() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, member_one) = setup_meter_and_oracle(&env, true);
    let member_two = Address::generate(&env);

    client.register_active_user(&member_one);
    client.register_active_user(&member_two);

    let proposal_id = client.propose_emergency_flow_rate(&member_one, &meter_id, &7777);
    client.execute_emergency_action(&member_one, &proposal_id);
}

#[test]
fn test_emergency_flow_rate_executes_after_unanimous_vote() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, member_one) = setup_meter_and_oracle(&env, true);
    let member_two = Address::generate(&env);

    client.register_active_user(&member_one);
    client.register_active_user(&member_two);

    let proposal_id = client.propose_emergency_flow_rate(&member_one, &meter_id, &8888);
    client.approve_emergency_action(&member_two, &proposal_id);
    client.execute_emergency_action(&member_one, &proposal_id);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.max_flow_rate_per_hour, 8888);
}

#[test]
#[should_panic(expected = "Error(Contract, #53)")]
fn test_non_member_cannot_propose_emergency_action() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, _registered_member) = setup_meter_and_oracle(&env, true);
    let non_member = Address::generate(&env);

    client.propose_emergency_pause(&non_member, &meter_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_duplicate_emergency_vote_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, member_one) = setup_meter_and_oracle(&env, true);
    let member_two = Address::generate(&env);
    client.register_active_user(&member_one);
    client.register_active_user(&member_two);

    let proposal_id = client.propose_emergency_pause(&member_one, &meter_id);
    client.approve_emergency_action(&member_two, &proposal_id);
    client.approve_emergency_action(&member_two, &proposal_id);
}

#[test]
fn test_emergency_pause_executes_after_unanimous_vote() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, member_one) = setup_meter_and_oracle(&env, true);
    let member_two = Address::generate(&env);

    client.register_active_user(&member_one);
    client.register_active_user(&member_two);

    let proposal_id = client.propose_emergency_pause(&member_one, &meter_id);
    client.approve_emergency_action(&member_two, &proposal_id);
    client.execute_emergency_action(&member_one, &proposal_id);

    let meter = client.get_meter(&meter_id).unwrap();
    assert!(meter.is_paused);
}

#[test]
#[should_panic(expected = "Error(Contract, #52)")]
fn test_oracle_recovery_blocks_new_emergency_actions() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, member_one) = setup_meter_and_oracle(&env, true);
    client.register_active_user(&member_one);

    let oracle_addr = client.get_current_rate();
    assert!(oracle_addr.is_some());

    let oracle_id = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_id);
    client.set_oracle(&oracle_id);
    oracle_client.set_price(&100, &2, &env.ledger().timestamp());

    client.propose_emergency_pause(&member_one, &meter_id);
}

#[test]
fn test_single_member_unanimity_allows_execution() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let (client, _token, meter_id, member_one) = setup_meter_and_oracle(&env, true);
    client.register_active_user(&member_one);

    let proposal_id = client.propose_emergency_flow_rate(&member_one, &meter_id, &4242);
    client.execute_emergency_action(&member_one, &proposal_id);

    let meter = client.get_meter(&meter_id).unwrap();
    assert_eq!(meter.max_flow_rate_per_hour, 4242);
}
