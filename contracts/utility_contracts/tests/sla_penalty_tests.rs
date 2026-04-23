// Standalone test for SLA Penalty Hooks
// This test validates the math and state transitions for SLA penalties.

#[derive(Clone, Debug, PartialEq)]
enum BillingType { PrePaid, PostPaid }

#[derive(Clone, Debug)]
struct SLAConfig {
    pub threshold_seconds: u64,
    pub penalty_multiplier_bps: i128,
}

#[derive(Clone, Debug)]
struct SLAState {
    pub accumulated_downtime: u64,
    pub last_report_timestamp: u64,
    pub is_penalty_active: bool,
}

#[derive(Clone, Debug)]
struct Meter {
    pub balance: i128,
    pub rate_per_unit: i128,
    pub sla_config: Option<SLAConfig>,
    pub sla_state: SLAState,
    pub last_update: u64,
}

fn settle_claim_logic(meter: &mut Meter, now: u64) -> i128 {
    let elapsed = now.saturating_sub(meter.last_update);
    let mut amount = (elapsed as i128).saturating_mul(meter.rate_per_unit);

    if let Some(config) = &meter.sla_config {
        // Automatic reversion if service stabilizes (no reports for 2x threshold)
        let stability_window = config.threshold_seconds.saturating_mul(2);
        if now.saturating_sub(meter.sla_state.last_report_timestamp) > stability_window {
            meter.sla_state.accumulated_downtime = 0;
            meter.sla_state.is_penalty_active = false;
        }
        
        if meter.sla_state.accumulated_downtime >= config.threshold_seconds {
            meter.sla_state.is_penalty_active = true;
            // The penalty mathematics do not cause underflow panics
            amount = amount.saturating_mul(config.penalty_multiplier_bps).saturating_div(10000);
        } else {
            meter.sla_state.is_penalty_active = false;
        }
    }

    meter.balance -= amount;
    meter.last_update = now;
    amount
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sla_penalty_application_and_reversion() {
        let mut meter = Meter {
            balance: 100000,
            rate_per_unit: 10,
            sla_config: Some(SLAConfig {
                threshold_seconds: 3600, // 1 hour
                penalty_multiplier_bps: 5000, // 50% discount
            }),
            sla_state: SLAState {
                accumulated_downtime: 0,
                last_report_timestamp: 1000,
                is_penalty_active: false,
            },
            last_update: 1000,
        };

        // 1. Normal claim (no downtime)
        let claim1 = settle_claim_logic(&mut meter, 2000);
        assert_eq!(claim1, 1000 * 10); // 10000
        assert_eq!(meter.balance, 90000);
        assert!(!meter.sla_state.is_penalty_active);

        // 2. Add downtime (2 hours) - above threshold
        meter.sla_state.accumulated_downtime = 7200;
        meter.sla_state.last_report_timestamp = 2000;
        
        // Claim should be penalized
        let claim2 = settle_claim_logic(&mut meter, 3000);
        // 1000s * 10 = 10000. Penalized by 50% = 5000.
        assert_eq!(claim2, 5000);
        assert_eq!(meter.balance, 85000);
        assert!(meter.sla_state.is_penalty_active);

        // 3. Stabilization (no reports for 2x threshold = 7200s)
        // Last report was at 2000. Stability reached at 2000 + 7200 = 9200.
        let claim3 = settle_claim_logic(&mut meter, 10000);
        // Elapsed = 10000 - 3000 = 7000.
        // Stability reached (10000 > 9200), penalty reset.
        assert_eq!(claim3, 7000 * 10);
        assert!(!meter.sla_state.is_penalty_active);
        assert_eq!(meter.sla_state.accumulated_downtime, 0);
    }

    #[test]
    fn test_conflicting_reports_simulated() {
        let mut meter = Meter {
            balance: 10000,
            rate_per_unit: 10,
            sla_config: Some(SLAConfig {
                threshold_seconds: 100,
                penalty_multiplier_bps: 8000, // 20% discount
            }),
            sla_state: SLAState {
                accumulated_downtime: 0,
                last_report_timestamp: 0,
                is_penalty_active: false,
            },
            last_update: 0,
        };

        // Node A reports downtime from 0 to 50. (Consensus count = 1, not applied)
        // Node B reports downtime from 0 to 50. (Consensus count = 2, APPLIED!)
        meter.sla_state.accumulated_downtime += 50;
        meter.sla_state.last_report_timestamp = 50;

        // Current downtime (50) < threshold (100). No penalty yet.
        let claim1 = settle_claim_logic(&mut meter, 100);
        assert_eq!(claim1, 100 * 10); // 1000
        assert!(!meter.sla_state.is_penalty_active);

        // Node A and B both report downtime from 100 to 200.
        meter.sla_state.accumulated_downtime += 100;
        meter.sla_state.last_report_timestamp = 200;

        // Accumulated downtime = 150 > threshold (100). PENALTY APPLIED!
        let claim2 = settle_claim_logic(&mut meter, 300);
        // Elapsed = 200. Base = 2000. Penalized (20%) = 1600.
        assert_eq!(claim2, 1600);
        assert!(meter.sla_state.is_penalty_active);
    }
}

fn main() {}
