# Stream Insurance Pool Governance System

## Overview

The Stream Insurance Pool Governance system implements a decentralized "Community Insurance" mechanism that provides mutual aid for utility security. Users can opt into a shared insurance pool by paying premiums, and the pool automatically lends funds to members whose utility streams are about to fail due to missed deposits.

## Key Features

### 1. Community Mutual Aid
- **Pooled Safety Buffer**: Multiple users contribute to a shared insurance fund
- **Auto-Lending**: Automatic emergency funding when member streams are at risk
- **Risk Sharing**: Distributes individual risk across the community
- **Decentralized Governance**: Pool participants vote on key parameters

### 2. Risk-Based Premium Calculation
- **Dynamic Pricing**: Premiums calculated based on individual risk assessment
- **Multi-Factor Risk Scoring**: Considers payment history, usage patterns, device security, and tenure
- **Fair Pricing**: Lower-risk users pay lower premiums, higher-risk users pay more
- **Transparent Scoring**: Risk factors are clearly defined and auditable

### 3. Governance Mechanisms
- **Proposal System**: Members can propose changes to pool parameters
- **Voting Power**: Based on premium contributions and tenure in the pool
- **Quorum Requirements**: 20% of voting power must participate for valid decisions
- **Approval Threshold**: 51% approval required for proposal execution
- **Timelock**: 7-day voting period ensures deliberate decision-making

## Architecture

### Core Components

#### InsurancePool
```rust
pub struct InsurancePool {
    pub total_funds: i128,              // Total pool balance
    pub total_members: u32,             // Number of active members
    pub total_voting_power: i128,       // Sum of all member voting power
    pub created_at: u64,                // Pool creation timestamp
    pub governance_admin: Address,       // Initial admin (can be changed via governance)
    pub base_premium_rate_bps: i128,    // Base premium rate (basis points)
    pub risk_multiplier_max: i128,      // Maximum risk multiplier
    pub is_active: bool,                // Pool operational status
    pub emergency_pause: bool,          // Emergency pause flag
}
```

#### InsurancePoolMember
```rust
pub struct InsurancePoolMember {
    pub user: Address,                  // Member's address
    pub premium_paid: i128,             // Total premium contributed
    pub join_timestamp: u64,            // When member joined
    pub last_claim_timestamp: u64,      // Last claim submission time
    pub claim_count: u32,               // Number of claims made
    pub risk_score: u32,                // Current risk score (0-1000)
    pub voting_power: i128,             // Calculated voting power
    pub is_active: bool,                // Member status
}
```

#### GovernanceProposal
```rust
pub struct GovernanceProposal {
    pub proposal_id: u64,               // Unique proposal identifier
    pub proposer: Address,              // Who created the proposal
    pub proposal_type: ProposalType,    // Type of change proposed
    pub description: Symbol,            // Brief description
    pub new_value: i128,                // Proposed new value
    pub created_at: u64,                // Creation timestamp
    pub voting_deadline: u64,           // When voting ends
    pub votes_for: i128,                // Voting power supporting
    pub votes_against: i128,            // Voting power opposing
    pub total_votes: i128,              // Total voting power participated
    pub is_executed: bool,              // Whether proposal was executed
    pub is_cancelled: bool,             // Whether proposal was cancelled
}
```

### Risk Assessment System

The system evaluates member risk across four dimensions:

1. **Payment History Score (0-250 points)**
   - PrePaid: Balance maintenance patterns
   - PostPaid: Debt-to-collateral ratios
   - Consistent positive balances = higher score

2. **Usage Stability Score (0-250 points)**
   - Peak usage vs. average usage ratios
   - Stable consumption patterns = higher score
   - High volatility = lower score

3. **Device Security Score (0-250 points)**
   - Device pairing status
   - Heartbeat frequency and recency
   - Proper cryptographic setup = higher score

4. **Tenure Score (0-250 points)**
   - Length of membership in pool
   - Account age and history
   - Longer tenure = higher score

**Total Risk Score**: Sum of all dimensions (0-1000)
- Lower scores indicate lower risk
- Used to calculate premium multipliers (0.5x - 3.0x)

### Premium Calculation

```
Base Premium = Monthly Usage Value × Base Premium Rate (BPS)
Risk Multiplier = 0.5 + (Risk Score / 1000) × 2.5
Final Premium = Base Premium × Risk Multiplier
```

Constraints:
- Minimum Premium: 100 XLM
- Maximum Premium: 10,000 XLM
- Base Rate Range: 0.1% - 10% of monthly usage

### Claim Processing

#### Automatic Approval
Small claims are automatically approved and processed if:
- Claim amount ≤ 1% of total pool funds
- Member risk score ≤ 300 (low risk)
- Member is in good standing

