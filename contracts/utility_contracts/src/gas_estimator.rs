use soroban_sdk::contracttype;
use soroban_sdk::String;
use soroban_sdk::{Address, Env};

pub struct GasCostEstimator;

impl GasCostEstimator {
    // Gas costs for different operations (in stroops)
    const REGISTER_METER: i128 = 10_000_000; // 0.1 XLM
    const TOP_UP: i128 = 5_000_000; // 0.05 XLM
    const CLAIM: i128 = 8_000_000; // 0.08 XLM
    const UPDATE_HEARTBEAT: i128 = 3_000_000; // 0.03 XLM
    const GROUP_TOP_UP_PER_METER: i128 = 6_000_000; // 0.06 XLM per meter
    const EMERGENCY_SHUTDOWN: i128 = 2_000_000; // 0.02 XLM
    const SUBMIT_ZK_REPORT: i128 = 50_000_000; // 0.5 XLM (includes pairing check)
    const SET_ZK_VK: i128 = 15_000_000; // 0.15 XLM

    // Estimated monthly operations per meter
    const CLAIMS_PER_MONTH: u32 = 30;
    const HEARTBEATS_PER_MONTH: u32 = 720;
    const TOP_UPS_PER_MONTH: u32 = 4;

    pub fn estimate_meter_monthly_cost(
        _env: &Env,
        is_group_meter: bool,
        _meters_in_group: u32,
    ) -> i128 {
        let mut monthly_cost = Self::REGISTER_METER;

        monthly_cost += (Self::CLAIM as u32 * Self::CLAIMS_PER_MONTH) as i128;
        monthly_cost += (Self::UPDATE_HEARTBEAT as u32 * Self::HEARTBEATS_PER_MONTH) as i128;
        monthly_cost += (Self::TOP_UP as u32 * Self::TOP_UPS_PER_MONTH) as i128;

        if is_group_meter {
            monthly_cost -= (Self::TOP_UP as u32 * Self::TOP_UPS_PER_MONTH) as i128;
            monthly_cost += (Self::GROUP_TOP_UP_PER_METER as u32 * Self::TOP_UPS_PER_MONTH) as i128;
        }

        monthly_cost
    }

    /// `percentage_group_meters_bps`: percentage in basis points (10000 = 100%)
    pub fn estimate_provider_monthly_cost(
        _env: &Env,
        number_of_meters: u32,
        percentage_group_meters_bps: i128, // basis points (10000 = 100%)
    ) -> i128 {
        let group_meters = ((number_of_meters as i128 * percentage_group_meters_bps) / 10000) as u32;
        let individual_meters = number_of_meters - group_meters;

        let group_cost = if group_meters > 0 {
            let groups = group_meters / 5;
            if groups > 0 {
                Self::estimate_meter_monthly_cost(_env, true, 5) * groups as i128
            } else {
                0
            }
        } else {
            0
        };

        let individual_cost =
            Self::estimate_meter_monthly_cost(_env, false, 0) * individual_meters as i128;

        group_cost + individual_cost
    }

    pub fn estimate_large_scale_costs(
        env: &Env,
        number_of_meters: u32,
        percentage_group_meters_bps: i128,
    ) -> LargeScaleCostEstimate {
        let monthly_cost_stroops = Self::estimate_provider_monthly_cost(env, number_of_meters, percentage_group_meters_bps);
        let annual_cost_stroops = monthly_cost_stroops * 12;
        let cost_per_meter_stroops = if number_of_meters > 0 { annual_cost_stroops / number_of_meters as i128 } else { 0 };

        // Convert to XLM (1 XLM = 10,000,000 stroops)
        let xlm_precision: i128 = 10_000_000;
        let monthly_cost_xlm = monthly_cost_stroops / xlm_precision;
        let annual_cost_xlm = annual_cost_stroops / xlm_precision;
        let cost_per_meter_xlm = cost_per_meter_stroops / xlm_precision;

        LargeScaleCostEstimate {
            number_of_meters,
            monthly_cost_stroops,
            annual_cost_stroops,
            cost_per_meter_stroops,
            monthly_cost_xlm,
            annual_cost_xlm,
            cost_per_meter_xlm,
            group_billing_enabled: percentage_group_meters_bps > 0,
        }
    }

    pub fn get_operation_cost(operation: &soroban_sdk::String) -> i128 {
        // Compare operation name by checking known byte patterns
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "register_meter") {
            return Self::REGISTER_METER;
        }
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "top_up") {
            return Self::TOP_UP;
        }
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "claim") {
            return Self::CLAIM;
        }
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "update_heartbeat") {
            return Self::UPDATE_HEARTBEAT;
        }
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "group_top_up") {
            return Self::GROUP_TOP_UP_PER_METER;
        }
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "emergency_shutdown") {
            return Self::EMERGENCY_SHUTDOWN;
        }
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "submit_zk_usage_report") {
            return Self::SUBMIT_ZK_REPORT;
        }
        if *operation == soroban_sdk::String::from_str(&soroban_sdk::Env::default(), "set_zk_verification_key") {
            return Self::SET_ZK_VK;
        }
        0
    }
}

#[contracttype]
#[derive(Clone)]
pub struct LargeScaleCostEstimate {
    pub number_of_meters: u32,
    pub monthly_cost_stroops: i128,
    pub annual_cost_stroops: i128,
    pub cost_per_meter_stroops: i128,
    pub monthly_cost_xlm: i128,
    pub annual_cost_xlm: i128,
    pub cost_per_meter_xlm: i128,
    pub group_billing_enabled: bool,
}

impl LargeScaleCostEstimate {
}
