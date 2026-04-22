# Local CI/CD Validation Script (PowerShell)
# This script runs the same checks as the GitHub Actions workflow

Write-Host "🔄 Running CI/CD Pipeline Validation locally..." -ForegroundColor Green
Write-Host ""

# Set environment variable
$env:CARGO_TERM_COLOR = "always"

Write-Host "📦 Installing Rust toolchain and dependencies..." -ForegroundColor Blue
rustup target add wasm32-unknown-unknown
cargo install cargo-fuzz 2>$null || Write-Host "⚠️  cargo-fuzz already installed" -ForegroundColor Yellow

Write-Host ""
Write-Host "🔍 Running code quality checks..." -ForegroundColor Blue

# Check formatting
Write-Host "  📝 Checking code formatting..." -ForegroundColor Cyan
$cargo_fmt_result = cargo fmt --all -- --check
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ Code formatting: PASSED" -ForegroundColor Green
} else {
    Write-Host "  ❌ Code formatting: FAILED" -ForegroundColor Red
    Write-Host "  Run 'cargo fmt' to fix formatting issues" -ForegroundColor Yellow
    exit 1
}

# Run clippy
Write-Host "  🔍 Running clippy linting..." -ForegroundColor Cyan
$cargo_clippy_result = cargo clippy --target wasm32-unknown-unknown -- -D warnings
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ Clippy linting: PASSED" -ForegroundColor Green
} else {
    Write-Host "  ❌ Clippy linting: FAILED" -ForegroundColor Red
    Write-Host "  Fix clippy warnings before committing" -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "🏗️ Building contract..." -ForegroundColor Blue

# Build WASM contract
Write-Host "  🏗️ Building WASM contract..." -ForegroundColor Cyan
$cargo_build_result = cargo build --target wasm32-unknown-unknown --release
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ WASM build: PASSED" -ForegroundColor Green
} else {
    Write-Host "  ❌ WASM build: FAILED" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "🧪 Running tests..." -ForegroundColor Blue

# Run unit tests
Write-Host "  🧪 Running unit tests..." -ForegroundColor Cyan
$cargo_test_result = cargo test
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ Unit tests: PASSED" -ForegroundColor Green
} else {
    Write-Host "  ❌ Unit tests: FAILED" -ForegroundColor Red
    exit 1
}

# Check fuzz tests
Write-Host "  🔍 Checking fuzz tests..." -ForegroundColor Cyan
$fuzz_dir = "contracts\utility_contracts\fuzz"
if (Test-Path $fuzz_dir) {
    Write-Host "  ✅ Fuzz tests: AVAILABLE" -ForegroundColor Green
    Write-Host "  📁 Fuzz test directory found" -ForegroundColor Cyan
    
    # List available fuzz targets
    $cargo_toml = "contracts\utility_contracts\fuzz\Cargo.toml"
    if (Test-Path $cargo_toml) {
        Write-Host "  🎯 Available fuzz targets:" -ForegroundColor Cyan
        $content = Get-Content $cargo_toml
        $lines = $content -split "`n"
        foreach ($line in $lines) {
            if ($line -match "name = ") {
                $target = $line -replace ".*name = " -replace ".*"
                Write-Host "    - $target" -ForegroundColor White
            }
        }
    }
} else {
    Write-Host "  ⚠️  Fuzz tests: NOT FOUND" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "📊 Pipeline Summary:" -ForegroundColor Green
Write-Host "  ✅ Code formatting: PASSED" -ForegroundColor Green
Write-Host "  ✅ Clippy linting: PASSED" -ForegroundColor Green  
Write-Host "  ✅ WASM build: PASSED" -ForegroundColor Green
Write-Host "  ✅ Unit tests: PASSED" -ForegroundColor Green
if (Test-Path $fuzz_dir) {
    Write-Host "  ✅ Fuzz tests: AVAILABLE" -ForegroundColor Green
} else {
    Write-Host "  ⚠️  Fuzz tests: NOT AVAILABLE" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "🎉 All checks passed! Your code is ready for commit/PR." -ForegroundColor Green
Write-Host "💡 Run this script before committing to ensure CI/CD will pass." -ForegroundColor Cyan
