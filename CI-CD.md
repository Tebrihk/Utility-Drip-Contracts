# CI/CD Pipeline for Utility Drip Contracts

This document describes the automated testing pipeline implemented for the Utility Drip Contracts project.

## 🔄 Workflow Overview

The GitHub Actions workflow (`.github/workflows/test.yml`) automatically runs on:
- **Push to main branch** - Ensures main branch is always tested
- **Pull Requests to main** - Prevents breaking changes from being merged

## ✅ Testing Stages

### 1. Environment Setup
- **Rust Toolchain**: Installs stable Rust with WASM target
- **Stellar CLI**: Installs Stellar CLI v25.1.0 for contract interaction
- **Dependency Caching**: Caches Cargo dependencies for faster builds

### 2. Code Quality Checks
- **Formatting**: `cargo fmt --all -- --check` ensures consistent code formatting
- **Linting**: `cargo clippy --target wasm32-unknown-unknown -- -D warnings` catches potential issues

### 3. Build & Test
- **WASM Build**: `cargo build --target wasm32-unknown-unknown --release` builds smart contract
- **Unit Tests**: `cargo test` runs all unit tests including fuzz tests
- **Fuzz Tests**: Detects and validates fuzz testing infrastructure

## 🧪 Fuzz Testing Integration

The pipeline includes automatic detection of fuzz tests:
- Checks for `contracts/utility_contracts/fuzz/` directory
- Installs `cargo-fuzz` if fuzz tests are present
- Validates fuzz testing infrastructure availability

## 📊 Test Coverage

### Current Test Suites
1. **Unit Tests**: Standard contract functionality tests
2. **Fuzz Tests**: 
   - Debt calculation underflow protection
   - Extreme usage scenarios
   - Balance handling edge cases
   - Arithmetic overflow protection

### Acceptance Criteria Validation
- ✅ Workflow runs on push to main
- ✅ `cargo test` passes successfully  
- ✅ Code formatting validated
- ✅ Clippy linting passes
- ✅ WASM build succeeds
- ✅ Fuzz tests infrastructure available

## 🔧 Pipeline Configuration

### Environment Variables
- `CARGO_TERM_COLOR: always` - Ensures colored output in logs

### Build Matrix
- **OS**: Ubuntu Latest (ubuntu-latest)
- **Target**: wasm32-unknown-unknown (for Soroban contracts)
- **Rust Version**: Stable with required components

## 📈 Pipeline Benefits

1. **Prevents Breaking Changes**: Every PR is automatically tested
2. **Code Quality**: Enforces formatting and linting standards
3. **Fast Feedback**: Caching and parallel execution provide quick results
4. **Comprehensive Testing**: Unit + fuzz testing coverage
5. **WASM Compatibility**: Ensures contracts build for target platform

## 🚀 Usage

### Automatic Execution
- No manual intervention required
- Tests run automatically on git events
- Results displayed in GitHub Actions UI

### Local Development
```bash
# Run same tests locally
cargo fmt --all -- --check
cargo clippy --target wasm32-unknown-unknown -- -D warnings
cargo build --target wasm32-unknown-unknown --release
cargo test

# Run fuzz tests (if available)
cd contracts/utility_contracts/fuzz
cargo fuzz run debt_calculation_fuzz -- -max_total_time 30
```

## 📋 Test Results Summary

The pipeline generates a summary in GitHub Actions including:
- ✅ Unit tests status
- ✅ Clippy linting status  
- ✅ Code formatting status
- ✅ WASM build status
- ✅ Fuzz tests availability

This ensures every pull request maintains code quality and prevents regressions in smart contract logic.
