# Pull Request: Meter Simulator CLI Tool

## Summary

This PR introduces a comprehensive Node.js CLI tool that mimics an ESP32 sending usage data to the Utility Drip smart contracts for local development and testing. The tool provides realistic energy consumption simulation with proper cryptographic authentication and multiple communication methods.

## Features Added

### 🔐 Device Authentication
- Ed25519 key pair generation for secure device authentication
- Cryptographic signing of all usage data matching contract requirements
- Public key registration and verification system
- Timestamp validation to prevent replay attacks

### 📊 Realistic Usage Simulation
- **Three simulation modes**: Realistic, Surge, and Low consumption patterns
- **Peak/off-peak pricing**: Automatic rate calculation based on UTC time (18:00-21:00 peak)
- **Variance and randomness**: 30% variance with random surge events
- **Configurable base rates**: Flexible rate settings per meter

### 📡 Communication Methods
- **Direct contract calls**: Submit usage data directly to Soroban contracts
- **MQTT publishing**: Full MQTT integration matching real ESP32 behavior
- **Multiple topics**: Usage, heartbeat, status, and command handling
- **QoS support**: Configurable quality of service levels

### 🛠️ CLI Commands
```bash
# Generate device keys
meter-simulator generate-keys

# Register meter with contract
meter-simulator register --keys device-keys.json --user USER --provider PROVIDER

# Start simulation
meter-simulator simulate --config meter-config.json --mode realistic

# Send single reading
meter-simulator send-reading --config meter-config.json --watts 250

# Check meter status
meter-simulator status --config meter-config.json
```

## Files Added

### Core Implementation
- `meter-simulator/package.json` - Project configuration and dependencies
- `meter-simulator/src/index.js` - Main CLI entry point with Commander.js
- `meter-simulator/src/config.js` - Configuration management with environment variables
- `meter-simulator/src/meter-device.js` - Device simulation logic and usage generation
- `meter-simulator/src/contract-interface.js` - Stellar/Soroban contract integration
- `meter-simulator/src/mqtt-publisher.js` - MQTT client and publishing logic

### Documentation and Examples
- `meter-simulator/README.md` - Comprehensive documentation
- `meter-simulator/.env.example` - Environment configuration template
- `meter-simulator/examples/basic-usage.js` - Usage examples and demonstrations
- `meter-simulator/scripts/setup.sh` - Linux/macOS setup script
- `meter-simulator/scripts/setup.ps1` - Windows setup script

### Testing and Validation
- `meter-simulator/tests/meter-device.test.js` - Unit tests for device simulation
- `meter-simulator/test-validation.js` - Structure validation script

## Technical Implementation

### Contract Integration
The simulator integrates with the existing Utility Drip contract:

- **SignedUsageData Structure**: Matches the contract's expected format exactly
- **Ed25519 Signatures**: Compatible with contract's signature verification
- **Peak Hour Detection**: Implements the same 18:00-21:00 UTC peak hours
- **Rate Calculation**: Applies 1.5x multiplier during peak hours
- **Precision Handling**: Uses 1000x precision factor matching contract

### Security Features
- **Private Key Protection**: Keys stored locally, never transmitted
- **Signature Verification**: All data cryptographically signed
- **Timestamp Validation**: Prevents replay attacks with 5-minute window
- **Usage Limits**: Enforces maximum usage per update (1 billion kWh)
- **Input Validation**: Comprehensive validation of all parameters

### MQTT Compatibility
The MQTT implementation matches real ESP32 behavior:

```json
{
  "meter_id": 1,
  "timestamp": 1710000000,
  "watt_hours_consumed": 250,
  "units_consumed": 1,
  "signature": "base64_encoded_64_byte_signature",
  "public_key": "base64_encoded_32_byte_public_key",
  "device_id": "ESP32-1",
  "firmware_version": "1.0.0",
  "battery_level": 85,
  "signal_strength": -70,
  "temperature": 25
}
```

## Usage Examples

### Basic Setup
```bash
# Install dependencies
cd meter-simulator && npm install

# Generate device keys
node src/index.js generate-keys

# Register meter
node src/index.js register --keys device-keys.json --user GD5... --provider GAB...

# Start simulation
node src/index.js simulate --config meter-config.json
```

### MQTT Integration
```bash
# Configure MQTT in .env
MQTT_HOST=localhost
MQTT_PORT=1883

# Start simulation with MQTT
node src/index.js simulate --config meter-config.json --mqtt
```

### Development Testing
```bash
# Run tests
npm test

# Validate structure
node test-validation.js

# Run example
node examples/basic-usage.js
```

## Dependencies

### Core Dependencies
- `commander` - CLI framework
- `chalk` - Terminal colors and formatting
- `inquirer` - Interactive prompts
- `tweetnacl` - Ed25519 cryptography
- `stellar-sdk` - Stellar/Soroban integration
- `mqtt` - MQTT client
- `axios` - HTTP requests
- `bs58` - Base58 encoding for keys

### Development Dependencies
- `jest` - Testing framework
- `eslint` - Code linting

## Testing

### Unit Tests
- Device simulation logic
- Usage generation algorithms
- Peak hour detection
- Signature generation
- Input validation

### Integration Tests
- Contract interface simulation
- MQTT publishing
- CLI command execution
- Configuration loading

### Manual Testing
- End-to-end simulation workflows
- MQTT broker integration
- Contract submission validation

## Configuration

The simulator uses environment variables for configuration:

```env
# Stellar Network
STELLAR_NETWORK=testnet
CONTRACT_ID=CB7PSJZALNWNX7NLOAM6LOEL4OJZMFPQZJMIYO522ZSACYWXTZIDEDSS

# MQTT Settings
MQTT_HOST=localhost
MQTT_PORT=1883

# Simulation Settings
DEFAULT_INTERVAL=30
BASE_WATT_HOURS=100
```

## Benefits for Development

1. **Local Testing**: Test contract interactions without real hardware
2. **Realistic Data**: Generate authentic usage patterns with peak/off-peak variations
3. **Security Validation**: Verify signature verification and authentication
4. **Performance Testing**: Load test contracts with simulated device traffic
5. **Protocol Validation**: Test MQTT integration and message formats
6. **Debugging**: Detailed logging and error reporting

## Future Enhancements

- [ ] Web dashboard for monitoring multiple simulated meters
- [ ] Configurable consumption profiles (residential, commercial, industrial)
- [ ] Historical data replay from real meter readings
- [ ] Integration with time-series databases
- [ ] Automated test scenarios and benchmarks
- [ ] Docker containerization for easy deployment

## Security Considerations

- Private keys are generated and stored locally
- All usage data is cryptographically signed before transmission
- Timestamp validation prevents replay attacks
- Maximum usage limits prevent abuse
- Input validation prevents malformed data

## Compatibility

- **Node.js**: 16.0.0+
- **Operating Systems**: Windows, macOS, Linux
- **MQTT Brokers**: Mosquitto, EMQX, HiveMQ
- **Stellar Networks**: Testnet (default), Mainnet (configurable)

## Documentation

Comprehensive documentation is provided in:
- `README.md` - Full usage guide and API reference
- `examples/basic-usage.js` - Code examples and demonstrations
- Inline code documentation throughout all source files

---

This meter simulator CLI tool provides a complete development environment for testing the Utility Drip smart contracts with realistic IoT device behavior, enabling faster development cycles and more robust testing without requiring physical hardware.
