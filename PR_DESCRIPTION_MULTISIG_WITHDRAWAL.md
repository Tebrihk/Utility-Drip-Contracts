#  #98 Implement Multi-Sig Provider Withdrawal Requirement
## Description

This PR implements a 3-of-5 multi-signature withdrawal mechanism for utility providers, ensuring that large withdrawals from the contract to a company's main treasury require authorization from multiple Finance Department wallets. This "Internal Control" prevents a single rogue employee or a compromised "Hot Wallet" from stealing the company's streaming revenue, providing enterprise-grade security for on-chain operations.

## Related Issue

Fixes #98

## Type of Change

- [ ] Bug fix
- [x] New feature
- [ ] Documentation update
- [ ] Code refactoring
- [ ] Performance improvement

## Changes Made

- Added `MultiSigConfig` struct to store provider multi-sig configuration including finance wallets, required signatures, threshold amount, and active status
- Added `WithdrawalRequest` struct to track withdrawal proposals with request ID, amount, destination, proposer, approvals, expiration, and execution status
- Implemented new data keys: `MultiSigConfig`, `WithdrawalRequest`, `WithdrawalRequestCount`, `WithdrawalApproval` for storage management
- Added new error codes: `MultiSigNotConfigured`, `MultiSigAlreadyConfigured`, `InvalidFinanceWalletCount`, `InvalidSignatureThreshold`, `NotAuthorizedFinanceWallet`, `WithdrawalRequestNotFound`, `WithdrawalRequestExpired`, `WithdrawalAlreadyExecuted`, `WithdrawalAlreadyCancelled`, `InsufficientApprovals`, `AlreadyApprovedWithdrawal`, `NotApprovedByWallet`, `AmountBelowMultiSigThreshold`, `MultiSigRequiredForAmount`
- Defined constants: `MAX_FINANCE_WALLETS` (5), `MIN_FINANCE_WALLETS` (3), `DEFAULT_REQUIRED_SIGNATURES` (3), `WITHDRAWAL_REQUEST_EXPIRY` (7 days), `DEFAULT_MULTISIG_THRESHOLD` ($100,000 USD)
- Implemented `configure_multisig_withdrawal`: Initialize multi-sig configuration for a provider with 3-5 finance wallets
- Implemented `update_multisig_config`: Update existing multi-sig configuration with new wallets, signatures requirement, or threshold
- Implemented `propose_multisig_withdrawal`: Create a withdrawal request that requires multi-sig approval
- Implemented `approve_multisig_withdrawal`: Approve a pending withdrawal request (accumulates approvals)
- Implemented `execute_multisig_withdrawal`: Execute withdrawal after reaching required approval threshold
- Implemented `revoke_multisig_approval`: Revoke a previously given approval
- Implemented `cancel_multisig_withdrawal`: Cancel a pending withdrawal request
- Implemented `disable_multisig` and `enable_multisig`: Toggle multi-sig requirement on/off
- Implemented getter functions: `get_multisig_config`, `get_withdrawal_request`, `has_approved_withdrawal`, `requires_multisig`, `get_withdrawal_request_count`
- Fixed bug in `deduct_units` - corrected `get_effective_rate` parameter count
- Fixed bug in `deduct_units` - corrected `verify_usage_signature` error handling
- Fixed bug in `withdraw_earnings_path_payment` - added missing `now` parameter in `refresh_activity`

## Testing

Comprehensive tests have been added to verify all multi-sig functionality:

- `test_configure_multisig_withdrawal`: Tests initial configuration of multi-sig with 3-of-5 wallets
- `test_multisig_withdrawal_full_flow`: Tests complete workflow from proposal → approvals → execution
- `test_multisig_requires_check`: Tests threshold checking for amounts requiring multi-sig
- `test_multisig_revoke_approval`: Tests revoking approvals decrements approval count
- `test_multisig_cancel_withdrawal`: Tests cancellation of pending withdrawal requests
- `test_multisig_enable_disable`: Tests toggling multi-sig on/off
- `test_multisig_update_config`: Tests updating configuration with new wallets and thresholds
- `test_multisig_get_withdrawal_request_count`: Tests request counter increments correctly
- `test_multisig_has_approved_withdrawal`: Tests approval status checking per wallet
- `test_multisig_withdrawal_expiration`: Tests that expiration time is set correctly (7 days)
- `test_multisig_insufficient_approvals_cannot_execute`: Tests that execution fails without sufficient approvals
- `test_multisig_invalid_wallet_count`: Tests wallet count validation (minimum 3 wallets)

## Checklist

- [x] Code follows project style
- [x] Self-reviewed my code
- [x] Commented complex code
- [x] Updated documentation
- [x] No new warnings
- [x] Added tests (if applicable)

## Screenshots (if applicable)

N/A - Smart contract implementation

## Additional Notes

### Security Considerations

1. **3-of-5 Multi-Sig**: Requires 3 out of 5 authorized Finance Department wallets to approve large withdrawals
2. **Threshold Protection**: Only withdrawals above the configured threshold (default $100,000) require multi-sig
3. **Expiration**: Withdrawal requests automatically expire after 7 days to prevent stale requests
4. **Atomic Operations**: Approvals and executions are atomic and auditable via events
5. **Revocation Support**: Finance wallets can revoke their approvals before execution
6. **Cancellation**: Original proposer or provider can cancel pending requests

### Event Emissions

The implementation emits events for all multi-sig operations:
- `MSigCfg`: Multi-sig configured
- `MSigUpd`: Configuration updated
- `MSigProp`: Withdrawal proposed
- `MSigAppr`: Approval added
- `MSigExec`: Withdrawal executed
- `MSigRvke`: Approval revoked
- `MSigCanc`: Withdrawal cancelled
- `MSigOff`: Multi-sig disabled
- `MSigOn`: Multi-sig enabled

### Usage Example

```rust
// 1. Provider configures multi-sig with 5 finance wallets
configure_multisig_withdrawal(provider, finance_wallets, 3, 100_000_00);

// 2. Finance wallet proposes a $150,000 withdrawal
let request_id = propose_multisig_withdrawal(provider, meter_id, 150_000_00, treasury);

// 3. Two more finance wallets approve
approve_multisig_withdrawal(provider, request_id); // Wallet 2
approve_multisig_withdrawal(provider, request_id); // Wallet 3

// 4. Execute with 3 approvals (threshold met)
execute_multisig_withdrawal(provider, request_id);
```
