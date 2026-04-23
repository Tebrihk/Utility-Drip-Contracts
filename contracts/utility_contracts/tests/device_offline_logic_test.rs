// Standalone test for Device-Offline Grace Period Logic
// This test validates the math and state transitions for the new offline features.

#[derive(Clone, Debug, PartialEq)]
enum BillingType { PrePaid, PostPaid }

#[derive(Clone, Debug)]
struct UsageData {
    total_watt_hours: i128,
    precision_factor: i128,
    first_reading_timestamp: u64,
}

#[derive(Clone, Debug)]
struct Meter {
    balance: i128,
    debt: i128,
    billing_type: BillingType,
    rate_per_unit: i128, // nominal rate per second for claim
    off_peak_rate: i128, // cost per unit
    peak_rate: i128,
    last_update: u64,
    last_heartbeat: u64,
    grace_period_start: u64,
    is_offline: bool,
    is_paused: bool,
    is_disputed: bool,
    is_closed: bool,
    estimated_usage_total: i128,
    usage_data: UsageData,
    milestone_deadline: u64,
    milestone_confirmed: bool,
}

const HEARTBEAT_THRESHOLD_SECONDS: u64 = 300; // 5 mins
const GRACE_PERIOD_SECONDS: u64 = 3600; // 1 hour for test
const DEBT_THRESHOLD: i128 = -1000;

fn calculate_historical_average(usage_data: &UsageData, now: u64) -> i128 {
    let elapsed = now.saturating_sub(usage_data.first_reading_timestamp);
    if elapsed == 0 {
        return 0;
    }
    usage_data.total_watt_hours.saturating_mul(usage_data.precision_factor).saturating_div(elapsed as i128)
}

fn get_effective_rate(meter: &Meter, _now: u64) -> i128 {
    // Simplified for test: always off-peak
    meter.off_peak_rate
}

fn settle_claim_logic(meter: &mut Meter, now: u64) -> i128 {
    let elapsed = now.saturating_sub(meter.last_update);
    let mut amount = 0;

    // Device-Offline Grace Period Logic
    if now.saturating_sub(meter.last_heartbeat) > HEARTBEAT_THRESHOLD_SECONDS {
        if !meter.is_offline {
            meter.is_offline = true;
            meter.grace_period_start = meter.last_heartbeat;
        }

        if now.saturating_sub(meter.grace_period_start) <= GRACE_PERIOD_SECONDS {
            // Estimate consumption based on historical averages
            let avg_units_per_second = calculate_historical_average(&meter.usage_data, now);
            let effective_rate = get_effective_rate(meter, now);
            
            let estimated_units = avg_units_per_second.saturating_mul(elapsed as i128).saturating_div(meter.usage_data.precision_factor);
            amount = estimated_units.saturating_mul(effective_rate);
            
            // Track estimated total for later reconciliation
            meter.estimated_usage_total = meter.estimated_usage_total.saturating_add(amount);
        } else {
            // Grace period expired - automatically pause
            meter.is_paused = true;
            amount = 0;
        }
    } else {
        // Device is online - use normal rate
        amount = (elapsed as i128).saturating_mul(meter.rate_per_unit);
    }

    if meter.milestone_deadline > 0 && now > meter.milestone_deadline && !meter.milestone_confirmed {
        amount /= 2;
    }

    let claimable = if amount > meter.balance && meter.balance - amount >= DEBT_THRESHOLD {
        amount
    } else if amount > meter.balance {
        meter.balance - DEBT_THRESHOLD
    } else {
        amount
    };

    meter.balance -= claimable;
    meter.last_update = now;
    claimable
}

fn deduct_units_logic(meter: &mut Meter, now: u64, units_consumed: i128) -> i128 {
    let effective_rate = get_effective_rate(meter, now);
    let discounted_rate = effective_rate; // No discount for simplicity

    if meter.is_offline {
        let estimated_cost = meter.estimated_usage_total;
        // Adjust balance: add back the estimate and let normal deduction handle actual
        meter.balance = meter.balance.saturating_add(estimated_cost);
        
        meter.is_offline = false;
        meter.estimated_usage_total = 0;
        meter.grace_period_start = 0;
    }

    meter.last_heartbeat = now;
    let cost = units_consumed.saturating_mul(discounted_rate);
    meter.balance -= cost;
    meter.last_update = now;
    cost
}

#[test]
fn test_offline_grace_period_and_reconciliation() {
    let mut meter = Meter {
        balance: 10000,
        debt: 0,
        billing_type: BillingType::PrePaid,
        rate_per_unit: 10, // nominal 10 per sec
        off_peak_rate: 1, // 1 per watt-hour
        peak_rate: 2,
        last_update: 1000,
        last_heartbeat: 1000,
        grace_period_start: 0,
        is_offline: false,
        is_paused: false,
        is_disputed: false,
        is_closed: false,
        estimated_usage_total: 0,
        usage_data: UsageData {
            total_watt_hours: 5000, // 5000 WH over 1000 seconds = 5 WH/s
            precision_factor: 1000,
            first_reading_timestamp: 0,
        },
        milestone_deadline: 0,
        milestone_confirmed: false,
    };

    // 1. Online claim at T=1100 (elapsed 100s)
    let claim1 = settle_claim_logic(&mut meter, 1100);
    assert_eq!(claim1, 100 * 10); // 1000
    assert_eq!(meter.balance, 9000);
    assert!(!meter.is_offline);

    // 2. Device goes offline at T=1100.
    // Next claim at T=1500 (elapsed 400s). Heartbeat was 1100. Diff = 400 > 300.
    let claim2 = settle_claim_logic(&mut meter, 1500);
    // Historical avg = 5000 / 1500 = 3.33 units/s? 
    // Wait, first_reading is 0, so elapsed is 1500.
    // 5000 * 1000 / 1500 = 3333 (precision 1000) = 3.333 WH/s
    // Estimated units = 3333 * 400 / 1000 = 1333 WH.
    // Amount = 1333 * 1 (off_peak_rate) = 1333.
    assert_eq!(claim2, 1333);
    assert_eq!(meter.balance, 9000 - 1333);
    assert!(meter.is_offline);
    assert_eq!(meter.estimated_usage_total, 1333);

    // 3. Device reconnects at T=1600 and reports usage for the whole period (1100 to 1600).
    // Let's say actual usage was 1200 WH.
    let cost = deduct_units_logic(&mut meter, 1600, 1200);
    // Reconciliation: balance += 1333, then balance -= 1200 * 1.
    // Balance was 7667. 7667 + 1333 = 9000. 9000 - 1200 = 7800.
    assert_eq!(meter.balance, 7800);
    assert!(!meter.is_offline);
    assert_eq!(meter.estimated_usage_total, 0);

    // 4. Grace period expiry test. 
    // Go offline at T=1600.
    // Claim at T=6000 (elapsed 4400s). Heartbeat was 1600. Diff = 4400 > 300.
    // Grace period starts at T=1600. 6000 - 1600 = 4400 > 3600 (GRACE_PERIOD_SECONDS).
    let _claim3 = settle_claim_logic(&mut meter, 6000);
    assert!(meter.is_paused);
    assert_eq!(meter.last_update, 6000);
}

fn main() {
    // This is just to allow running with cargo or rustc
}
