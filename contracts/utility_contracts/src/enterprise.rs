//! Issues #248–#251: fleet caps, P2P energy exchange, liveness/slashing, priority grid shed.
//! Helpers use `persistent` storage for fleet totals and `temporary` for heartbeat TTL data.

use crate::{panic_with_error, symbol_short, ContractError, ContinuousFlow, DataKey};
use soroban_sdk::{contracttype, token, Address, Bytes, BytesN, Env};

// --- Issue #248: Fleet cap ---

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FleetState {
    /// Sum of active `flow_rate_per_second` for all non-depleted streams (i128, saturating ops).
    pub active_tokens_per_second: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FleetLimitUpdatedEvent {
    pub provider: Address,
    pub old_cap: i128,
    pub new_cap: i128,
}

fn fleet_cap_default_unlimited() -> i128 {
    i128::MAX / 4
}

pub fn fleet_get_active_sum(env: &Env, provider: &Address) -> i128 {
    let key = DataKey::FleetAgg(provider.clone());
    env.storage()
        .persistent()
        .get::<DataKey, FleetState>(&key)
        .map(|s| s.active_tokens_per_second)
        .unwrap_or(0)
}

pub fn fleet_get_cap(env: &Env, provider: &Address) -> i128 {
    env.storage()
        .persistent()
        .get::<DataKey, i128>(&DataKey::FleetCap(provider.clone()))
        .unwrap_or(fleet_cap_default_unlimited())
}

/// Atomically applies delta (may be negative) to fleet aggregate using saturating arithmetic.
pub fn fleet_apply_delta(env: &Env, provider: &Address, delta: i128) {
    if delta == 0 {
        return;
    }
    let key = DataKey::FleetAgg(provider.clone());
    let mut st = env
        .storage()
        .persistent()
        .get::<DataKey, FleetState>(&key)
        .unwrap_or(FleetState {
            active_tokens_per_second: 0,
        });
    st.active_tokens_per_second = st.active_tokens_per_second.saturating_add(delta);
    if st.active_tokens_per_second < 0 {
        st.active_tokens_per_second = 0;
    }
    env.storage().persistent().set(&key, &st);
}

pub fn fleet_assert_room_for_new_stream(env: &Env, provider: &Address, new_stream_rate: i128) {
    if new_stream_rate <= 0 {
        return;
    }
    let cap = fleet_get_cap(env, provider);
    let sum = fleet_get_active_sum(env, provider);
    let next = sum.saturating_add(new_stream_rate);
    if next > cap {
        panic_with_error!(env, ContractError::FleetCapExceeded);
    }
}

pub fn set_fleet_cap_super_admin(env: &Env, provider: Address, new_cap: i128, admin: Address) {
    admin.require_auth();
    let super_a = env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::CurrentAdmin)
        .unwrap_or_else(|| panic_with_error!(env, ContractError::UnauthorizedAdmin));
    let dao = env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::DaoGovernor);
    let ok = admin == super_a || dao.as_ref() == Some(&admin);
    if !ok {
        panic_with_error!(env, ContractError::UnauthorizedAdmin);
    }
    let old = fleet_get_cap(env, &provider);
    env.storage()
        .persistent()
        .set(&DataKey::FleetCap(provider.clone()), &new_cap);
    env.events().publish(
        (symbol_short!("FltLimit"),),
        FleetLimitUpdatedEvent {
            provider: provider.clone(),
            old_cap: old,
            new_cap,
        },
    );
}

