use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address, Env,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SBTError {
    AlreadyInitialized = 1,
    NotAuthorized = 2,
    AlreadyMinted = 3,
}

#[contracttype]
pub enum SBTDataKey {
    Admin,
    SBTRecord(Address), // Maps user address -> SBTMetadata
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SBTMetadata {
    pub carbon_saved: i128,
    pub reliability_score: u32,
    pub issue_date: u64,
}

#[contract]
pub struct ImpactSBTMinter;

#[contractimpl]
impl ImpactSBTMinter {
    /// Initialize the SBT contract with the authorized minter (the main Utility Contract)
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&SBTDataKey::Admin) {
            panic_with_error!(&env, SBTError::AlreadyInitialized);
        }
        env.storage().instance().set(&SBTDataKey::Admin, &admin);
    }

    /// Mint the Soulbound Token (On-Chain Green CV).
    /// Note: No transfer functions exist in this contract, making it strictly Soulbound.
    pub fn mint_impact_sbt(
        env: Env,
        to: Address,
        carbon_saved: i128,
        reliability_score: u32,
    ) {
        // Require authorization from the Admin (Utility Contract)
        let admin: Address = env.storage().instance().get(&SBTDataKey::Admin).unwrap();
        admin.require_auth();

        // Check if already minted (Soulbound credentials are 1 per address in this context)
        if env.storage().persistent().has(&SBTDataKey::SBTRecord(to.clone())) {
            panic_with_error!(&env, SBTError::AlreadyMinted);
        }

        let metadata = SBTMetadata {
            carbon_saved,
            reliability_score,
            issue_date: env.ledger().timestamp(),
        };

        // Store persistently so the CV lives forever on the ledger
        env.storage().persistent().set(&SBTDataKey::SBTRecord(to.clone()), &metadata);

        env.events().publish(
            (symbol_short!("MintSBT"), to),
            (carbon_saved, reliability_score),
        );
    }

    /// View function to fetch a user's On-Chain Green CV
    pub fn get_sbt(env: Env, user: Address) -> Option<SBTMetadata> {
        env.storage().persistent().get(&SBTDataKey::SBTRecord(user))
    }
}
