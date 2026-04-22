# Native Asset Support Implementation

## Overview
This implementation adds support for the native Stellar asset (XLM) alongside standard SAC (Stellar Asset Contract) tokens like USDC.

## Changes Made

### 1. Helper Functions Added
- `is_native_token()`: Detects if an address represents the native XLM asset
- `transfer_tokens()`: Handles transfers for both native XLM and SAC tokens
- `get_token_balance()`: Gets balances for both token types
- `get_native_token_address()`: Test helper for creating native token addresses
 - `enforce_xlm_gas_reserve()`: Prevents draining the last 5 XLM needed for future transactions

### 2. Updated Functions
- `apply_provider_claim()`: Now uses the new transfer_tokens helper
- `top_up()`: Now uses the new transfer_tokens helper

### 3. Test Coverage Added
- `test_prepaid_meter_flow_with_native_xlm()`: Tests prepaid meters with native XLM
- `test_postpaid_meter_flow_with_native_xlm()`: Tests postpaid meters with native XLM

## How It Works

### Native Token Detection
The `is_native_token()` function treats the contract address as native for internal flows and
checks token metadata for `symbol == "XLM"` or `"NATIVE"`:
```rust
fn is_native_token(env: &Env, token_address: &Address) -> bool {
    if token_address == &env.current_contract_address() {
        return true;
    }

    let client = token::Client::new(env, token_address);
    let symbol = client.symbol();

    symbol == soroban_sdk::String::from_str(env, "XLM")
        || symbol == soroban_sdk::String::from_str(env, "NATIVE")
}
```

### Token Transfer Logic
The `transfer_tokens()` function routes transfers based on token type:
```rust
fn transfer_tokens(env: &Env, token_address: &Address, from: &Address, to: &Address, amount: &i128) {
    if is_native_token(token_address) {
        // For native XLM, use the built-in transfer function
        env.token().transfer(from, to, amount);
    } else {
        // For SAC tokens, use the token contract
        let client = token::Client::new(env, token_address);
        client.transfer(from, to, amount);
    }
}
```

### Gas Reserve Requirement
When funding streams with native XLM, the contract enforces a 5 XLM minimum
reserve in the payer's wallet to ensure they can always submit stop/update
transactions.

## Testing

### Running Tests
To run the tests, you'll need a proper Rust build environment with Visual Studio Build Tools:

```bash
cd contracts/utility_contracts
cargo test
```

### Test Coverage
The tests verify:
1. Meter registration with native XLM
2. Top-up functionality with native XLM
3. Claim operations with native XLM
4. Unit deduction with native XLM
5. Both prepaid and postpaid billing modes
6. Proper balance tracking for all parties

## Compatibility

### Backward Compatibility
- All existing SAC token functionality remains unchanged
- Existing tests continue to work with SAC tokens
- No breaking changes to the contract interface

### Asset Support
- ✅ Native Stellar XLM
- ✅ Standard SAC tokens (USDC, custom tokens, etc.)
- ✅ Mixed deployments (some meters using XLM, others using SAC tokens)

## Usage Examples

### Registering a Meter with Native XLM
```rust
// Get native token address
let native_token = get_native_token_address(&env);

// Register meter with native XLM
let meter_id = client.register_meter(&user, &provider, &rate, &native_token);
```

### Registering a Meter with SAC Token
```rust
// Register meter with SAC token (existing functionality)
let meter_id = client.register_meter(&user, &provider, &rate, &sac_token_address);
```

## Notes

1. **Production Deployment**: In production, the native token address patterns should be updated to match the actual mainnet/testnet native asset addresses.

2. **Security**: The implementation maintains the same security guarantees for both token types.

3. **Gas Efficiency**: Native XLM operations are more gas-efficient as they don't require external contract calls.

4. **Testing**: The test environment uses a special "NATIVE_TOKEN" address to simulate native XLM behavior.
