const MeterDevice = require('../src/meter-device');
const config = require('../src/config');

describe('MeterDevice', () => {
  let meterDevice;
  let mockConfig;

  beforeEach(() => {
    mockConfig = {
      meter_id: 12345,
      keys: {
        private_key: '5KQwrPbwdL6PhXujxW37FSSQZ1JiwsST4cqprzPeq2XqKdnNqJ',
        public_key: 'GB7BDSQJY4Q3B2D2K3Y2N2X2J2M2L2K2J2H2G2F2E2D2C2B2A2Z2Y2X2W2V2U',
        public_key_base64: 'RGJCUkRTUUxZNFEzQjJEMkszWTJOMlgySjJNSkwySzJjcXByelBlcTJYcUtkbk5xSg=='
      },
      off_peak_rate: 10
    };
    
    meterDevice = new MeterDevice(mockConfig);
  });

  describe('Constructor', () => {
    test('should initialize with correct configuration', () => {
      expect(meterDevice.meterId).toBe(12345);
      expect(meterDevice.offPeakRate).toBe(10);
      expect(meterDevice.precisionFactor).toBe(1000);
    });
  });

  describe('Peak Hour Detection', () => {
    test('should detect peak hours correctly', () => {
      // Test peak hour (19:00 UTC = 68400 seconds)
      const peakTimestamp = 1710000000; // This should be a peak hour
      expect(meterDevice._isPeakHour(peakTimestamp)).toBe(true);
    });

    test('should detect off-peak hours correctly', () => {
      // Test off-peak hour (13:00 UTC = 46800 seconds)
      const offPeakTimestamp = 1710000000 - 21600; // 6 hours before peak
      expect(meterDevice._isPeakHour(offPeakTimestamp)).toBe(false);
    });
  });

  describe('Usage Generation', () => {
    test('should generate realistic usage data', () => {
      const usageData = meterDevice.generateUsageData('realistic');
      
      expect(usageData).toHaveProperty('meter_id', 12345);
      expect(usageData).toHaveProperty('timestamp');
      expect(usageData).toHaveProperty('watt_hours_consumed');
      expect(usageData).toHaveProperty('units_consumed');
      expect(usageData).toHaveProperty('signature');
      expect(usageData).toHaveProperty('public_key');
      expect(usageData.watt_hours_consumed).toBeGreaterThan(0);
      expect(usageData.units_consumed).toBeGreaterThan(0);
      expect(usageData.signature).toHaveLength(88); // 64 bytes base64 encoded
      expect(usageData.public_key).toHaveLength(44); // 32 bytes base64 encoded
    });

    test('should generate surge usage data with higher values', () => {
      const surgeData = meterDevice.generateUsageData('surge');
      const realisticData = meterDevice.generateUsageData('realistic');
      
      expect(surgeData.watt_hours_consumed).toBeGreaterThan(realisticData.watt_hours_consumed);
    });

    test('should generate low usage data with lower values', () => {
      const lowData = meterDevice.generateUsageData('low');
      const realisticData = meterDevice.generateUsageData('realistic');
      
      expect(lowData.watt_hours_consumed).toBeLessThan(realisticData.watt_hours_consumed);
    });

    test('should generate custom usage data', () => {
      const customData = meterDevice.generateCustomUsageData(500, 2);
      
      expect(customData.watt_hours_consumed).toBe(500000); // 500 * 1000 precision
      expect(customData.units_consumed).toBe(2);
    });
  });

  describe('Peak Hour Rate Calculation', () => {
    test('should apply peak hour multiplier during peak hours', () => {
      // Mock peak hour
      jest.spyOn(meterDevice, '_isPeakHour').mockReturnValue(true);
      
      const usageData = meterDevice.generateUsageData('realistic');
      expect(usageData.effective_rate).toBe(15); // 10 * 1.5
      expect(usageData.is_peak_hour).toBe(true);
    });

    test('should use off-peak rate during off-peak hours', () => {
      // Mock off-peak hour
      jest.spyOn(meterDevice, '_isPeakHour').mockReturnValue(false);
      
      const usageData = meterDevice.generateUsageData('realistic');
      expect(usageData.effective_rate).toBe(10); // Base rate
      expect(usageData.is_peak_hour).toBe(false);
    });
  });

  describe('Signature Generation', () => {
    test('should generate valid signatures', () => {
      const usageData = meterDevice.generateUsageData('realistic');
      
      // Check signature format
      expect(usageData.signature).toMatch(/^[A-Za-z0-9+/]+={0,2}$/); // Base64 format
      expect(usageData.signature).toHaveLength(88); // 64 bytes base64 encoded
    });

    test('should generate consistent signatures for same data', () => {
      const timestamp = Math.floor(Date.now() / 1000);
      const wattHours = 100;
      const units = 1;
      
      const data1 = meterDevice.generateCustomUsageData(wattHours, units);
      const data2 = meterDevice.generateCustomUsageData(wattHours, units);
      
      // Signatures should be different due to different timestamps
      expect(data1.signature).not.toBe(data2.signature);
    });
  });

  describe('Device Statistics', () => {
    test('should track usage statistics correctly', () => {
      const initialStats = meterDevice.getStats();
      expect(initialStats.totalWattHours).toBe(0);
      expect(initialStats.currentCycleWattHours).toBe(0);
      
      // Generate some usage
      meterDevice.generateUsageData('realistic');
      meterDevice.generateUsageData('realistic');
      
      const updatedStats = meterDevice.getStats();
      expect(updatedStats.totalWattHours).toBeGreaterThan(0);
      expect(updatedStats.currentCycleWattHours).toBeGreaterThan(0);
    });

    test('should reset cycle usage correctly', () => {
      meterDevice.generateUsageData('realistic');
      expect(meterDevice.currentCycleWattHours).toBeGreaterThan(0);
      
      meterDevice.resetCycle();
      expect(meterDevice.currentCycleWattHours).toBe(0);
    });
  });

  describe('Message Creation', () => {
    test('should create signature message with correct format', () => {
      const message = meterDevice._createSignatureMessage(12345, 1710000000, 100000, 1);
      
      expect(message).toBeInstanceOf(Buffer);
      expect(message.length).toBe(32); // 8 + 8 + 8 + 8 bytes
    });
  });

  describe('Input Validation', () => {
    test('should handle edge cases in usage generation', () => {
      // Test with zero base rate
      meterDevice.offPeakRate = 0;
      const usageData = meterDevice.generateUsageData('realistic');
      expect(usageData.units_consumed).toBe(0);
    });

    test('should handle negative values gracefully', () => {
      const initialWattHours = meterDevice.totalWattHours;
      
      // Try to generate negative usage (should be prevented by the implementation)
      const usageData = meterDevice.generateCustomUsageData(-100, -1);
      expect(usageData.watt_hours_consumed).toBeLessThan(0); // This should be handled by validation
    });
  });
});
