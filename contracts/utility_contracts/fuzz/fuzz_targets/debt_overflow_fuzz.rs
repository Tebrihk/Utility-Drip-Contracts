#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as TestAddress, Address, Env};
use utility_contracts::{UtilityContract, BillingType};

fuzz_target!(|data: &[u8]| {
    // Need at least 80 bytes for comprehensive debt testing
    if data.len() < 80 {
        return;
    }
    
    // Extract values for debt calculation testing
    let mut bytes_usage = [0u8; 16];
    let mut bytes_rate = [0u8; 16];
    let mut bytes_balance = [0u8; 16];
    let mut bytes_collateral = [0u8; 16];
    let mut bytes_debt = [0u8; 16];
    
    bytes_usage.copy_from_slice(&data[0..16]);
    bytes_rate.copy_from_slice(&data[16..32]);
    bytes_balance.copy_from_slice(&data[32..48]);
    bytes_collateral.copy_from_slice(&data[48..64]);
    bytes_debt.copy_from_slice(&data[64..80]);
    
    let usage = i128::from_be_bytes(bytes_usage);
    let rate = i128::from_be_bytes(bytes_rate);
    let balance = i128::from_be_bytes(bytes_balance);
    let collateral_limit = i128::from_be_bytes(bytes_collateral);
    let existing_debt = i128::from_be_bytes(bytes_debt);
    
    let env = Env::default();
    let contract_id = env.register_contract(None, UtilityContract);
    let client = utility_contracts::UtilityContractClient::new(&env, &contract_id);
    
    // Create test addresses
    let user = TestAddress::generate(&env);
    let provider = TestAddress::generate(&env);
    let token = TestAddress::generate(&env);
    
    // Mock the oracle address
    env.storage().instance().set(&utility_contracts::DataKey::Oracle, &provider);
    
    // Test edge cases for debt calculations
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
    
    let test_rates = vec![
        rate,
        rate.saturating_mul(1000),
        rate.saturating_mul(1000000),
        i128::MAX,
        i128::MIN,
        1,
        0,
    ];
    
    let test_balances = vec![
        balance,
        balance.saturating_mul(1000),
        i128::MAX,
        i128::MIN,
        1,
        0,
        -1000000,  // Negative balance (debt)
        -i128::MAX, // Maximum debt
    ];
    
    // Test postpaid billing with extreme values
    for &test_usage in &test_usages {
        for &test_rate in &test_rates {
            for &test_balance in &test_balances {
                let meter_id = ((test_usage as u64).wrapping_add(test_rate as u64).wrapping_add(test_balance as u64)) % 1000000 + 1;
                
                // Create meter with postpaid billing
                let create_result = std::panic::catch_unwind(|| {
                    client.register_meter_with_mode(
                        &meter_id,
                        &user,
                        &provider,
                        &test_rate,
                        &token,
                        &BillingType::PostPaid,
                        &TestAddress::generate(&env), // device_public_key
                        &0u32, // priority_index
                    );
                });
                
                if create_result.is_err() {
                    continue;
                }
                
                // Set initial balance/debt state
                if test_balance != 0 {
                    let set_balance_result = std::panic::catch_unwind(|| {
                        if test_balance > 0 {
                            client.top_up(&meter_id, &test_balance, &user);
                        } else {
                            // Simulate debt by creating negative balance
                            // This would typically happen through usage deduction
                            client.update_usage(&meter_id, &test_usage.abs());
                        }
                    });
                    
                    if set_balance_result.is_err() {
                        continue;
                    }
                }
                
                // Test usage deduction with extreme values
                let deduction_result = std::panic::catch_unwind(|| {
                    client.update_usage(&meter_id, &test_usage);
                });
                
                if deduction_result.is_err() {
                    panic!("Usage deduction crashed with usage: {}, rate: {}, balance: {}", 
                           test_usage, test_rate, test_balance);
                }
                
                // Test claim operations which involve debt calculations
                let claim_result = std::panic::catch_unwind(|| {
                    client.claim(&meter_id);
                });
                
                // Claims should handle debt gracefully
                if let Err(_) = claim_result {
                    // May be expected for extreme debt scenarios
                }
                
                // Test top-up operations that might involve debt settlement
                let topup_amounts = vec![
                    test_usage.saturating_mul(test_rate),
                    test_usage.saturating_mul(test_rate).saturating_mul(2),
                    i128::MAX,
                    test_balance.abs(),
                    1,
                    0,
                ];
                
                for &topup_amount in &topup_amounts {
                    let topup_result = std::panic::catch_unwind(|| {
                        client.top_up(&meter_id, &topup_amount, &user);
                    });
                    
                    if topup_result.is_err() {
                        // May be expected for extreme amounts
                        continue;
                    }
                    
                    // Test claim after top-up
                    let post_topup_claim = std::panic::catch_unwind(|| {
                        client.claim(&meter_id);
                    });
                    
                    if post_topup_claim.is_err() {
                        panic!("Post top-up claim crashed with amount: {}", topup_amount);
                    }
                }
            }
        }
    }
    
    // Test collateral limit calculations
    let test_collateral_limits = vec![
        collateral_limit,
        collateral_limit.saturating_mul(1000),
        i128::MAX,
        i128::MIN,
        1,
        0,
    ];
    
    for &test_collateral in &test_collateral_limits {
        let meter_id = 777777u64;
        
        // Create meter and set collateral limit
        let create_result = std::panic::catch_unwind(|| {
            client.register_meter_with_mode(
                &meter_id,
                &user,
                &provider,
                &1000i128, // base rate
                &token,
                &BillingType::PostPaid,
                &TestAddress::generate(&env),
                &0u32,
            );
        });
        
        if create_result.is_err() {
            continue;
        }
        
        // Simulate large debt that might approach collateral limits
        let large_usage = test_collateral.saturating_mul(2);
        let debt_result = std::panic::catch_unwind(|| {
            client.update_usage(&meter_id, &large_usage);
        });
        
        if debt_result.is_err() {
            panic!("Large usage debt calculation crashed with collateral: {}", test_collateral);
        }
        
        // Test claim with large debt
        let claim_result = std::panic::catch_unwind(|| {
            client.claim(&meter_id);
        });
        
        // Should handle large debt scenarios gracefully
        if let Err(_) = claim_result {
            // May be expected for extreme debt scenarios
        }
    }
    
    // Test debt threshold scenarios
    let debt_thresholds = vec![
        -1000i128,
        -1000000i128,
        -1000000000i128,
        -i128::MAX / 2,
        -i128::MAX,
    ];
    
    for &threshold in &debt_thresholds {
        let meter_id = 666666u64;
        
        // Create meter
        let create_result = std::panic::catch_unwind(|| {
            client.register_meter_with_mode(
                &meter_id,
                &user,
                &provider,
                &1000i128,
                &token,
                &BillingType::PostPaid,
                &TestAddress::generate(&env),
                &0u32,
            );
        });
        
        if create_result.is_err() {
            continue;
        }
        
        // Simulate debt approaching threshold
        let debt_usage = threshold.abs();
        let threshold_result = std::panic::catch_unwind(|| {
            client.update_usage(&meter_id, &debt_usage);
        });
        
        if threshold_result.is_err() {
            panic!("Debt threshold calculation crashed with threshold: {}", threshold);
        }
        
        // Test behavior at debt threshold
        let threshold_claim = std::panic::catch_unwind(|| {
            client.claim(&meter_id);
        });
        
        // Should handle threshold scenarios
        if let Err(_) = threshold_claim {
            // May be expected at debt thresholds
        }
    }
    
    // Test edge case: Maximum debt scenario
    let max_debt_result = std::panic::catch_unwind(|| {
        let meter_id = 555555u64;
        client.register_meter_with_mode(
            &meter_id,
            &user,
            &provider,
            &i128::MAX, // Maximum rate
            &token,
            &BillingType::PostPaid,
            &TestAddress::generate(&env),
            &0u32,
        );
        
        // Create maximum possible debt
        client.update_usage(&meter_id, &i128::MAX);
        client.claim(&meter_id);
        
        // Try to top-up with maximum amount
        client.top_up(&meter_id, &i128::MAX, &user);
        client.claim(&meter_id);
    });
    
    // Should handle maximum debt scenario gracefully
    if let Err(_) = max_debt_result {
        // Expected for extreme edge case
    }
});