// --- Issue #249: P2P adapter ---

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P2PExchangeFinalizedEvent {
    pub supplier: Address,
    pub consumer: Address,
    pub net_consumer_to_supplier: i128,
    pub grid_fee_to_treasury: i128,
    pub ledger: u32,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum P2PRole {
    Supplier = 0,
    Consumer = 1,
}

/// Fixed-point friendly net over `delta_seconds`: supply_rate and demand_rate are tokens/sec (same unit as streams).
pub fn p2p_net_flow_amount(
    supply_rate: i128,
    demand_rate: i128,
    delta_seconds: i128,
) -> i128 {
    let net_rate = supply_rate.saturating_sub(demand_rate);
    net_rate.saturating_mul(delta_seconds)
}

pub fn p2p_finalize_exchange(
    env: &Env,
    supplier: Address,
    consumer: Address,
    utility_treasury: Address,
    supply_rate: i128,
    demand_rate: i128,
    delta_seconds: i128,
    grid_fee_bps: i128,
    battery_credit_cap: i128,
    consumer_token: &Address,
) -> (i128, i128) {
    supplier.require_auth();
    consumer.require_auth();
    if supplier == consumer {
        panic_with_error!(env, ContractError::SelfP2PNotAllowed);
    }
    if grid_fee_bps < 0 || grid_fee_bps > 10_000 {
        panic_with_error!(env, ContractError::InvalidTokenAmount);
    }
    if delta_seconds <= 0 {
        return (0, 0);
    }
    let gross = p2p_net_flow_amount(supply_rate, demand_rate, delta_seconds);
    let trade_volume = gross.abs();
    let grid_fee = if trade_volume > 0 && grid_fee_bps > 0 {
        trade_volume.saturating_mul(grid_fee_bps) / 10_000
    } else {
        0
    };
    let mut credit_delta = gross.saturating_sub(grid_fee);
    let vault_key = DataKey::P2PCreditVault(supplier.clone());
    let mut vault_balance: i128 = env
        .storage()
        .instance()
        .get(&vault_key)
        .unwrap_or(0);

    if credit_delta > 0 {
        // Supplier surplus: cap vault to prevent inflation when battery full.
        let room = battery_credit_cap.saturating_sub(vault_balance);
        if credit_delta > room {
            credit_delta = room;
        }
        vault_balance = vault_balance.saturating_add(credit_delta);
        env.storage().instance().set(&vault_key, &vault_balance);
    } else if credit_delta < 0 {
        let pay = (-credit_delta).min(vault_balance);
        vault_balance = vault_balance.saturating_sub(pay);
        env.storage().instance().set(&vault_key, &vault_balance);
        credit_delta = -pay;
    }

    if grid_fee > 0 {
        let token_client = token::Client::new(env, consumer_token);
        token_client.transfer(&consumer, &utility_treasury, &grid_fee);
    }

    env.events().publish(
        (symbol_short!("P2PFin"),),
        P2PExchangeFinalizedEvent {
            supplier: supplier.clone(),
            consumer: consumer.clone(),
            net_consumer_to_supplier: credit_delta,
            grid_fee_to_treasury: grid_fee,
            ledger: env.ledger().sequence(),
        },
    );

    (credit_delta, grid_fee)
}

// --- Issue #250: Liveness ---

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLivenessBreachedEvent {
    pub stream_id: u64,
    pub blackout_ledgers: u32,
    pub slashed_buffer: i128,
    pub meter_id: u64,
}

pub fn stream_heartbeat(
    env: &Env,
    stream_id: u64,
    meter_id: u64,
    signature: BytesN<64>,
    pub_key: BytesN<32>,
) {
    let mut flow = crate::get_continuous_flow_or_panic(env, stream_id);
    if flow.device_mac_pubkey != pub_key {
        panic_with_error!(env, ContractError::PublicKeyMismatch);
    }
    let mut payload = [0u8; 16];
    payload[..8].copy_from_slice(&stream_id.to_be_bytes());
    payload[8..].copy_from_slice(&meter_id.to_be_bytes());
    let msg = Bytes::from_slice(env, &payload);
    env.crypto().ed25519_verify(&pub_key, &msg, &signature);

    let key = DataKey::StreamLastHeartbeat(stream_id);
    env.storage()
        .temporary()
        .set(&key, &env.ledger().sequence());

    flow.is_unreliable = false;
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
}

/// Called from settlement paths: if heartbeat stale, slash buffer proportionally and optionally mark unreliable.
pub fn liveness_check_and_slash(
    env: &Env,
    stream_id: u64,
    meter_id: u64,
    stale_threshold_ledgers: u32,
) -> i128 {
    if stale_threshold_ledgers == 0 {
        return 0;
    }
    let key = DataKey::StreamLastHeartbeat(stream_id);
    let last: u32 = env.storage().temporary().get(&key).unwrap_or(0);
    let now = env.ledger().sequence();
    let delta = if last == 0 {
        0
    } else {
        now.saturating_sub(last)
    };
    if delta <= stale_threshold_ledgers {
        return 0;
    }

    let mut flow = crate::get_continuous_flow_or_panic(env, stream_id);
    // Proportional to blackout duration beyond threshold (one threshold unit = baseline slash fraction).
    let excess = (delta - stale_threshold_ledgers) as i128;
    let base = stale_threshold_ledgers as i128;
    let slash = flow
        .buffer_balance
        .saturating_mul(excess)
        .saturating_div(base.saturating_add(excess).max(1));
    flow.buffer_balance = flow.buffer_balance.saturating_sub(slash);
    flow.is_unreliable = true;
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);

    env.events().publish(
        (symbol_short!("LiveBrch"),),
        DeviceLivenessBreachedEvent {
            stream_id,
            blackout_ledgers: delta,
            slashed_buffer: slash,
            meter_id,
        },
    );
    slash
}

