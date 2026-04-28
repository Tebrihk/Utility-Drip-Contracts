# Automated Gas Metering Metrics Implementation Guide

## Overview

This document describes the automated gas metering metrics system for Soroban smart contract unit tests. The system provides comprehensive gas measurement, benchmarking, and analytics capabilities integrated into the test suite.

## Features

### 1. **Automated Gas Tracking**
- Capture gas consumption for each operation
- Track gas usage across test suites
- Minimal measurement overhead

### 2. **Benchmarking Capabilities**
- Compare actual vs estimated gas costs
- Identify regressions
- Track optimization impact

### 3. **Comprehensive Analytics**
- Per-operation statistics (min, max, avg)
- Gas hotspot identification
- Efficiency ratio calculations

### 4. **Performance Monitoring**
- Detect gas usage deviations
- Compare implementations (baseline vs optimized)
- Generate detailed reports

### 5. **Constraint Validation**
- Define operation-level gas limits
- Validate total gas budgets
- Check efficiency ratios

## Architecture

### Core Components

#### `GasMeter`
Global metrics collector using `lazy_static`:
- Records measurements
- Manages test context stack
- Generates statistics and reports

#### `GasMeasurement`
Individual operation measurement:
- Operation name
- Estimated vs actual gas
- Timestamp
- Test context

#### `GasStatistics`
Aggregated metrics for an operation:
- Count of measurements
- Min, max, average gas
- Efficiency ratio
- Variance percentage

#### `GasReport`
Comprehensive summary report:
- Total gas consumed
- Per-operation breakdown
- Average efficiency
- Pretty printing

### Supporting Structures

#### `GasBaseline`
Reference gas costs for common operations (in stroops):
- `SIMPLE_READ`: 1,000,000
- `SIMPLE_WRITE`: 2,000,000
- `TOKEN_TRANSFER`: 3,000,000
- `STORAGE_OPERATION`: 5,000,000
- `CROSS_CONTRACT_CALL`: 10,000,000
- Contract-specific operations from `GasCostEstimator`

#### `GasConstraints`
Configuration for validation:
- Operation-level limits (BTreeMap)
- Total gas limit
- Minimum efficiency ratio

## Usage Patterns

### Basic Pattern: Measure a Single Operation

```rust
#[test]
fn test_meter_registration_gas() {
    let _guard = TestGasGuard::new("test_meter_registration_gas");

    // Measure operation with pre-defined estimated cost
    measure_gas("register_meter", GasBaseline::REGISTER_METER, || {
        // Perform registration
        // ...
    });

    // Metrics automatically recorded
}
```

### Pattern: Batch Operation Profiling

```rust
#[test]
fn test_batch_operations() {
    let _guard = TestGasGuard::new("test_batch_operations");

    let operations = vec![
        ("create_stream", GasBaseline::SIMPLE_WRITE),
        ("update_rate", GasBaseline::STORAGE_OPERATION),
        ("withdraw", GasBaseline::TOKEN_TRANSFER),
    ];

    for (op_name, estimated) in operations {
        measure_gas(op_name, estimated, || {
            // Operation logic
        });
    }

    let report = GAS_METER.generate_report();
    report.print_summary();
}
```

### Pattern: Comparative Benchmarking

```rust
#[test]
fn test_optimization_impact() {
    let _guard = TestGasGuard::new("test_optimization_impact");

    // Baseline implementation
    measure_gas("baseline_calc", 10_000_000, || {
        // Original calculation
    });

    // Optimized implementation
    measure_gas("optimized_calc", 10_000_000, || {
        // Improved calculation
    });

    let baseline = GAS_METER.get_operation_statistics("baseline_calc");
    let optimized = GAS_METER.get_operation_statistics("optimized_calc");

    if let (Some(b), Some(o)) = (baseline, optimized) {
        let improvement = ((b.avg_gas - o.avg_gas) as f64 / b.avg_gas as f64) * 100.0;
        println!("Optimization improved gas by {:.2}%", improvement);
    }
}
```

### Pattern: Regression Detection

```rust
#[test]
fn test_regression_detection() {
    let _guard = TestGasGuard::new("test_regression_detection");

    // Run multiple iterations of same operation
    for _ in 0..10 {
        measure_gas("streaming_operation", 10_000_000, || {
            // Operation
        });
    }

    // Find deviations > 20%
    let deviations = GAS_METER.get_deviations(20.0);
    
    if !deviations.is_empty() {
        panic!("Gas regression detected in {} operations", deviations.len());
    }
}
```

### Pattern: Hotspot Analysis

```rust
#[test]
fn test_identify_hotspots() {
    // ... run multiple operations ...

    // Get top 5 most expensive
    let hotspots = get_gas_hotspots(5);
    
    for (op_name, total_gas) in hotspots {
        println!("Hotspot: {} - {} stroops", op_name, total_gas);
    }
}
```

### Pattern: Constraint Validation

```rust
#[test]
fn test_gas_constraints() {
    // ... run operations ...

    let mut constraints = GasConstraints::default();
    constraints.operation_limits.insert("expensive_op".to_string(), 15_000_000);
    constraints.total_gas_limit = Some(100_000_000);
    constraints.min_efficiency_ratio = Some(1.2);

    let result = validate_gas_constraints(&constraints);
    result.print_report();
    
    assert!(result.is_valid, "Gas constraints violated!");
}
```

## Integration with Test Suite

### Step 1: Add Module to lib.rs

```rust
#[cfg(test)]
pub mod gas_metrics;
```

### Step 2: Add Dependencies to Cargo.toml

```toml
[dev-dependencies]
lazy_static = "1.4"
```

### Step 3: Use in Existing Tests

Update existing test functions to measure gas:

**Before:**
```rust
#[test]
fn test_create_stream() {
    // Test implementation
}
```

