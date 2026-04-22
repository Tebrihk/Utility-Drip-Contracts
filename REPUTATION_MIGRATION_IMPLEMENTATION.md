# Inter-Susu Reputation Migration Implementation

## Issue #127: Support for Inter-Susu_Reputation_Migration_for_Renters

### Overview

This implementation provides a "Reputation Migration" function that allows users to export their "Utility Reliability Score" from an old contract instance to a new one when moving to a new city. The old contract "Burns" the old record, and the new one "Mints" it, enabling "Portable Trust" - allowing a person's good character to be their most valuable, global, and mobile asset.

### Architecture

The implementation consists of three main components:

1. **ReputationRecord** - Stores user reputation data
2. **ReputationMigration** - Tracks migration history
3. **Migration Functions** - Export/Import functionality

### Data Structures

#### ReputationRecord
```rust
pub struct ReputationRecord {
    pub user: Address,
    pub reliability_score: u32,        // 0-100 score
    pub total_payments: u32,            // Total number of payments
    pub on_time_payments: u32,          // On-time payment count
    pub total_usage: i128,              // Total utility usage
    pub created_at: u64,               // Creation timestamp
    pub last_updated: u64,             // Last update timestamp
    pub is_active: bool,               // Active status (burned = false)
}
```

#### ReputationMigration
```rust
pub struct ReputationMigration {
    pub old_contract: Address,
    pub new_contract: Address,
    pub user: Address,
    pub reputation_record: ReputationRecord,
    pub migration_timestamp: u64,
    pub nullifier: BytesN<32>,         // ZK compatibility
}
```

### Core Functions

#### 1. Export Reputation (Burn)
```rust
pub fn export_reputation(env: Env, user: Address) -> ReputationRecord
```

- **Purpose**: Export and burn reputation from old contract
- **Authorization**: Requires user authentication
- **Process**:
  1. Retrieves user's reputation record
  2. Marks record as inactive (burned)
  3. Emits RepExport event
  4. Returns the reputation data for migration

#### 2. Import Reputation (Mint)
```rust
pub fn import_reputation(
    env: Env,
    old_contract: Address,
    user: Address,
    reputation_record: ReputationRecord,
    migration_signature: BytesN<64>,
    nullifier: BytesN<32>,
)
```

- **Purpose**: Import and mint reputation in new contract
- **Authorization**: Requires user authentication
- **Security Features**:
  - Prevents double migration via nullifier tracking
  - Validates migration hasn't occurred before
  - Stores migration history
- **Process**:
  1. Validates migration prerequisites
  2. Creates migration record
  3. Activates reputation in new contract
  4. Emits RepImport event

#### 3. Get Reputation
```rust
pub fn get_reputation(env: Env, user: Address) -> ReputationRecord
```

- **Purpose**: Retrieve user's current reputation
- **Returns**: Active reputation record

#### 4. Update Reputation Score
```rust
pub fn update_reputation_score(env: Env, user: Address, payment_amount: i128, on_time: bool)
```

- **Purpose**: Update reputation based on payment history
- **Features**:
  - Creates new record if none exists
  - Calculates weighted average for reliability score
  - Tracks payment timeliness and usage

### Security Features

#### 1. Double Migration Prevention
- Uses nullifiers to prevent the same reputation from being migrated multiple times
- Tracks migrated reputation pairs (user, old_contract)

#### 2. Signature Verification
- Placeholder for migration signature verification
- In production, would verify cryptographic signature from old contract

#### 3. Authorization
- All functions require proper user authentication
- Only contract owner can update reputation scores

### Storage Keys

```rust
UserReputation(Address)              // User's reputation record
ReputationMigration(BytesN<32>)     // Migration record (by nullifier)
MigratedReputation(Address, Address) // Migration flag (user, old_contract)
NullifierMap(BytesN<32>)            // Used nullifier tracking
```

### Events

#### RepExport
- **Topics**: (symbol_short!("RepExport"), user)
- **Data**: (reliability_score, timestamp)

#### RepImport
- **Topics**: (symbol_short!("RepImport"), user)
- **Data**: (reliability_score, old_contract)

### Test Coverage

Comprehensive test suite includes:

1. **Export Tests**:
   - Verifies reputation export burns old record
   - Validates event emission
   - Tests error cases (no reputation found)

2. **Import Tests**:
   - Verifies reputation import mints new record
   - Tests migration record storage
   - Validates nullifier prevention

3. **Integration Tests**:
   - Complete migration flow
   - Reputation updates after migration
   - Error scenarios

### Usage Flow

1. **User moves to new city**
2. **Export from old contract**:
   ```
   old_contract.export_reputation(user)
   ```
3. **Import to new contract**:
   ```
   new_contract.import_reputation(
       old_contract_address,
       user,
       exported_reputation,
       migration_signature,
       unique_nullifier
   )
   ```
4. **Continue using reputation**:
   ```
   new_contract.update_reputation_score(user, amount, on_time)
   ```

### Benefits

1. **Portable Trust**: Users maintain their reputation across locations
2. **SocialFi Integration**: Reputation becomes a valuable, mobile asset
3. **Migration Security**: Prevents double-spending and fraud
4. **ZK Compatibility**: Nullifier support for privacy features
5. **Seamless Experience**: Users don't lose trust when moving

### Future Enhancements

1. **Cross-Chain Migration**: Support for different blockchain networks
2. **Reputation Markets**: Allow reputation trading (with controls)
3. **Advanced Scoring**: Incorporate more factors into reliability score
4. **Privacy Features**: Full ZK proof implementation
5. **Governance**: Community-based reputation validation

### Implementation Status

- [x] Core data structures
- [x] Export/Burn functionality
- [x] Import/Mint functionality
- [x] Security measures (nullifiers, double migration prevention)
- [x] Comprehensive test suite
- [x] Event emission
- [x] Documentation

This implementation successfully addresses Issue #127 and provides the foundation for "Portable Trust" in the Utility Drip ecosystem.
