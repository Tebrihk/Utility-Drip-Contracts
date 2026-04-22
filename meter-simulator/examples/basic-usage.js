#!/usr/bin/env node

/**
 * Basic Usage Example for Meter Simulator
 * This example demonstrates the core functionality of the meter simulator
 */

const MeterDevice = require('../src/meter-device');
const ContractInterface = require('../src/contract-interface');
const MQTTPublisher = require('../src/mqtt-publisher');
const config = require('../src/config');

async function basicUsageExample() {
  console.log('🚀 Meter Simulator - Basic Usage Example\n');

  try {
    // 1. Create a mock meter configuration
    const meterConfig = {
      meter_id: 12345,
      keys: {
        private_key: '5KQwrPbwdL6PhXujxW37FSSQZ1JiwsST4cqprzPeq2XqKdnNqJ',
        public_key: 'GB7BDSQJY4Q3B2D2K3Y2N2X2J2M2L2K2J2H2G2F2E2D2C2B2A2Z2Y2X2W2V2U',
        public_key_base64: 'RGJCUkRTUUxZNFEzQjJEMkszWTJOMlgySjJNSkwySzJjcXByelBlcTJYcUtkbk5xSg=='
      },
      off_peak_rate: 10
    };

    // 2. Create meter device instance
    console.log('📱 Creating meter device...');
    const device = new MeterDevice(meterConfig);
    console.log(`✅ Meter device created with ID: ${device.meterId}`);

    // 3. Generate usage data in different modes
    console.log('\n📊 Generating usage data in different modes:');
    
    const realisticData = device.generateUsageData('realistic');
    console.log(`🔸 Realistic: ${realisticData.display_watt_hours}Wh, ${realisticData.units_consumed} units`);
    console.log(`   Peak Hour: ${realisticData.is_peak_hour ? 'Yes' : 'No'}`);
    console.log(`   Rate: ${realisticData.effective_rate} tokens/sec`);

    const surgeData = device.generateUsageData('surge');
    console.log(`🔸 Surge: ${surgeData.display_watt_hours}Wh, ${surgeData.units_consumed} units`);

    const lowData = device.generateUsageData('low');
    console.log(`🔸 Low: ${lowData.display_watt_hours}Wh, ${lowData.units_consumed} units`);

    // 4. Create contract interface
    console.log('\n🔗 Creating contract interface...');
    const contract = new ContractInterface(config.contract);
    console.log('✅ Contract interface created');

    // 5. Submit usage data (simulated)
    console.log('\n📤 Submitting usage data to contract...');
    const result = await contract.submitUsageData(realisticData);
    console.log(`✅ Usage data submitted: ${JSON.stringify(result, null, 2)}`);

    // 6. Get meter status
    console.log('\n📊 Getting meter status...');
    const meter = await contract.getMeter(meterConfig.meter_id);
    console.log(`✅ Meter status: Active=${meter.is_active}, Balance=${meter.balance}`);

    // 7. Demonstrate MQTT publishing (optional)
    console.log('\n📡 Demonstrating MQTT publishing...');
    try {
      const mqtt = new MQTTPublisher(config.mqtt);
      await mqtt.connect();
      await mqtt.publishUsageData(realisticData);
      await mqtt.disconnect();
      console.log('✅ MQTT demonstration completed');
    } catch (mqttError) {
      console.log('⚠️  MQTT demonstration skipped (broker not available)');
    }

    // 8. Show device statistics
    console.log('\n📈 Device Statistics:');
    const stats = device.getStats();
    console.log(`🔸 Total Usage: ${stats.totalWattHours}Wh`);
    console.log(`🔸 Current Cycle: ${stats.currentCycleWattHours}Wh`);
    console.log(`🔸 Last Reading: ${stats.lastReadingTime}`);

    console.log('\n✅ Basic usage example completed successfully!');

  } catch (error) {
    console.error('❌ Error in basic usage example:', error.message);
    process.exit(1);
  }
}

// Run the example
if (require.main === module) {
  basicUsageExample();
}

module.exports = basicUsageExample;
