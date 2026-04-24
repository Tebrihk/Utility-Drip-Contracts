# Utility Drip Contracts

A Soroban smart contract for utility metering and billing with gas buffer functionality to ensure reliable provider withdrawals during network congestion.

## Features

### Core Functionality
- **Utility Metering**: Track energy consumption with precision billing
- **Prepaid & Postpaid Billing**: Support for both billing models
- **Provider Withdrawals**: Automated daily withdrawal limits (10% of total pool)
- **Usage Tracking**: Detailed watt-hour consumption data
- **Heartbeat Monitoring**: Detect offline meters automatically

### 🆕 Gas Buffer Feature
The Gas Buffer feature ensures **100% Service Availability** even during periods of extreme Stellar network congestion:

- **Pre-paid Gas**: Providers can deposit XLM as a gas buffer during initialization
- **Automatic Fallback**: When network fees spike, the contract uses the gas buffer to ensure withdrawal transactions clear
- **Critical Utility Payouts**: Guarantees that essential utility payments succeed even in stressed blockchain environments
- **Buffer Management**: Providers can top-up, withdraw, and monitor their gas buffer balance

## Project Structure

```text
.
├── contracts
│   └── utility_contracts
│       ├── src
│       │   ├── lib.rs         # Main contract implementation
│       │   └── test.rs        # Comprehensive test suite
│       └── Cargo.toml
├── Cargo.toml
├── README.md
└── HARDWARE.md
```

## Gas Buffer Implementation

### Key Components

1. **GasBuffer Structure**:
   ```rust
   pub struct GasBuffer {
       pub balance: i128,
       pub last_top_up: u64,
       pub provider: Address,
       pub token: Address,
   }
   ```

2. **Constants**:
   - `MIN_GAS_BUFFER`: 100 XLM (minimum required buffer)
   - `MAX_GAS_BUFFER`: 10,000 XLM (maximum buffer capacity)
   - `GAS_BUFFER_TOP_UP_THRESHOLD`: 200 XLM (auto-top up trigger)

3. **Functions**:
   - `initialize_gas_buffer()`: Set up initial gas buffer
   - `top_up_gas_buffer()`: Add funds to existing buffer
   - `withdraw_from_gas_buffer()`: Remove excess funds (maintaining minimum)
   - `get_gas_buffer_balance()`: Check current buffer status

### How It Works

1. **Initialization**: Provider sets up gas buffer with minimum 100 XLM
2. **Normal Operation**: Regular withdrawals use standard token transfers
3. **High Fee Detection**: When network congestion is detected, contract automatically:
   - Deducts gas fee from buffer
   - Ensures withdrawal transaction succeeds
   - Emits `GasBufferUsed` event for transparency
4. **Buffer Management**: Provider can monitor and replenish buffer as needed

## Deployed Contract
- **Network:** Stellar Testnet
- **Contract ID:** CB7PSJZALNWNX7NLOAM6LOEL4OJZMFPQZJMIYO522ZSACYWXTZIDEDSS

## Additional Documentation
- [Hardware Spec Integration](HARDWARE.md)

## Usage Examples

### Setting Up Gas Buffer
```rust
// Initialize gas buffer with 500 XLM
contract.initialize_gas_buffer(
    &provider_address,
    &xlm_token_address,
    &500
);
```

### Checking Buffer Status
```rust
let buffer_balance = contract.get_gas_buffer_balance(&provider_address);
println!("Current gas buffer: {} XLM", buffer_balance);
```

### Topping Up Buffer
```rust
// Add 200 XLM to gas buffer
contract.top_up_gas_buffer(
    &provider_address,
    &xlm_token_address,
    &200
);
```

## Benefits

- **Reliability**: Ensures critical utility payments never fail due to network congestion
- **Reputation**: Maintains 100% service availability guarantee
- **Flexibility**: Providers control their buffer size and management
- **Transparency**: All gas buffer operations emit events for monitoring
- **Cost-Effective**: Only uses buffer when necessary, preserves provider capital
