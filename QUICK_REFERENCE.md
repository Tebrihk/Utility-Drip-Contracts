# Gas Metering Integration Quick Reference

## Quick Start: 30 Seconds

### Step 1: Add Guard to Test
```rust
#[test]
fn my_test() {
    let _guard = TestGasGuard::new("my_test");
    
    // rest of test...
}
```

### Step 2: Measure Operation
```rust
measure_gas("operation_name", ESTIMATED_GAS, || {
    // operation code
});
```

### Step 3: View Report
```rust
let report = GAS_METER.generate_report();
report.print_summary();
```

---

## Common Gas Baselines (in stroops)

```rust
GasBaseline::SIMPLE_READ              // 1M    (0.01 XLM)
GasBaseline::SIMPLE_WRITE             // 2M    (0.02 XLM)
GasBaseline::TOKEN_TRANSFER           // 3M    (0.03 XLM)
GasBaseline::STORAGE_OPERATION        // 5M    (0.05 XLM)
GasBaseline::CROSS_CONTRACT_CALL      // 10M   (0.10 XLM)

GasBaseline::REGISTER_METER           // 10M
GasBaseline::TOP_UP                   // 5M
GasBaseline::CLAIM                    // 8M
GasBaseline::UPDATE_HEARTBEAT         // 3M
GasBaseline::GROUP_TOP_UP_PER_METER   // 6M
GasBaseline::EMERGENCY_SHUTDOWN       // 2M
GasBaseline::SUBMIT_ZK_REPORT         // 50M
GasBaseline::SET_ZK_VK                // 15M
```

---

## Common Usage Patterns

### Pattern 1: Simple Measurement
```rust
#[test]
fn test_operation() {
    let _guard = TestGasGuard::new("test_operation");
    
    measure_gas("op", 5_000_000, || {
        // operation
    });
}
```

### Pattern 2: Get Statistics
```rust
let stats = GAS_METER.get_operation_statistics("op_name");
if let Some(s) = stats {
    println!("Avg: {} stroops", s.avg_gas);
}
```

### Pattern 3: Find Expensive Operations
```rust
let expensive = GAS_METER.get_expensive_operations(15_000_000);
for op in expensive {
    println!("Expensive: {} ({} stroops)", op.operation_name, op.actual_gas);
}
```

### Pattern 4: Check for Regressions
```rust
let deviations = GAS_METER.get_deviations(20.0); // 20% tolerance
assert!(deviations.is_empty(), "Gas usage regression detected");
```

### Pattern 5: Compare Implementations
```rust
measure_gas("baseline", 10_000_000, || { /* old code */ });
measure_gas("optimized", 10_000_000, || { /* new code */ });

let b = GAS_METER.get_operation_statistics("baseline");
let o = GAS_METER.get_operation_statistics("optimized");
```

### Pattern 6: Validate Constraints
```rust
let mut constraints = GasConstraints::default();
constraints.operation_limits.insert("op".to_string(), 12_000_000);

let result = validate_gas_constraints(&constraints);
assert!(result.is_valid);
```

---

## Metrics Glossary

| Metric | Meaning |
|--------|---------|
| `actual_gas` | Measured gas consumption |
| `estimated_gas` | Expected/budgeted gas |
| `efficiency_ratio` | actual / estimated (< 1 is good) |
| `variance` | actual - estimated (negative is good) |
| `variance %` | variance / estimated * 100 |

---

## Report Example

```
===== GAS METERING SUMMARY REPORT =====
Total Measurements: 15
Total Gas Consumed: 120000000 stroops
Total Estimated Gas: 150000000 stroops
Average Efficiency Ratio: 0.8000x

Operation Breakdown:
Operation                         Count     Avg Gas  Estimated     Ratio
================================================================================
create_stream                        5   10000000    10000000    1.0000x
update_rate                          5    5000000     5000000    1.0000x
withdraw_stream                      5    6000000     8000000    0.7500x
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| No measurements recorded | Ensure you have `TestGasGuard` in test |
| All measurements identical | Operations may be too small or mocked |
| High variance (> 50%) | Increase operation iterations or size |
| Hotspots not showing | Need more measurements/bigger operations |
| Constraints failing | Adjust limits or optimize operations |

---

## Integration Checklist

- [ ] Add `lazy_static = "1.4"` to dev-dependencies
- [ ] Add `pub mod gas_metrics` to lib.rs (under `#[cfg(test)]`)
- [ ] Add `TestGasGuard` to first lines of tests
- [ ] Wrap operations with `measure_gas()`
- [ ] Review report with `report.print_summary()`
- [ ] Set gas constraints for operations
- [ ] Add to CI/CD pipeline
- [ ] Document baselines for custom operations

---

## Code Template: Gas-Instrumented Test

```rust
#[test]
fn test_my_feature() {
    let _guard = TestGasGuard::new("test_my_feature");
    
    // Setup
    let env = Env::default();
    // ... setup code ...
    
    // Measure operation
    let result = measure_gas("my_operation", 10_000_000, || {
        // actual operation
    });
    
    // Verify result
    assert!(!result.is_empty());
    
    // Optional: Get stats
    let stats = GAS_METER.get_operation_statistics("my_operation");
    if let Some(s) = stats {
        println!("Gas used: {} stroops", s.avg_gas);
    }
    
    // Optional: Print report
    let report = GAS_METER.generate_report();
    report.print_summary();
}
```

---

## Advanced Features

### Hotspot Detection (Top 5 Expensive Operations)
```rust
let hotspots = get_gas_hotspots(5);
```

### Regression Check (20% Tolerance)
```rust
let deviations = GAS_METER.get_deviations(20.0);
```

### Constraint Validation
```rust
let result = validate_gas_constraints(&constraints);
result.print_report();
```

### Clear Metrics
```rust
GAS_METER.clear();
```

### Get All Statistics
```rust
let all_stats = GAS_METER.get_all_statistics();
```

---

## Files Overview

| File | Purpose |
|------|---------|
| `gas_metrics.rs` | Core metering module |
| `gas_metrics_examples.rs` | 10+ usage examples |
| `GAS_METERING_GUIDE.md` | Comprehensive guide |
| `QUICK_REFERENCE.md` | This file |

---

## Support

For questions or issues:
1. Check `GAS_METERING_GUIDE.md` for detailed documentation
2. Review `gas_metrics_examples.rs` for usage patterns
3. Check test output for specific error messages

---

## Version Info

- **Module**: gas_metrics
- **Rust Edition**: 2021
- **Dependencies**: lazy_static 1.4, proptest 1.4
- **Test Framework**: Rust built-in testing

---

**Last Updated**: 2024
