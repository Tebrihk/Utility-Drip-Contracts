#!/bin/bash

# Local CI/CD Validation Script
# This script runs the same checks as the GitHub Actions workflow

echo "🔄 Running CI/CD Pipeline Validation locally..."
echo

# Set environment variable
export CARGO_TERM_COLOR=always

echo "📦 Installing Rust toolchain and dependencies..."
rustup target add wasm32-unknown-unknown
cargo install cargo-fuzz 2>/dev/null || echo "⚠️  cargo-fuzz already installed"

echo
echo "🔍 Running code quality checks..."

# Check formatting
echo "  📝 Checking code formatting..."
if cargo fmt --all -- --check; then
    echo "  ✅ Code formatting: PASSED"
else
    echo "  ❌ Code formatting: FAILED"
    echo "  Run 'cargo fmt' to fix formatting issues"
    exit 1
fi

# Run clippy
echo "  🔍 Running clippy linting..."
if cargo clippy --target wasm32-unknown-unknown -- -D warnings; then
    echo "  ✅ Clippy linting: PASSED"
else
    echo "  ❌ Clippy linting: FAILED"
    echo "  Fix clippy warnings before committing"
    exit 1
fi

echo
echo "🏗️ Building contract..."

# Build WASM contract
echo "  🏗️ Building WASM contract..."
if cargo build --target wasm32-unknown-unknown --release; then
    echo "  ✅ WASM build: PASSED"
else
    echo "  ❌ WASM build: FAILED"
    exit 1
fi

echo
echo "🧪 Running tests..."

# Run unit tests
echo "  🧪 Running unit tests..."
if cargo test; then
    echo "  ✅ Unit tests: PASSED"
else
    echo "  ❌ Unit tests: FAILED"
    exit 1
fi

# Check fuzz tests
echo "  🔍 Checking fuzz tests..."
if [ -d "contracts/utility_contracts/fuzz" ]; then
    echo "  ✅ Fuzz tests: AVAILABLE"
    echo "  📁 Fuzz test directory found"
    
    # List available fuzz targets
    if [ -f "contracts/utility_contracts/fuzz/Cargo.toml" ]; then
        echo "  🎯 Available fuzz targets:"
        grep -A 1 "\[\[bin\]" contracts/utility_contracts/fuzz/Cargo.toml | grep "name" | sed 's/.*name = "//g' | sed 's/"//g' | sed 's/^/    - /'
    fi
else
    echo "  ⚠️  Fuzz tests: NOT FOUND"
fi

echo
echo "📊 Pipeline Summary:"
echo "  ✅ Code formatting: PASSED"
echo "  ✅ Clippy linting: PASSED"  
echo "  ✅ WASM build: PASSED"
echo "  ✅ Unit tests: PASSED"
if [ -d "contracts/utility_contracts/fuzz" ]; then
    echo "  ✅ Fuzz tests: AVAILABLE"
else
    echo "  ⚠️  Fuzz tests: NOT AVAILABLE"
fi

echo
echo "🎉 All checks passed! Your code is ready for commit/PR."
echo "💡 Run this script before committing to ensure CI/CD will pass."