pub fn pardon_liveness_slash(env: &Env, stream_id: u64, provider: Address) {
    provider.require_auth();
    let mut flow = crate::get_continuous_flow_or_panic(env, stream_id);
    if flow.provider != provider {
        panic_with_error!(env, ContractError::UnauthorizedAdmin);
    }
    flow.is_unreliable = false;
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), &flow);
}

// --- Issue #251: Priority tier + tier-epoch ---

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PriorityTier {
    Critical = 3,
    High = 2,
    Standard = 1,
    Low = 0,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderGridEpoch {
    /// Incremented on each load-shed event (O(1)).
    pub epoch: u64,
    /// Streams with `tier_rank < floor_rank` are subject to shed unless Critical.
    pub floor_rank: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridShortageAlertEvent {
    pub provider: Address,
    pub new_epoch: u64,
    pub floor_tier: u32,
}

pub fn tier_rank(tier: PriorityTier) -> u32 {
    tier as u32
}

pub fn provider_grid_state(env: &Env, provider: &Address) -> ProviderGridEpoch {
    env.storage()
        .instance()
        .get::<DataKey, ProviderGridEpoch>(&DataKey::ProviderGridEpoch(provider.clone()))
        .unwrap_or(ProviderGridEpoch {
            epoch: 0,
            floor_rank: 0,
        })
}

pub fn global_load_shed(env: &Env, provider: Address, minimum_surviving_tier: PriorityTier, grid_admin: Address) {
    grid_admin.require_auth();
    let admin = env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::GridAdministrator)
        .unwrap_or_else(|| panic_with_error!(env, ContractError::UnauthorizedAdmin));
    if grid_admin != admin {
        panic_with_error!(env, ContractError::UnauthorizedAdmin);
    }
    let mut st = provider_grid_state(env, &provider);
    st.epoch = st.epoch.saturating_add(1);
    st.floor_rank = tier_rank(minimum_surviving_tier);
    env.storage()
        .instance()
        .set(&DataKey::ProviderGridEpoch(provider.clone()), &st);

    env.events().publish(
        (symbol_short!("GridAlert"),),
        GridShortageAlertEvent {
            provider: provider.clone(),
            new_epoch: st.epoch,
            floor_tier: minimum_surviving_tier as u32,
        },
    );
}

pub fn stream_should_grid_pause(flow: &ContinuousFlow, grid: &ProviderGridEpoch) -> bool {
    if flow.priority_tier == PriorityTier::Critical as u32 {
        return false;
    }
    if flow.grid_epoch_seen >= grid.epoch {
        return false;
    }
    let tr = flow.priority_tier;
    tr < grid.floor_rank
}

pub fn stream_acknowledge_grid_epoch(env: &Env, stream_id: u64, flow: &mut ContinuousFlow) {
    let grid = provider_grid_state(env, &flow.provider);
    flow.grid_epoch_seen = grid.epoch;
    env.storage()
        .instance()
        .set(&DataKey::ContinuousFlow(stream_id), flow);
}
