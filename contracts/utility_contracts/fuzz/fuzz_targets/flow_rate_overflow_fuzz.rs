#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as TestAddress, Address, Env};
use utility_contracts::{UtilityContract, ContinuousFlow, StreamStatus};

fuzz_target!(|data: &[u8]| {
    // Need at least 64 bytes for comprehensive flow rate testing
    if data.len() < 64 {
        return;
    }
    
    // Extract multiple i128 values from the data
    let mut bytes_flow_rate = [0u8; 16];
    let mut bytes_balance = [0u8; 16];
    let mut bytes_buffer = [0u8; 16];
    let mut bytes_timestamp = [0u8; 16];
    
    bytes_flow_rate.copy_from_slice(&data[0..16]);
    bytes_balance.copy_from_slice(&data[16..32]);
    bytes_buffer.copy_from_slice(&data[32..48]);
    bytes_timestamp.copy_from_slice(&data[48..64]);
    
    let flow_rate = i128::from_be_bytes(bytes_flow_rate);
    let balance = i128::from_be_bytes(bytes_balance);
    let buffer_balance = i128::from_be_bytes(bytes_buffer);
    let timestamp_delta = u64::from_be_bytes([
        data[64], data[65], data[66], data[67], data[68], data[69], data[70], data[71]
    ]) if data.len() > 71 else 0;
    
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = utility_contracts::UtilityContractClient::new(&env, &contract_id);
    
    // Create test addresses
    let provider = TestAddress::generate(&env);
    let token = TestAddress::generate(&env);
    
    // Test edge cases for flow rate calculations
    let test_flow_rates = vec![
        flow_rate,
        flow_rate.saturating_mul(1000),  // Large multiplier
        flow_rate.saturating_div(2),     // Division
        i128::MAX,
        i128::MIN,
        i128::MAX / 2,
        1,                              // Minimum positive
        0,                              // Zero flow rate
        -1,                             // Negative flow rate (should be handled)
    ];
    
    let test_balances = vec![
        balance,
        balance.saturating_mul(1000),
        i128::MAX,
        i128::MIN,
        1,
        0,
    ];
    
    // Test flow rate overflow scenarios
    for &test_flow_rate in &test_flow_rates {
        for &test_balance in &test_balances {
            // Test continuous stream creation with extreme values
            let stream_id = ((test_flow_rate as u64).wrapping_add(test_balance as u64)) % 1000000 + 1;
            
            let result = std::panic::catch_unwind(|| {
                client.create_continuous_stream(
                    &stream_id,
                    &test_flow_rate,
                    &test_balance,
                    &provider
                );
            });
            
            // Contract should handle extreme values gracefully or panic with proper error
            if let Err(_) = result {
                // Expected behavior for invalid inputs
                continue;
            }
            
            // Test flow rate updates with extreme values
            let update_flow_rates = vec![
                test_flow_rate.saturating_mul(10),
                test_flow_rate.saturating_div(2),
                test_flow_rate.saturating_add(i128::MAX / 1000),
                test_flow_rate.saturating_sub(i128::MAX / 1000),
            ];
            
            for &new_flow_rate in &update_flow_rates {
                let update_result = std::panic::catch_unwind(|| {
                    client.update_continuous_flow_rate(&stream_id, new_flow_rate);
                });
                
                // Should handle updates gracefully
                if let Err(_) = update_result {
                    // Expected for invalid flow rates
                    continue;
                }
            }
            
            // Test balance additions with potential overflow
            let addition_amounts = vec![
                test_balance.saturating_mul(1000),
                test_balance.saturating_div(2),
                i128::MAX,
                i128::MIN,
                1,
                0,
            ];
            
            for &add_amount in &addition_amounts {
                let add_result = std::panic::catch_unwind(|| {
                    client.add_continuous_balance(&stream_id, add_amount);
                });
                
                // Should handle additions gracefully
                if let Err(_) = add_result {
                    // Expected for invalid amounts
                    continue;
                }
            }
            
            // Test withdrawal calculations
            let withdrawal_amounts = vec![
                test_balance / 2,
                test_balance / 10,
                test_balance.saturating_mul(2),
                i128::MAX,
                1,
                0,
            ];
            
            for &withdraw_amount in &withdrawal_amounts {
                let withdraw_result = std::panic::catch_unwind(|| {
                    client.withdraw_continuous(&stream_id, withdraw_amount);
                });
                
                // Should handle withdrawals gracefully
                if let Err(_) = withdraw_result {
                    // Expected for invalid withdrawals
                    continue;
                }
            }
        }
    }
    
    // Test timestamp-based calculations with extreme time deltas
    let test_timestamps = vec![
        timestamp_delta,
        timestamp_delta.saturating_mul(1000),
        u64::MAX,
        u64::MAX / 2,
        1,
        0,
    ];
    
    for &time_delta in &test_timestamps {
        // Create a stream for timestamp testing
        let stream_id = 999999u64;
        let base_flow_rate = 1000i128;
        let base_balance = 1000000i128;
        
        let create_result = std::panic::catch_unwind(|| {
            client.create_continuous_stream(&stream_id, &base_flow_rate, &base_balance, &provider);
        });
        
        if create_result.is_err() {
            continue;
        }
        
        // Test flow calculations over extreme time periods
        let current_time = env.ledger().timestamp();
        env.ledger().set_timestamp(current_time.saturating_add(time_delta));
        
        let calc_result = std::panic::catch_unwind(|| {
            // This should trigger flow calculation based on time delta
            let _flow = client.get_continuous_flow(&stream_id);
            let _balance = client.get_continuous_balance(&stream_id);
        });
        
        if calc_result.is_err() {
            panic!("Flow calculation crashed with time delta: {}", time_delta);
        }
        
        // Test pause/resume with extreme timestamps
        let pause_result = std::panic::catch_unwind(|| {
            client.pause_stream(&stream_id);
        });
        
        if pause_result.is_ok() {
            let resume_result = std::panic::catch_unwind(|| {
                client.resume_stream(&stream_id, &base_flow_rate);
            });
            
            if resume_result.is_err() {
                panic!("Resume crashed with time delta: {}", time_delta);
            }
        }
    }
    
    // Test precision factor calculations
    let precision_factors = vec![1i128, 1000i128, 1_000_000i128, i128::MAX / 1000];
    
    for &precision in &precision_factors {
        for &test_flow_rate in &test_flow_rates {
            // Test flow rate precision calculations
            let precision_result = std::panic::catch_unwind(|| {
                let _precise_flow = test_flow_rate.saturating_mul(precision);
                let _adjusted_flow = _precise_flow.saturating_div(precision);
            });
            
            if precision_result.is_err() {
                panic!("Precision calculation crashed with flow_rate: {} and precision: {}", 
                       test_flow_rate, precision);
            }
        }
    }
    
    // Test edge case: Maximum values combined
    let max_flow_rate = i128::MAX;
    let max_balance = i128::MAX;
    let max_time = u64::MAX;
    
    let edge_case_result = std::panic::catch_unwind(|| {
        let stream_id = 888888u64;
        client.create_continuous_stream(&stream_id, &max_flow_rate, &max_balance, &provider);
        
        env.ledger().set_timestamp(max_time);
        let _flow = client.get_continuous_flow(&stream_id);
    });
    
    // Should handle gracefully or panic with proper error
    if let Err(_) = edge_case_result {
        // Expected behavior for extreme edge case
    }
});
