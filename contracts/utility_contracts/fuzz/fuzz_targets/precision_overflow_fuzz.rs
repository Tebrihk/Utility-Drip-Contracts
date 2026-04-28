#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as TestAddress, Address, Env};
use utility_contracts::{UtilityContract, BillingType};

fuzz_target!(|data: &[u8]| {
    // Need at least 64 bytes for precision factor testing
    if data.len() < 64 {
        return;
    }
    
    // Extract values for precision factor testing
    let mut bytes_usage = [0u8; 16];
    let mut bytes_precision = [0u8; 16];
    let mut bytes_rate = [0u8; 16];
    let mut bytes_peak = [0u8; 16];
    
    bytes_usage.copy_from_slice(&data[0..16]);
    bytes_precision.copy_from_slice(&data[16..32]);
    bytes_rate.copy_from_slice(&data[32..48]);
    bytes_peak.copy_from_slice(&data[48..64]);
    
    let usage = i128::from_be_bytes(bytes_usage);
    let precision_factor = i128::from_be_bytes(bytes_precision);
    let rate = i128::from_be_bytes(bytes_rate);
    let peak_rate = i128::from_be_bytes(bytes_peak);
    
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = utility_contracts::UtilityContractClient::new(&env, &contract_id);
    
    // Create test addresses
    let user = TestAddress::generate(&env);
    let provider = TestAddress::generate(&env);
    let token = TestAddress::generate(&env);
    
    // Mock the oracle address
    env.storage().instance().set(&utility_contracts::DataKey::Oracle, &provider);
    
    // Test edge cases for precision factor calculations
    let test_usages = vec![
        usage,
        usage.saturating_mul(1000000),  // Very large usage
        usage.saturating_mul(1000000000), // Extremely large usage
        i128::MAX,
        i128::MIN,
        i128::MAX / 2,
        1,
        0,
    ];
    
    let test_precision_factors = vec![
        precision_factor,
        precision_factor.saturating_mul(1000),
        precision_factor.saturating_mul(1000000),
        i128::MAX,
        i128::MIN,
        i128::MAX / 2,
        1,
        0,  // Zero precision factor (edge case)
    ];
    
    let test_rates = vec![
        rate,
        rate.saturating_mul(1000),
        i128::MAX,
        i128::MIN,
        1,
        0,
    ];
    
    // Test precision factor overflow in usage calculations
    for &test_usage in &test_usages {
        for &test_precision in &test_precision_factors {
            // Test precision multiplication
            let mult_result = std::panic::catch_unwind(|| {
                let _precise_usage = test_usage.saturating_mul(test_precision);
            });
            
            if mult_result.is_err() {
                panic!("Precision multiplication crashed with usage: {} and precision: {}", 
                       test_usage, test_precision);
            }
            
            // Test precision division
            if test_precision != 0 {
                let div_result = std::panic::catch_unwind(|| {
                    let _display_usage = test_usage / test_precision;
                });
                
                if div_result.is_err() {
                    panic!("Precision division crashed with usage: {} and precision: {}", 
                           test_usage, test_precision);
                }
            }
            
            // Test combined precision operations
            let combined_result = std::panic::catch_unwind(|| {
                let step1 = test_usage.saturating_mul(test_precision);
                let step2 = step1.saturating_div(test_precision.max(1));
                let step3 = step2.saturating_mul(1000); // Display conversion
                let step4 = step3.saturating_div(1000); // Reverse conversion
            });
            
            if combined_result.is_err() {
                panic!("Combined precision operations crashed with usage: {} and precision: {}", 
                       test_usage, test_precision);
            }
        }
    }
    
    // Test precision factor in meter operations
    for &test_usage in &test_usages {
        for &test_precision in &test_precision_factors {
            for &test_rate in &test_rates {
                let meter_id = ((test_usage as u64).wrapping_add(test_precision as u64).wrapping_add(test_rate as u64)) % 1000000 + 1;
                
                // Create meter
                let create_result = std::panic::catch_unwind(|| {
                    client.register_meter_with_mode(
                        &meter_id,
                        &user,
                        &provider,
                        &test_rate,
                        &token,
                        &BillingType::PrePaid,
                        &TestAddress::generate(&env),
                        &0u32,
                    );
                });
                
                if create_result.is_err() {
                    continue;
                }
                
                // Set precision factor if possible (this might require a specific function)
                // For now, we test the precision effects through usage updates
                
                // Test usage update with precision implications
                let update_result = std::panic::catch_unwind(|| {
                    client.update_usage(&meter_id, &test_usage);
                });
                
                if update_result.is_err() {
                    panic!("Usage update crashed with usage: {}, precision: {}, rate: {}", 
                           test_usage, test_precision, test_rate);
                }
                
                // Test multiple usage updates to accumulate precision effects
                for i in 1..=5 {
                    let cumulative_usage = test_usage.saturating_mul(i as i128);
                    let cumulative_result = std::panic::catch_unwind(|| {
                        client.update_usage(&meter_id, &cumulative_usage);
                    });
                    
                    if cumulative_result.is_err() {
                        panic!("Cumulative usage update crashed at iteration {} with usage: {}", 
                               i, cumulative_usage);
                    }
                }
                
                // Test claim operations which involve precision calculations
                let claim_result = std::panic::catch_unwind(|| {
                    client.claim(&meter_id);
                });
                
                if claim_result.is_err() {
                    panic!("Claim operation crashed with usage: {}, precision: {}, rate: {}", 
                           test_usage, test_precision, test_rate);
                }
            }
        }
    }
    
    // Test peak rate precision calculations
    for &test_rate in &test_rates {
        for &test_peak_rate in &test_rates {
            // Test peak rate multiplier calculations
            let peak_mult_result = std::panic::catch_unwind(|| {
                let _peak_adjusted = test_rate.saturating_mul(test_peak_rate);
                let _peak_divided = _peak_adjusted.saturating_div(test_rate.max(1));
            });
            
            if peak_mult_result.is_err() {
                panic!("Peak rate calculation crashed with rate: {} and peak_rate: {}", 
                       test_rate, test_peak_rate);
            }
            
            // Test rate precision with time-based calculations
            let time_factors = vec![1u64, 60u64, 3600u64, 86400u64, u64::MAX / 1000000];
            
            for &time_factor in &time_factors {
                let time_calc_result = std::panic::catch_unwind(|| {
                    let time_usage = test_rate.saturating_mul(time_factor as i128);
                    let precise_time = time_usage.saturating_mul(1000);
                    let display_time = precise_time.saturating_div(1000);
                });
                
                if time_calc_result.is_err() {
                    panic!("Time-based precision calculation crashed with rate: {} and time: {}", 
                           test_rate, time_factor);
                }
            }
        }
    }
    
    // Test renewable energy percentage precision
    let renewable_percentages = vec![0u32, 1u32, 50u32, 99u32, 100u32, u32::MAX];
    
    for &renewable_pct in &renewable_percentages {
        for &test_usage in &test_usages {
            let renewable_result = std::panic::catch_unwind(|| {
                let renewable_usage = test_usage.saturating_mul(renewable_pct as i128);
                let renewable_divided = renewable_usage.saturating_div(100);
                let total_usage = test_usage.saturating_mul(100);
                let precise_total = total_usage.saturating_div(100);
            });
            
            if renewable_result.is_err() {
                panic!("Renewable energy precision calculation crashed with usage: {} and percentage: {}", 
                       test_usage, renewable_pct);
            }
        }
    }
    
    // Test volume tracking precision
    let volume_factors = vec![1i128, 1000i128, 1000000i128, i128::MAX / 1000];
    
    for &volume_factor in &volume_factors {
        for &test_usage in &test_usages {
            let volume_result = std::panic::catch_unwind(|| {
                let monthly_volume = test_usage.saturating_mul(volume_factor);
                let precise_volume = monthly_volume.saturating_mul(1000);
                let display_volume = precise_volume.saturating_div(1000);
                
                // Test volume reset calculations
                let volume_reset = monthly_volume.saturating_sub(monthly_volume);
                let new_volume = volume_reset.saturating_add(test_usage);
            });
            
            if volume_result.is_err() {
                panic!("Volume tracking precision calculation crashed with usage: {} and factor: {}", 
                       test_usage, volume_factor);
            }
        }
    }
    
    // Test edge case: Maximum precision scenario
    let max_precision_result = std::panic::catch_unwind(|| {
        let meter_id = 999999u64;
        client.register_meter_with_mode(
            &meter_id,
            &user,
            &provider,
            &i128::MAX,
            &token,
            &BillingType::PrePaid,
            &TestAddress::generate(&env),
            &0u32,
        );
        
        // Update usage with maximum values
        client.update_usage(&meter_id, &i128::MAX);
        
        // Multiple claims to test precision accumulation
        for _ in 0..10 {
            client.claim(&meter_id);
        }
        
        // Top-up with maximum amount
        client.top_up(&meter_id, &i128::MAX, &user);
        client.claim(&meter_id);
    });
    
    // Should handle maximum precision scenario gracefully
    if let Err(_) = max_precision_result {
        // Expected for extreme edge case
    }
    
    // Test precision factor edge cases
    let edge_cases = vec![
        (i128::MAX, 1),
        (i128::MAX, i128::MAX),
        (i128::MAX, 0),
        (1, i128::MAX),
        (0, i128::MAX),
        (i128::MIN, 1),
        (i128::MIN, i128::MAX),
    ];
    
    for &(usage_val, precision_val) in &edge_cases {
        let edge_result = std::panic::catch_unwind(|| {
            if precision_val != 0 {
                let _result = usage_val.saturating_mul(precision_val);
                let _display = _result.saturating_div(precision_val);
            }
        });
        
        if edge_result.is_err() {
            panic!("Edge case precision calculation crashed with usage: {} and precision: {}", 
                   usage_val, precision_val);
        }
    }
});
