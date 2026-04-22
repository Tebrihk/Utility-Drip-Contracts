const crypto = require('crypto');
const nacl = require('tweetnacl');
const bs58 = require('bs58');
const config = require('./config');

class MeterDevice {
  constructor(meterConfig) {
    this.meterId = meterConfig.meter_id;
    this.privateKey = bs58.decode(meterConfig.keys.private_key);
    this.publicKey = bs58.decode(meterConfig.keys.public_key);
    this.offPeakRate = meterConfig.off_peak_rate;
    this.precisionFactor = 1000; // Match contract precision
    
    // Device state
    this.totalWattHours = 0;
    this.currentCycleWattHours = 0;
    this.lastReadingTimestamp = Math.floor(Date.now() / 1000);
  }

  /**
   * Generate realistic usage data based on simulation mode
   */
  generateUsageData(mode = 'realistic') {
    const now = Math.floor(Date.now() / 1000);
    const isPeakHour = this._isPeakHour(now);
    
    let wattHours, units;
    
    switch (mode) {
      case 'surge':
        wattHours = this._generateSurgeUsage(isPeakHour);
        break;
      case 'low':
        wattHours = this._generateLowUsage(isPeakHour);
        break;
      case 'realistic':
      default:
        wattHours = this._generateRealisticUsage(isPeakHour);
        break;
    }
    
    // Calculate units based on rate and time
    const timeElapsed = Math.max(1, now - this.lastReadingTimestamp);
    const effectiveRate = isPeakHour ? this.offPeakRate * 1.5 : this.offPeakRate;
    units = Math.ceil((wattHours * effectiveRate) / (3600 * 1000)); // Convert to units
    
    // Update device state
    this.totalWattHours += wattHours;
    this.currentCycleWattHours += wattHours;
    this.lastReadingTimestamp = now;
    
    return this._createSignedUsageData(wattHours, units, now);
  }

  /**
   * Generate custom usage data with specific values
   */
  generateCustomUsageData(wattHours, units) {
    const now = Math.floor(Date.now() / 1000);
    
    // Update device state
    this.totalWattHours += wattHours;
    this.currentCycleWattHours += wattHours;
    this.lastReadingTimestamp = now;
    
    return this._createSignedUsageData(wattHours, units, now);
  }

  /**
   * Check if current time is during peak hours
   */
  _isPeakHour(timestamp) {
    const secondsInDay = timestamp % config.constants.DAY_IN_SECONDS;
    return secondsInDay >= config.constants.PEAK_HOUR_START && 
           secondsInDay < config.constants.PEAK_HOUR_END;
  }

  /**
   * Generate realistic usage patterns
   */
  _generateRealisticUsage(isPeakHour) {
    const base = config.simulation.baseWattHours;
    const variance = config.simulation.variance;
    const peakMultiplier = config.simulation.peakMultiplier;
    
    // Add random variance
    const randomFactor = 1 + (Math.random() - 0.5) * 2 * variance;
    
    // Apply peak hour multiplier
    let wattHours = base * randomFactor;
    if (isPeakHour) {
      wattHours *= peakMultiplier;
    }
    
    // Random surge events
    if (Math.random() < config.simulation.surgeProbability) {
      wattHours *= 2 + Math.random() * 2; // 2x to 4x surge
    }
    
    return Math.round(wattHours);
  }

  /**
   * Generate surge usage (high consumption)
   */
  _generateSurgeUsage(isPeakHour) {
    const base = config.simulation.baseWattHours * 3; // 3x base
    const variance = 0.2; // Less variance in surge mode
    
    const randomFactor = 1 + (Math.random() - 0.5) * 2 * variance;
    let wattHours = base * randomFactor;
    
    if (isPeakHour) {
      wattHours *= 1.5; // Additional peak multiplier
    }
    
    return Math.round(wattHours);
  }

  /**
   * Generate low usage (minimal consumption)
   */
  _generateLowUsage(isPeakHour) {
    const base = config.simulation.baseWattHours * 0.3; // 30% of base
    const variance = 0.4; // More variance at low levels
    
    const randomFactor = 1 + (Math.random() - 0.5) * 2 * variance;
    let wattHours = base * randomFactor;
    
    // Peak hours have less impact on low usage
    if (isPeakHour) {
      wattHours *= 1.2;
    }
    
    return Math.max(10, Math.round(wattHours)); // Minimum 10 Wh
  }

  /**
   * Create signed usage data structure
   */
  _createSignedUsageData(wattHours, units, timestamp) {
    // Apply precision factor (matching contract)
    const preciseWattHours = wattHours * this.precisionFactor;
    
    // Create the message to sign (matching contract's UsageReport structure)
    const message = this._createSignatureMessage(this.meterId, timestamp, preciseWattHours, units);
    
    // Sign the message
    const signature = nacl.sign.detached(message, this.privateKey);
    
    return {
      meter_id: this.meterId,
      timestamp: timestamp,
      watt_hours_consumed: preciseWattHours,
      units_consumed: units,
      signature: Buffer.from(signature).toString('base64'),
      public_key: Buffer.from(this.publicKey).toString('base64'),
      // Additional metadata for debugging
      display_watt_hours: wattHours, // Human-readable value
      is_peak_hour: this._isPeakHour(timestamp),
      effective_rate: this._isPeakHour(timestamp) ? this.offPeakRate * 1.5 : this.offPeakRate
    };
  }

  /**
   * Create message for signature (matching contract's UsageReport XDR format)
   */
  _createSignatureMessage(meterId, timestamp, wattHours, units) {
    // Create a binary representation matching the contract's XDR format
    const buffer = Buffer.alloc(8 + 8 + 16 + 16); // u64 + u64 + i128 + i128
    
    // Write meter_id (u64)
    buffer.writeBigUInt64LE(BigInt(meterId), 0);
    
    // Write timestamp (u64)
    buffer.writeBigUInt64LE(BigInt(timestamp), 8);
    
    // Write watt_hours_consumed (i128)
    buffer.writeBigInt64LE(BigInt(wattHours), 16);
    
    // Write units_consumed (i128)
    buffer.writeBigInt64LE(BigInt(units), 24);
    
    return buffer;
  }

  /**
   * Get device statistics
   */
  getStats() {
    return {
      meterId: this.meterId,
      totalWattHours: this.totalWattHours,
      currentCycleWattHours: this.currentCycleWattHours,
      lastReadingTimestamp: this.lastReadingTimestamp,
      lastReadingTime: new Date(this.lastReadingTimestamp * 1000).toLocaleString(),
      uptime: Math.floor((Date.now() / 1000) - this.lastReadingTimestamp)
    };
  }

  /**
   * Reset current cycle usage
   */
  resetCycle() {
    this.currentCycleWattHours = 0;
  }
}

module.exports = MeterDevice;