**After:**
```rust
#[test]
fn test_create_stream() {
    let _guard = TestGasGuard::new("test_create_stream");
    
    measure_gas("create_stream", GasBaseline::REGISTER_METER, || {
        // Test implementation
    });
    
    // Verify results
}
```

## Metrics Collection

### Metrics Captured

For each operation:
- **Operation Name**: Identifier for the operation
- **Estimated Gas**: Pre-calculated expected cost
- **Actual Gas**: Measured gas consumption
- **Timestamp**: When measurement was taken
- **Test Context**: Which test is running

### Statistics Calculated

- **Count**: Number of measurements
- **Min/Max**: Minimum and maximum gas
- **Average**: Mean gas consumption
- **Total**: Sum of all gas
- **Efficiency Ratio**: actual_gas / estimated_gas
- **Variance**: actual_gas - estimated_gas
- **Variance %**: ((actual - estimated) / estimated) * 100

## Report Generation

### Summary Report

```
===== GAS METERING SUMMARY REPORT =====
Total Measurements: 25
Total Gas Consumed: 250000000 stroops
Total Estimated Gas: 300000000 stroops
Average Efficiency Ratio: 0.8333x

Operation Breakdown:
Operation                         Count     Avg Gas  Estimated     Ratio
================================================================================
create_stream                        5   10000000    10000000    1.0000x
update_rate                         10    5000000     5000000    1.0000x
withdraw                            10    8000000     8000000    1.0000x
```

### Detailed Report

Includes per-operation statistics:
- Min/max gas values
- Average consumption
- Total gas and estimates
- Variance percentage

### Validation Report

Shows constraint validation results:
- Passed/failed status
- List of violations
- List of warnings

## Best Practices

### 1. Use Realistic Test Data
- Match production patterns
- Use similar operation volumes
- Test edge cases

### 2. Set Appropriate Baselines
- Use `GasBaseline` constants for common operations
- Adjust for contract-specific operations
- Document any custom baselines

### 3. Monitor for Regressions
- Track gas metrics across commits
- Set reasonable variance tolerances
- Alert on unexpected changes

### 4. Optimize Systematically
- Benchmark before and after changes
- Validate optimization improvements
- Document gas savings

### 5. Validate Against Production
- Compare test estimates with actual Soroban costs
- Allow for test vs production variance
- Adjust baselines as needed

## Gas Budget Planning

### Estimating Monthly Costs

Use `GasCostEstimator::estimate_provider_monthly_cost()`:
- Number of meters
- Percentage of group meters
- Returns estimated monthly cost

### Per-Meter Costs

Breakdown by operation type:
- Registration: 10M stroops
- Claims: 240M stroops/month (30 claims)
- Heartbeats: 2160M stroops/month (720 heartbeats)
- Top-ups: 20M stroops/month (4 top-ups)

## Troubleshooting

### Issue: Gas measurements seem inaccurate

**Solution:** 
- Ensure operations are actually performing work
- Check that test environment matches production
- Verify baseline estimates are appropriate

### Issue: High variance in measurements

**Solution:**
- Increase measurement iterations
- Check for system load
- Use larger operations (more time elapsed)

### Issue: Hotspots not appearing

**Solution:**
- Measure more operations
- Use larger batch sizes
- Check that operations are expensive enough

## Advanced Usage

### Custom Gas Metering for Contract-Specific Operations

```rust
// Define contract-specific baseline
const CUSTOM_OPERATION_GAS: i128 = 25_000_000;

measure_gas("custom_operation", CUSTOM_OPERATION_GAS, || {
    // Operation implementation
});
```

### Performance Regression Test Suite

```rust
let mut baseline = PerformanceBaseline::new();
baseline.add_baseline("op1".to_string(), 5_000_000);
baseline.add_baseline("op2".to_string(), 10_000_000);

// Run tests

let regressions = baseline.check_regression(10.0); // 10% tolerance
assert!(regressions.is_empty());
```

### Gas Scaling Analysis

Track how gas changes with operation complexity:

```rust
for size in [10, 50, 100, 500] {
    measure_gas(format!("op_size_{}", size), size as i128 * 10_000, || {
        // Variable operation
    });
}
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
- name: Run Gas Metering Tests
  run: cargo test --lib gas_metrics
  
- name: Check Gas Constraints
  run: cargo test --lib gas_constraints_validation
```

### Storing Historical Data

- Export `GasReport` to JSON
- Track metrics over time
- Identify trends and regressions
- Alert on significant changes

## Files Added

1. **`gas_metrics.rs`**: Core metering module
   - `GasMeter`: Global metrics collector
   - `GasMeasurement`: Individual measurement
   - `GasStatistics`: Aggregated stats
   - Measurement functions and macros

2. **`gas_metrics_examples.rs`**: Usage examples
   - Basic measurement
   - Batch profiling
   - Comparative benchmarking
   - Hotspot analysis
   - Constraint validation

3. **`stream_balance_property_tests.rs`**: Property-based tests (added with this PR)
   - Stream balance invariants
   - Withdrawal sequences
   - Rate change handling
   - Edge cases

## Next Steps

1. **Integrate with Existing Tests**: Update current test suite to use gas metrics
2. **Set Baselines**: Establish gas cost baselines for all operations
3. **Monitor Trends**: Track gas usage across commits
4. **Optimize**: Use metrics to identify and fix inefficient operations
5. **Document**: Add gas requirements to contract documentation

## References

- [Soroban Documentation](https://developers.stellar.org/docs/build/smart-contracts)
- [Gas Costs and Budgets](https://developers.stellar.org/docs/learn/smart-contracts/concepts/gas-and-fees)
- [GasCostEstimator](./gas_estimator.rs): Existing gas estimation module