#### Manual Review Process
Larger claims require governance approval:
1. Member submits claim with reason
2. Community reviews claim details
3. Voting period for approval/rejection
4. If approved, funds are transferred

#### Claim Limits
- Maximum claim: 10% of total pool funds
- Cooldown period: 30 days between claims
- Emergency override: Governance can approve exceptions

### Governance Proposal Types

1. **ChangePremiumRate**: Adjust base premium percentage
2. **ChangeRiskMultiplier**: Modify maximum risk multiplier
3. **ChangeMaxClaimAmount**: Adjust maximum claim limits
4. **AddMember**: Approve new member applications
5. **RemoveMember**: Remove problematic members
6. **EmergencyPause**: Pause pool operations
7. **ChangeGovernanceAdmin**: Transfer admin rights

### Integration with Utility Contracts

#### Fee Allocation
- 0.5% of every utility claim is allocated to the insurance pool
- Provides sustainable funding for the pool
- Creates alignment between utility usage and insurance funding

#### Emergency Funding
When a member's utility stream is at risk:
1. System detects low balance or payment failure
2. If member is in insurance pool, automatic claim is triggered
3. Funds are transferred to member's meter balance
4. Member's claim history is updated

#### Throttling Integration
- Insurance pool members get priority during network throttling
- Pool membership considered in priority calculations
- Provides additional utility security benefit

## Usage Examples

### Creating an Insurance Pool

```rust
// Admin creates the pool with 1% base premium rate
UtilityContract::create_insurance_pool(
    env,
    admin_address,
    100, // 1% in basis points
)?;
```

### Joining the Pool

```rust
// Calculate required premium for user's meter
let premium = UtilityContract::calculate_premium_amount(
    env,
    user_address,
    meter_id,
)?;

// Join the pool
UtilityContract::join_insurance_pool(
    env,
    user_address,
    meter_id,
    premium,
)?;
```

### Submitting a Claim

```rust
// Submit emergency funding claim
let claim_id = UtilityContract::submit_insurance_claim(
    env,
    claimant_address,
    meter_id,
    requested_amount,
    symbol_short!("EmergFund"),
)?;
```

### Creating Governance Proposals

```rust
// Propose to change premium rate to 1.5%
let proposal_id = UtilityContract::create_governance_proposal(
    env,
    proposer_address,
    ProposalType::ChangePremiumRate,
    symbol_short!("NewRate"),
    150, // 1.5% in basis points
)?;
```

### Voting on Proposals

```rust
// Vote in favor of the proposal
UtilityContract::vote_on_proposal(
    env,
    voter_address,
    proposal_id,
    true, // vote for
)?;
```

## Security Considerations

### Access Control
- Only pool members can vote on proposals
- Minimum voting power required to create proposals (5% of total)
- Cooldown periods prevent spam claims
- Emergency pause mechanism for crisis situations

### Economic Security
- Risk-based pricing prevents adverse selection
- Claim limits prevent pool drainage
- Diversified risk across multiple members
- Sustainable funding through utility fee allocation

### Governance Security
- Quorum requirements prevent minority control
- Voting power based on stake and tenure
- Timelock periods allow for deliberation
- Transparent proposal and voting process

## Benefits

### For Individual Users
- **Utility Security**: Protection against service interruption
- **Lower Individual Risk**: Shared risk across community
- **Governance Participation**: Voice in pool management
- **Priority Access**: Benefits during network congestion

### For the Ecosystem
- **Network Stability**: Reduced service interruptions
- **Community Building**: Shared incentives and cooperation
- **Sustainable Funding**: Self-funding through utility fees
- **Decentralized Governance**: Community-controlled parameters

### For Utility Providers
- **Reduced Defaults**: Insurance covers payment gaps
- **Stable Revenue**: More predictable payment flows
- **Customer Retention**: Enhanced service reliability
- **Risk Mitigation**: Shared responsibility for customer defaults

## Future Enhancements

### Advanced Risk Models
- Machine learning-based risk assessment
- Integration with external credit scoring
- Dynamic risk adjustment based on market conditions
- Predictive analytics for claim probability

### Cross-Pool Insurance
- Multiple specialized pools (residential, commercial, industrial)
- Inter-pool reinsurance mechanisms
- Risk transfer between pools
- Specialized coverage types

### Integration Expansions
- Integration with DeFi lending protocols
- Automated market makers for premium pricing
- Tokenized insurance positions
- Cross-chain insurance coverage

## Conclusion

The Stream Insurance Pool Governance system creates a robust, community-driven mutual aid mechanism that enhances utility security while maintaining decentralized governance. By combining risk-based pricing, democratic decision-making, and automatic emergency funding, it provides a sustainable solution for utility payment security that benefits all participants in the ecosystem.