#![no_std]
use soroban_sdk::{contract, contracttype, contractimpl, Address, Env, token, Vec};

// Simplified version focusing on core security fixes
// Yield farming features removed due to Soroban type compatibility issues

#[contracttype]
#[derive(Clone)]
pub struct Meter {
    pub user: Address,
    pub provider: Address,
    pub rate_per_second: i128,
    pub balance: i128,
    pub last_update: u64,
    pub is_active: bool,
    pub token: Address,
    pub max_flow_rate_per_hour: i128,
    pub last_claim_time: u64,
    pub claimed_this_hour: i128,
    pub heartbeat: u64,
    pub parent_account: Option<Address>,
}

#[contracttype]
#[derive(Clone)]
pub struct BillingGroup {
    pub parent_account: Address,
    pub child_meters: Vec<u64>,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct WebhookConfig {
    pub url_hash: u64, // Store hash of URL instead of full URL
    pub user: Address,
    pub is_active: bool,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct LowBalanceAlert {
    pub meter_id: u64,
    pub user: Address,
    pub remaining_balance: i128,
    pub hours_remaining_scaled: u32, // hours_remaining * 100 to avoid float
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct InitializedEvent {
    pub root_admin: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MeterRegisteredEvent {
    pub meter_id: u64,
    pub user: Address,
    pub provider: Address,
    pub rate_per_second: i128,
    pub token: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MeterToppedUpEvent {
    pub meter_id: u64,
    pub user: Address,
    pub amount: i128,
    pub new_balance: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FlowRateUpdatedEvent {
    pub meter_id: u64,
    pub user: Address,
    pub new_max_flow_rate: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ClaimEvent {
    pub meter_id: u64,
    pub provider: Address,
    pub amount_claimed: i128,
    pub remaining_balance: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EmergencyShutdownEvent {
    pub meter_id: u64,
    pub provider: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct HeartbeatUpdatedEvent {
    pub meter_id: u64,
    pub user: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BillingGroupCreatedEvent {
    pub parent_account: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct GroupTopUpEvent {
    pub parent_account: Address,
    pub amount_per_meter: i128,
    pub total_amount: i128,
    pub meter_count: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct WebhookConfiguredEvent {
    pub user: Address,
    pub webhook_url_hash: u64, // Store hash instead of full URL
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct WebhookDeactivatedEvent {
    pub user: Address,
    pub timestamp: u64,
}

#[contracttype]
pub enum DataKey {
    Meter(u64),
    Count,
    BillingGroup(Address),
    WebhookConfig(Address),
    LastAlert(u64), // meter_id -> timestamp of last alert
    Initialized,
    RootAdmin,
}

#[contract]
pub struct UtilityContract;

#[contractimpl]
impl UtilityContract {
    /// Initialize the contract with root admin
    /// Can only be called once
    pub fn initialize(env: Env, root_admin: Address) {
        // Check if already initialized
        if env.storage().instance().get(&DataKey::Initialized).unwrap_or(false) {
            panic!("Contract already initialized");
        }
        
        // Set initialization flag
        env.storage().instance().set(&DataKey::Initialized, &true);
        
        // Set root admin
        env.storage().instance().set(&DataKey::RootAdmin, &root_admin);
        
        // Emit initialization event
        env.events().publish(
            ("Initialized", "Contract"),
            InitializedEvent {
                root_admin: root_admin.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );
    }
    
    /// Check if contract is initialized
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Initialized).unwrap_or(false)
    }
    
    /// Get the root admin address
    pub fn get_root_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::RootAdmin)
    }
    
    /// Require root admin authentication
    fn require_root_admin(env: &Env) {
        let root_admin: Address = env.storage().instance().get(&DataKey::RootAdmin)
            .ok_or("Contract not initialized").unwrap();
        root_admin.require_auth();
    }
    pub fn register_meter(
        env: Env,
        user: Address,
        provider: Address,
        rate: i128,
        token: Address,
        parent_account: Option<Address>,
    ) -> u64 {
        user.require_auth();
        let mut count: u64 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        count += 1;

        let meter = Meter {
            user: user.clone(),
            provider: provider.clone(),
            rate_per_second: rate,
            balance: 0,
            last_update: env.ledger().timestamp(),
            is_active: false,
            token: token.clone(),
            max_flow_rate_per_hour: rate * 3600, // Default to 1 hour of normal flow
            last_claim_time: env.ledger().timestamp(),
            claimed_this_hour: 0,
            heartbeat: env.ledger().timestamp(),
            parent_account: parent_account.clone(),
        };

        env.storage().instance().set(&DataKey::Meter(count), &meter);
        env.storage().instance().set(&DataKey::Count, &count);
        
        // Emit meter registration event
        env.events().publish(
            ("MeterRegistered", "Meter"),
            MeterRegisteredEvent {
                meter_id: count,
                user: user.clone(),
                provider: provider.clone(),
                rate_per_second: rate,
                token: token.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );
        
        // If this meter has a parent account, add it to the billing group
        if let Some(parent) = parent_account {
            Self::add_meter_to_billing_group(env, parent, count);
        }
        
        count
    }

    pub fn top_up(env: Env, meter_id: u64, amount: i128) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.user.require_auth();

        let client = token::Client::new(&env, &meter.token);
        client.transfer(&meter.user, &env.current_contract_address(), &amount);

        meter.balance += amount;
        meter.is_active = true;
        meter.last_update = env.ledger().timestamp();
        
        let new_balance = meter.balance;
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
        
        // Emit top-up event
        env.events().publish(
            ("MeterToppedUp", "Meter"),
            MeterToppedUpEvent {
                meter_id,
                user: meter.user.clone(),
                amount,
                new_balance,
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    pub fn set_max_flow_rate(env: Env, meter_id: u64, max_rate: i128) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.user.require_auth();
        
        meter.max_flow_rate_per_hour = max_rate;
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
        
        // Emit flow rate update event
        env.events().publish(
            ("FlowRateUpdated", "Meter"),
            FlowRateUpdatedEvent {
                meter_id,
                user: meter.user.clone(),
                new_max_flow_rate: max_rate,
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    pub fn claim(env: Env, meter_id: u64) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.provider.require_auth();

        let now = env.ledger().timestamp();
        let elapsed = now.checked_sub(meter.last_update).unwrap_or(0);
        let amount = (elapsed as i128) * meter.rate_per_second;
        
        // Check if we need to reset the hourly counter
        let hours_passed = now.checked_sub(meter.last_claim_time).unwrap_or(0) / 3600;
        if hours_passed >= 1 {
            meter.claimed_this_hour = 0;
            meter.last_claim_time = now;
        }
        
        // Ensure we don't overdraw the balance
        let claimable = if amount > meter.balance {
            meter.balance
        } else {
            amount
        };
        
        // Apply max flow rate cap
        let final_claimable = if claimable > 0 {
            let remaining_hourly_capacity = meter.max_flow_rate_per_hour - meter.claimed_this_hour;
            if claimable > remaining_hourly_capacity {
                remaining_hourly_capacity
            } else {
                claimable
            }
        } else {
            0
        };

        if final_claimable > 0 {
            let client = token::Client::new(&env, &meter.token);
            client.transfer(&env.current_contract_address(), &meter.provider, &final_claimable);
            meter.balance -= final_claimable;
            meter.claimed_this_hour += final_claimable;
        }

        meter.last_update = now;
        if meter.balance <= 0 {
            meter.is_active = false;
        }

        let remaining_balance = meter.balance;
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
        
        // Emit claim event
        env.events().publish(
            ("Claim", "Meter"),
            ClaimEvent {
                meter_id,
                provider: meter.provider.clone(),
                amount_claimed: final_claimable,
                remaining_balance,
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    pub fn get_meter(env: Env, meter_id: u64) -> Option<Meter> {
        env.storage().instance().get(&DataKey::Meter(meter_id))
    }

    pub fn calculate_expected_depletion(env: Env, meter_id: u64) -> Option<u64> {
        if let Some(meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
            if meter.balance <= 0 || meter.rate_per_second <= 0 {
                return Some(0); // Already depleted or no consumption
            }
            
            let seconds_until_depletion = meter.balance / meter.rate_per_second;
            let current_time = env.ledger().timestamp();
            Some(current_time + seconds_until_depletion as u64)
        } else {
            None
        }
    }

    pub fn emergency_shutdown(env: Env, meter_id: u64) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.provider.require_auth();
        
        // Immediately disable the meter
        meter.is_active = false;
        
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
        
        // Emit emergency shutdown event
        env.events().publish(
            ("EmergencyShutdown", "Meter"),
            EmergencyShutdownEvent {
                meter_id,
                provider: meter.provider.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    pub fn update_heartbeat(env: Env, meter_id: u64) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.user.require_auth();
        
        meter.heartbeat = env.ledger().timestamp();
        
        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
        
        // Emit heartbeat update event
        env.events().publish(
            ("HeartbeatUpdated", "Meter"),
            HeartbeatUpdatedEvent {
                meter_id,
                user: meter.user.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    pub fn is_meter_offline(env: Env, meter_id: u64) -> bool {
        if let Some(meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
            let current_time = env.ledger().timestamp();
            let time_since_heartbeat = current_time.checked_sub(meter.heartbeat).unwrap_or(0);
            // Consider offline if heartbeat is > 1 hour old (3600 seconds)
            time_since_heartbeat > 3600
        } else {
            true // Meter not found, consider offline
        }
    }

    // Group Billing Functions
    pub fn create_billing_group(env: Env, parent_account: Address) {
        parent_account.require_auth();
        
        let billing_group = BillingGroup {
            parent_account: parent_account.clone(),
            child_meters: Vec::new(&env),
            created_at: env.ledger().timestamp(),
        };
        
        env.storage().instance().set(&DataKey::BillingGroup(parent_account.clone()), &billing_group);
        
        // Emit billing group creation event
        env.events().publish(
            ("BillingGroupCreated", "BillingGroup"),
            BillingGroupCreatedEvent {
                parent_account: parent_account.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    fn add_meter_to_billing_group(env: Env, parent_account: Address, meter_id: u64) {
        let mut billing_group: BillingGroup = env.storage().instance()
            .get(&DataKey::BillingGroup(parent_account.clone()))
            .unwrap_or_else(|| BillingGroup {
                parent_account: parent_account.clone(),
                child_meters: Vec::new(&env),
                created_at: env.ledger().timestamp(),
            });
        
        // Add meter to the group if not already present
        // Note: Soroban Vec doesn't have contains method, so we need to check manually
        let mut found = false;
        let len = billing_group.child_meters.len();
        for i in 0..len {
            if billing_group.child_meters.get(i).unwrap_or(0) == meter_id {
                found = true;
                break;
            }
        }
        
        if !found {
            billing_group.child_meters.push_back(meter_id);
            env.storage().instance().set(&DataKey::BillingGroup(parent_account), &billing_group);
        }
    }

    pub fn group_top_up(env: Env, parent_account: Address, amount_per_meter: i128) {
        parent_account.require_auth();
        
        let billing_group: BillingGroup = env.storage().instance()
            .get(&DataKey::BillingGroup(parent_account.clone()))
            .ok_or("Billing group not found").unwrap();
        
        if billing_group.child_meters.is_empty() {
            return;
        }
        
        let total_amount = amount_per_meter * billing_group.child_meters.len() as i128;
        let meter_count = billing_group.child_meters.len() as u32;
        
        // Transfer total amount from parent to contract
        if billing_group.child_meters.len() > 0 {
            let first_meter_id = billing_group.child_meters.get(0).unwrap_or(0);
            if let Some(first_meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(first_meter_id)) {
                let client = token::Client::new(&env, &first_meter.token);
                client.transfer(&parent_account, &env.current_contract_address(), &total_amount);
            }
        }
        
        // Distribute funds to all child meters
        let len = billing_group.child_meters.len();
        for i in 0..len {
            let meter_id = billing_group.child_meters.get(i).unwrap_or(0);
            if let Some(mut meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
                meter.balance += amount_per_meter;
                meter.is_active = true;
                meter.last_update = env.ledger().timestamp();
                env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
            }
        }
        
        // Emit group top-up event
        env.events().publish(
            ("GroupTopUp", "BillingGroup"),
            GroupTopUpEvent {
                parent_account: parent_account.clone(),
                amount_per_meter,
                total_amount,
                meter_count,
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    pub fn get_billing_group(env: Env, parent_account: Address) -> Option<BillingGroup> {
        env.storage().instance().get(&DataKey::BillingGroup(parent_account))
    }

    pub fn remove_meter_from_billing_group(env: Env, parent_account: Address, meter_id: u64) {
        parent_account.require_auth();
        
        let mut billing_group: BillingGroup = env.storage().instance()
            .get(&DataKey::BillingGroup(parent_account.clone()))
            .ok_or("Billing group not found").unwrap();
        
        // Create a new Vec without the meter to remove (Soroban Vec doesn't have retain)
        let mut new_child_meters = Vec::new(&env);
        let len = billing_group.child_meters.len();
        for i in 0..len {
            let child_meter_id = billing_group.child_meters.get(i).unwrap_or(0);
            if child_meter_id != meter_id {
                new_child_meters.push_back(child_meter_id);
            }
        }
        
        billing_group.child_meters = new_child_meters;
        env.storage().instance().set(&DataKey::BillingGroup(parent_account), &billing_group);
        
        // Update the meter to remove parent reference
        if let Some(mut meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
            meter.parent_account = None;
            env.storage().instance().set(&DataKey::Meter(meter_id), &meter);
        }
    }

    // Gas Cost Estimator Functions - Note: These are for off-chain calculations only
    // pub fn estimate_meter_monthly_cost(env: Env, is_group_meter: bool, meters_in_group: u32) -> i128 {
    //     GasCostEstimator::estimate_meter_monthly_cost(&env, is_group_meter, meters_in_group)
    // }

    // pub fn estimate_provider_monthly_cost(env: Env, number_of_meters: u32, percentage_group_meters: f32) -> i128 {
    //     GasCostEstimator::estimate_provider_monthly_cost(&env, number_of_meters, percentage_group_meters)
    // }

    // pub fn estimate_large_scale_cost(env: Env, number_of_meters: u32, group_billing_enabled: bool) -> LargeScaleCostEstimate {
    //     GasCostEstimator::estimate_large_scale_cost(&env, number_of_meters, group_billing_enabled)
    // }

    // pub fn get_operation_cost(_env: Env, operation: String) -> i128 {
    //     GasCostEstimator::get_operation_cost(&operation)
    // }

    // Webhook and Alert Functions
    pub fn configure_webhook(env: Env, user: Address, webhook_url_hash: u64) {
        user.require_auth();
        
        let webhook_config = WebhookConfig {
            url_hash: webhook_url_hash,
            user: user.clone(),
            is_active: true,
            created_at: env.ledger().timestamp(),
        };
        
        env.storage().instance().set(&DataKey::WebhookConfig(user.clone()), &webhook_config);
        
        // Emit webhook configuration event
        env.events().publish(
            ("WebhookConfigured", "Webhook"),
            WebhookConfiguredEvent {
                user: user.clone(),
                webhook_url_hash,
                timestamp: env.ledger().timestamp(),
            }
        );
    }

    pub fn deactivate_webhook(env: Env, user: Address) {
        user.require_auth();
        
        if let Some(mut config) = env.storage().instance().get::<_, WebhookConfig>(&DataKey::WebhookConfig(user.clone())) {
            config.is_active = false;
            env.storage().instance().set(&DataKey::WebhookConfig(user.clone()), &config);
            
            // Emit webhook deactivation event
            env.events().publish(
                ("WebhookDeactivated", "Webhook"),
                WebhookDeactivatedEvent {
                    user: user.clone(),
                    timestamp: env.ledger().timestamp(),
                }
            );
        }
    }

    pub fn get_webhook_config(env: Env, user: Address) -> Option<WebhookConfig> {
        env.storage().instance().get(&DataKey::WebhookConfig(user))
    }

    fn check_and_send_low_balance_alert(env: &Env, meter: &Meter, meter_id: u64) {
        // Only check if webhook is configured for this user
        let webhook_config = match env.storage().instance().get::<_, WebhookConfig>(&DataKey::WebhookConfig(meter.user.clone())) {
            Some(config) if config.is_active => config,
            _ => return, // No active webhook configured
        };

        // Calculate hours remaining (using integer math)
        let hours_remaining_scaled = if meter.rate_per_second > 0 {
            let hourly_consumption = meter.rate_per_second * 3600;
            (meter.balance * 100) / hourly_consumption // scaled by 100
        } else {
            0 // No consumption means infinite hours, but we'll treat as 0 for alert purposes
        };

        // Check if balance is low (< 24 hours)
        if hours_remaining_scaled < 2400 { // < 24 hours * 100
            // Check if we've sent an alert recently (within last 12 hours)
            let current_time = env.ledger().timestamp();
            let last_alert_time: Option<u64> = env.storage().instance().get(&DataKey::LastAlert(meter_id));
            
            if let Some(last_time) = last_alert_time {
                if current_time.checked_sub(last_time).unwrap_or(0) < 43200 { // 12 hours in seconds
                    return; // Already sent alert recently
                }
            }

            // Create and send alert
            let alert = LowBalanceAlert {
                meter_id,
                user: meter.user.clone(),
                remaining_balance: meter.balance,
                hours_remaining_scaled: hours_remaining_scaled.try_into().unwrap_or(0),
                timestamp: current_time,
            };

            // Store the alert timestamp
            env.storage().instance().set(&DataKey::LastAlert(meter_id), &current_time);

            // In a real implementation, this would make an HTTP call to the webhook
            // For now, we'll just emit an event for the alert
            env.events().publish(
                ("LowBalanceAlert", "Alert"),
                (meter_id, meter.user.clone(), meter.balance, hours_remaining_scaled as u32, current_time)
            );
        }
    }

    pub fn get_pending_alerts(env: Env, user: Address) -> u64 {
        // Simplified implementation - just return count of alerts for user
        // In practice, you'd maintain a more sophisticated alert tracking system
        let count: u64 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        let mut alert_count = 0u64;
        
        for meter_id in 1..=count {
            if let Some(meter) = env.storage().instance().get::<_, Meter>(&DataKey::Meter(meter_id)) {
                if meter.user == user {
                    // Check if meter has low balance (using integer math)
                    if meter.balance > 0 && meter.rate_per_second > 0 {
                        // hours_remaining = balance / (rate_per_second * 3600)
                        let hourly_consumption = meter.rate_per_second * 3600;
                        let hours_remaining_scaled = (meter.balance * 100) / hourly_consumption; // scaled by 100
                        if hours_remaining_scaled < 2400 { // < 24 hours * 100
                            alert_count += 1;
                        }
                    }
                }
            }
        }
        
        alert_count
    }

    // Enhanced claim function with webhook integration
    pub fn claim_with_alerts(env: Env, meter_id: u64) {
        let mut meter: Meter = env.storage().instance().get(&DataKey::Meter(meter_id)).ok_or("Meter not found").unwrap();
        meter.provider.require_auth();

        let now = env.ledger().timestamp();
        let elapsed = now.checked_sub(meter.last_update).unwrap_or(0);
        let amount = (elapsed as i128) * meter.rate_per_second;
        
        // Check if we need to reset the hourly counter
        let hours_passed = now.checked_sub(meter.last_claim_time).unwrap_or(0) / 3600;
        if hours_passed >= 1 {
            meter.claimed_this_hour = 0;
            meter.last_claim_time = now;
        }
        
        // Ensure we don't overdraw the balance
        let claimable = if amount > meter.balance {
            meter.balance
        } else {
            amount
        };
        
        // Apply max flow rate cap
        let final_claimable = if claimable > 0 {
            let remaining_hourly_capacity = meter.max_flow_rate_per_hour - meter.claimed_this_hour;
            if claimable > remaining_hourly_capacity {
                remaining_hourly_capacity
            } else {
                claimable
            }
        } else {
            0
        };

        if final_claimable > 0 {
            let client = token::Client::new(&env, &meter.token);
            client.transfer(&env.current_contract_address(), &meter.provider, &final_claimable);
            meter.balance -= final_claimable;
            meter.claimed_this_hour += final_claimable;
        }

        meter.last_update = now;
        if meter.balance <= 0 {
            meter.is_active = false;
        }

        env.storage().instance().set(&DataKey::Meter(meter_id), &meter);

        // Check for low balance and send alert if needed
        Self::check_and_send_low_balance_alert(&env, &meter, meter_id);
    }
}

mod test;
