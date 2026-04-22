use ink::prelude::vec::Vec;
use ink::storage::Mapping;

#[derive(scale::Encode, scale::Decode, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct AssetShare {
    pub asset: String,       // Stellar asset code (e.g. USDC, NATIVE)
    pub percentage: u8,      // share of total flow (0–100)
    pub tokens_per_second: u128, // computed pro‑rated flow
}

#[ink::contract]
pub mod basket_stream {
    use super::*;

    #[ink(storage)]
    pub struct BasketStream {
        /// Basket of up to 3 assets
        basket: Vec<AssetShare>,
        /// Owner of the stream
        owner: AccountId,
        /// Total flow rate in tokens per second
        total_rate: u128,
    }

    impl BasketStream {
        #[ink(constructor)]
        pub fn new(owner: AccountId, assets: Vec<(String, u8)>, total_rate: u128) -> Self {
            assert!(assets.len() <= 3, "Max 3 assets allowed");
            let basket = assets
                .into_iter()
                .map(|(asset, pct)| AssetShare {
                    asset,
                    percentage: pct,
                    tokens_per_second: total_rate * pct as u128 / 100,
                })
                .collect();

            Self { basket, owner, total_rate }
        }

        /// Withdraw distributes pro‑rated amounts atomically
        #[ink(message)]
        pub fn withdraw(&self, seconds: u128) -> Vec<(String, u128)> {
            self.basket
                .iter()
                .map(|a| {
                    let amount = a.tokens_per_second * seconds;
                    (a.asset.clone(), amount)
                })
                .collect()
        }

        /// Update basket composition
        #[ink(message)]
        pub fn update_basket(&mut self, assets: Vec<(String, u8)>, total_rate: u128) {
            assert!(self.env().caller() == self.owner, "Only owner can update");
            assert!(assets.len() <= 3, "Max 3 assets allowed");

            self.total_rate = total_rate;
            self.basket = assets
                .into_iter()
                .map(|(asset, pct)| AssetShare {
                    asset,
                    percentage: pct,
                    tokens_per_second: total_rate * pct as u128 / 100,
                })
                .collect();
        }

        /// Get current basket
        #[ink(message)]
        pub fn get_basket(&self) -> Vec<AssetShare> {
            self.basket.clone()
        }
    }
}
