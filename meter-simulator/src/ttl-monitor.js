#!/usr/bin/env node

/**
 * TTL Monitor and Auto-Bumper for Soroban Contract
 *
 * This script monitors the Time-To-Live (TTL) of the contract's ledger entries
 * and automatically extends them when they fall below a threshold.
 *
 * Usage:
 *   node src/ttl-monitor.js [options]
 *
 * Options:
 *   --check-only    Only check TTL without bumping
 *   --interval <ms> Check interval in milliseconds (default: 300000 = 5 minutes)
 *   --threshold <n> TTL threshold in ledgers (default: 1000)
 *   --extend-to <n> Extend TTL to this many ledgers (default: 10000)
 */

const { Server, Networks, TransactionBuilder, Operation, Keypair, SorobanRpc } = require('stellar-sdk');
const config = require('./config');
const fs = require('fs');

class TTLMonitor {
  constructor(options = {}) {
    this.network = config.contract.network === 'mainnet' ? Networks.PUBLIC : Networks.TESTNET;
    this.rpcUrl = config.contract.rpcUrl;
    this.contractId = config.contract.contractId;
    this.server = new SorobanRpc.Server(this.rpcUrl);
    this.horizon = new Server(config.contract.horizonUrl);

    // Service account for bumping
    const serviceSecret = process.env.SERVICE_ACCOUNT_SECRET;
    if (!serviceSecret) {
      throw new Error('SERVICE_ACCOUNT_SECRET environment variable is required');
    }
    this.serviceKeypair = Keypair.fromSecret(serviceSecret);

    this.checkOnly = options.checkOnly || false;
    this.interval = options.interval || 300000; // 5 minutes
    this.threshold = options.threshold || 1000; // ledgers
    this.extendTo = options.extendTo || 10000; // ledgers

    this.isRunning = false;
  }

  async start() {
    console.log('🚀 Starting TTL Monitor...');
    console.log(`📊 Contract ID: ${this.contractId}`);
    console.log(`🌐 Network: ${this.network}`);
    console.log(`⏱️  Check Interval: ${this.interval / 1000} seconds`);
    console.log(`🎯 TTL Threshold: ${this.threshold} ledgers`);
    console.log(`🔄 Extend To: ${this.extendTo} ledgers`);

    this.isRunning = true;

    // Initial check
    await this.checkAndBump();

    // Schedule periodic checks
    if (!this.checkOnly) {
      setInterval(async () => {
        if (this.isRunning) {
          await this.checkAndBump();
        }
      }, this.interval);
    }
  }

  stop() {
    console.log('🛑 Stopping TTL Monitor...');
    this.isRunning = false;
  }

  async checkAndBump() {
    try {
      console.log('🔍 Checking contract TTL...');

      // Get current ledger
      const latestLedger = await this.server.getLatestLedger();
      const currentLedger = latestLedger.sequence;

      console.log(`📈 Current Ledger: ${currentLedger}`);

      // Get contract instance ledger entry
      const contractInstanceKey = this.getContractInstanceKey();
      const ledgerEntries = await this.server.getLedgerEntries([contractInstanceKey]);

      if (ledgerEntries.entries.length === 0) {
        console.log('⚠️  Contract instance not found');
        return;
      }

      const entry = ledgerEntries.entries[0];
      const lastModified = entry.lastModifiedLedgerSeq;
      const ttl = entry.ttl;

      console.log(`📅 Last Modified: ${lastModified}`);
      console.log(`⏳ TTL: ${ttl} ledgers`);

      // Calculate remaining TTL
      const remainingTTL = lastModified + ttl - currentLedger;
      console.log(`⏰ Remaining TTL: ${remainingTTL} ledgers`);

      if (remainingTTL < this.threshold) {
        console.log(`⚠️  TTL below threshold (${this.threshold}), extending...`);

        if (!this.checkOnly) {
          await this.bumpTTL([contractInstanceKey], this.extendTo);
          console.log('✅ TTL extended successfully');
        } else {
          console.log('🔍 Check-only mode: would extend TTL');
        }
      } else {
        console.log(`✅ TTL is healthy (${remainingTTL} > ${this.threshold})`);
      }

    } catch (error) {
      console.error('❌ Error checking TTL:', error.message);
    }
  }

  getContractInstanceKey() {
    const { xdr } = require('stellar-sdk');

    return xdr.LedgerKey.contractData(new xdr.LedgerKeyContractData({
      contract: new xdr.ScAddress(xdr.ScAddressType.scAddressTypeContract(this.contractId)),
      key: xdr.ScVal.scvContractInstance(),
      durability: xdr.ContractDataDurability.persistent()
    }));
  }

  async bumpTTL(ledgerKeys, extendTo) {
    try {
      // Load service account
      const account = await this.horizon.loadAccount(this.serviceKeypair.publicKey());

      // Create transaction with ExtendFootprintTTLOp
      const transaction = new TransactionBuilder(account, {
        fee: '1000', // Higher fee for Soroban ops
        networkPassphrase: this.network
      })
        .addOperation(Operation.extendFootprintTtl({
          extendTo: extendTo
        }))
        .setTimeout(30)
        .build();

      // Set the footprint
      const footprint = new SorobanRpc.AssembledTransactionFootprint(ledgerKeys, []);
      transaction.setSorobanData(new SorobanRpc.SorobanTransactionData(footprint, 0));

      // Sign and submit
      transaction.sign(this.serviceKeypair);

      const result = await this.server.sendTransaction(transaction);
      console.log(`📤 Transaction submitted: ${result.hash}`);

      // Wait for confirmation
      let status = result.status;
      let attempts = 0;
      while (status === 'PENDING' && attempts < 10) {
        await new Promise(resolve => setTimeout(resolve, 1000));
        const response = await this.server.getTransaction(result.hash);
        status = response.status;
        attempts++;
      }

      if (status === 'SUCCESS') {
        console.log('✅ Transaction confirmed');
      } else {
        console.log(`❌ Transaction failed: ${status}`);
      }

    } catch (error) {
      throw new Error(`Failed to bump TTL: ${error.message}`);
    }
  }
}

// CLI interface
if (require.main === module) {
  const args = process.argv.slice(2);
  const options = {};

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--check-only':
        options.checkOnly = true;
        break;
      case '--interval':
        options.interval = parseInt(args[i + 1]);
        i++;
        break;
      case '--threshold':
        options.threshold = parseInt(args[i + 1]);
        i++;
        break;
      case '--extend-to':
        options.extendTo = parseInt(args[i + 1]);
        i++;
        break;
      default:
        console.log('Unknown option:', args[i]);
        process.exit(1);
    }
  }

  const monitor = new TTLMonitor(options);

  // Handle graceful shutdown
  process.on('SIGINT', () => {
    monitor.stop();
    process.exit(0);
  });

  monitor.start().catch(error => {
    console.error('Failed to start TTL monitor:', error.message);
    process.exit(1);
  });
}

module.exports = TTLMonitor;