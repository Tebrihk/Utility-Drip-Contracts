use ink::prelude::string::String;
use ink::storage::Mapping;

#[ink::contract]
pub mod oracle_flow {
    use super::*;

    #[ink(storage)]
    pub struct OracleFlow {
        /// Current flow rate in tokens per second
        tokens_per_second: u128,
        /// Hard cap set by user to prevent bill shock
        hard_cap: u128,
        /// Authorized oracle account
        oracle: AccountId,
        /// Owner of the contract (user)
        owner: AccountId,
    }

    impl OracleFlow {
        #[ink(constructor)]
        pub fn new(owner: AccountId, oracle: AccountId, initial_rate: u128, hard_cap: u128) -> Self {
            Self {
                tokens_per_second: initial_rate,
                hard_cap,
                oracle,
                owner,
            }
        }

        /// Oracle updates the flow rate based on market price or consumption
        #[ink(message)]
        pub fn update_rate(&mut self, new_rate: u128) -> Result<(), String> {
            let caller = self.env().caller();
            if caller != self.oracle {
                return Err(String::from("Unauthorized oracle"));
            }

            // Apply hard cap protection
            if new_rate > self.hard_cap {
                self.tokens_per_second = self.hard_cap;
            } else {
                self.tokens_per_second = new_rate;
            }

            Ok(())
        }

        /// User can update their hard cap
        #[ink(message)]
        pub fn set_hard_cap(&mut self, new_cap: u128) -> Result<(), String> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err(String::from("Only owner can set cap"));
            }
            self.hard_cap = new_cap;
            Ok(())
        }

        /// Get current flow rate
        #[ink(message)]
        pub fn get_rate(&self) -> u128 {
            self.tokens_per_second
        }

        /// Get current hard cap
        #[ink(message)]
        pub fn get_cap(&self) -> u128 {
            self.hard_cap
        }
    }
}
