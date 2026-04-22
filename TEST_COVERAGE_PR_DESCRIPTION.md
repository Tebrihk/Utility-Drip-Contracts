## Summary

This PR implements automated test coverage enforcement using cargo-tarpaulin for the Utility-Drip smart contracts project. The setup ensures that code quality is maintained by requiring a minimum of 85% test coverage for all pull requests.

## Changes Made

### 1. Dependencies Configuration
- **Updated `Cargo.toml`**: Added `cargo-tarpaulin = "0.27"` as a workspace dev-dependency
- **Created `tarpaulin.toml`**: Configuration file with 85% coverage threshold and multiple output formats

### 2. GitHub Action Workflow
- **Created `.github/workflows/test-coverage.yml`**: Comprehensive CI/CD pipeline that:
  - Runs on all pull requests and pushes to main/master branches
  - Installs Rust toolchain and cargo-tarpaulin
  - Generates coverage reports in HTML, XML, and JSON formats
  - Enforces 85% minimum coverage threshold
  - Uploads coverage to Codecov for visualization
  - Provides coverage artifacts for download

### 3. Coverage Features
- **Multiple Output Formats**: HTML (visual), XML (Codecov), JSON (threshold checking)
- **Workspace Coverage**: Analyzes all contracts in the workspace
- **Automated Enforcement**: Fails PRs if coverage drops below 85%
- **Artifact Storage**: HTML reports available for 30 days
- **Integration**: Works with existing Soroban smart contract structure

## Technical Details

### Coverage Configuration
```toml
[config]
fail-under = 85.0
output-types = ["Html", "Xml", "Json"]
include-tests = true
debug = true
workspace = true
```

### Workflow Features
- **Caching**: Optimized build times with cargo registry caching
- **Threshold Checking**: Automated coverage percentage validation
- **Reporting**: Integration with Codecov and artifact storage
- **Error Handling**: Graceful failure handling with clear error messages

## Benefits

1. **Quality Assurance**: Ensures comprehensive test coverage across all smart contracts
2. **Automated Enforcement**: Prevents merging of low-quality code
3. **Visibility**: Clear coverage reports available to all contributors
4. **Integration**: Seamlessly works with existing development workflow
5. **Flexibility**: Configurable threshold and output formats

## Testing

The workflow has been designed to:
- Run on every pull request to main/master branches
- Generate comprehensive coverage reports
- Provide clear feedback on coverage status
- Allow manual verification through artifact downloads

## Usage

Once merged, the workflow will automatically:
1. Run on all new pull requests
2. Check coverage against the 85% threshold
3. Fail PRs that don't meet the requirement
4. Provide detailed coverage reports for review

## Files Added/Modified

- `Cargo.toml` - Added cargo-tarpaulin dependency
- `tarpaulin.toml` - New configuration file
- `.github/workflows/test-coverage.yml` - New GitHub Action workflow

This implementation provides a robust foundation for maintaining code quality and ensuring comprehensive test coverage across the Utility-Drip smart contracts project.
