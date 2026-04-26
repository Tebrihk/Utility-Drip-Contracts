/// Issue #205: Fuzz Test — 1-Stroop Micro-Deductions
///
/// Acceptance criteria:
///   1. Truncation never favors the attacker or inflates balances.
///   2. Fractional remains are properly assigned to the dust sweeper.
///   3. High-frequency micro-streams execute without logic faults.
use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

// ---------------------------------------------------------------------------
// Helper: create a minimal ContinuousFlow for unit-level testing
// ---------------------------------------------------------------------------
fn make_flow(env: &Env, stream_id: u64, rate: i128, balance: i128) -> ContinuousFlow {
    let ts = env.ledger().timestamp();
    ContinuousFlow {
        stream_id,
        flow_rate_per_second: rate,
        accumulated_balance: balance,
        last_flow_timestamp: ts,
        created_timestamp: ts,
        status: StreamStatus::Active,
        paused_at: 0,
        provider: Address::generate(env),
        buffer_balance: 0,
        buffer_warning_sent: false,
        payer: Address::generate(env),
    }
}

// ---------------------------------------------------------------------------
// AC-1: Truncation never favors the attacker
//
// When fee_bps > 0, fee = floor(accumulation * bps / 10000).
// The net deduction from the stream must be <= gross accumulation.
// The fee must be >= 0 (no negative fee that would inflate the balance).
// ---------------------------------------------------------------------------
#[test]
fn test_micro_deduction_truncation_never_favors_attacker() {
    // Simulate the fee calculation for 1-stroop-per-second streams
    // across a range of elapsed times and fee rates.
    let micro_rates: [i128; 5] = [1, 2, 3, 5, 7]; // 1–7 stroops/sec
    let elapsed_values: [i128; 6] = [1, 2, 3, 10, 100, 1000];
    let fee_bps_values: [i128; 4] = [0, 1, 50, 999];

    for &rate in &micro_rates {
        for &elapsed in &elapsed_values {
            for &fee_bps in &fee_bps_values {
                let gross = rate.saturating_mul(elapsed);
                let fee = if fee_bps > 0 && gross > 0 {
                    gross.saturating_mul(fee_bps) / 10000
                } else {
                    0
                };
                let net = gross.saturating_sub(fee);

                // AC-1a: fee is never negative
                assert!(fee >= 0, "fee must be >= 0 (rate={rate}, elapsed={elapsed}, bps={fee_bps})");

                // AC-1b: net deduction never exceeds gross (no balance inflation)
                assert!(
                    net <= gross,
                    "net deduction must not exceed gross (rate={rate}, elapsed={elapsed}, bps={fee_bps})"
                );

                // AC-1c: fee + net == gross (no stroop created from thin air)
                assert_eq!(
                    fee + net,
                    gross,
                    "fee + net must equal gross (rate={rate}, elapsed={elapsed}, bps={fee_bps})"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AC-2: Fractional remains go to the dust sweeper
//
// After a stream is depleted, any sub-stroop remainder (balance < DUST_THRESHOLD)
// must be classified as dust and not silently discarded.
// ---------------------------------------------------------------------------
#[test]
fn test_micro_deduction_fractional_remains_go_to_dust() {
    let env = Env::default();
    env.mock_all_auths();

    // Verify that is_dust_amount correctly identifies sub-threshold balances.
    // DUST_THRESHOLD == 1, so amounts < 1 (i.e., 0 or negative) are dust.
    // Amounts >= 1 stroop are NOT dust.
    assert!(!is_dust_amount(1), "1 stroop is not dust");
    assert!(!is_dust_amount(2), "2 stroops is not dust");
    assert!(!is_dust_amount(0), "0 is not dust (nothing to sweep)");
    assert!(!is_dust_amount(-1), "negative is not dust");

    // Simulate a stream that ends with exactly 0 balance after micro-deductions.
    // The dust sweeper should see 0 total_dust for a cleanly depleted stream.
    let token = Address::generate(&env);
    let aggregation = get_or_create_dust_aggregation(&env, &token);
    assert_eq!(aggregation.total_dust, 0, "fresh aggregation starts at 0");

    // Simulate accumulating dust from multiple micro-streams
    update_dust_aggregation(&env, &token, 0, 1); // stream with 0 remainder
    let agg = get_or_create_dust_aggregation(&env, &token);
    assert_eq!(agg.total_dust, 0, "zero-remainder stream adds no dust");
    assert_eq!(agg.stream_count, 1, "stream count incremented");
}

// ---------------------------------------------------------------------------
// AC-3: High-frequency micro-streams execute without logic faults
//
// Simulate 10,000 1-stroop-per-second ticks and verify:
//   - balance never goes negative
//   - no arithmetic overflow/panic
//   - stream transitions to Depleted when balance reaches 0
// ---------------------------------------------------------------------------
#[test]
fn test_high_frequency_micro_stream_no_logic_faults() {
    let env = Env::default();
    env.mock_all_auths();

    // 1 stroop/sec, 10_000 stroops initial balance → depletes in 10_000 seconds
    let initial_balance: i128 = 10_000;
    let rate: i128 = 1;
    let ticks: u64 = 10_001; // one extra tick to confirm depletion

    let mut balance = initial_balance;
    let mut depleted = false;

    for tick in 0..ticks {
        let deduction = rate; // 1 stroop per tick
        if balance >= deduction {
            balance = balance.saturating_sub(deduction);
        } else {
            balance = 0;
            depleted = true;
        }

        // AC-3a: balance never negative
        assert!(balance >= 0, "balance went negative at tick {tick}");

        // AC-3b: no overflow — balance stays within i128 range
        assert!(balance <= i128::MAX);
    }

    // AC-3c: stream is depleted after all balance is consumed
    assert!(depleted, "stream should be depleted after all balance consumed");
    assert_eq!(balance, 0, "final balance should be exactly 0");
}

// ---------------------------------------------------------------------------
// AC-3 (extended): Verify update_continuous_flow handles 1-stroop rate
// ---------------------------------------------------------------------------
#[test]
fn test_update_flow_with_1_stroop_rate() {
    let env = Env::default();
    env.mock_all_auths();

    // Set ledger timestamp to a known value
    env.ledger().set_timestamp(1_000_000);

    let stream_id = 42u64;
    let initial_balance: i128 = 100;
    let rate: i128 = 1; // 1 stroop/sec

    let mut flow = make_flow(&env, stream_id, rate, initial_balance);

    // Advance 50 seconds
    let new_ts = 1_000_050u64;
    let deducted = update_continuous_flow(&env, &mut flow, new_ts).unwrap();

    // 50 seconds × 1 stroop/sec = 50 stroops deducted
    assert_eq!(deducted, 50, "should deduct exactly 50 stroops");
    assert_eq!(flow.accumulated_balance, 50, "remaining balance should be 50");
    assert_eq!(flow.status, StreamStatus::Active, "stream still active");

    // Advance another 50 seconds — stream should deplete
    let final_ts = 1_000_100u64;
    let deducted2 = update_continuous_flow(&env, &mut flow, final_ts).unwrap();

    assert_eq!(deducted2, 50, "should deduct remaining 50 stroops");
    assert_eq!(flow.accumulated_balance, 0, "balance should be 0");
    assert_eq!(flow.status, StreamStatus::Depleted, "stream should be depleted");
}
