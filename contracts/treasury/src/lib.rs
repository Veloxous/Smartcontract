#![no_std]

pub mod events;
pub mod types;

use soroban_sdk::{contract, contractimpl, token, Address, Env, String, Vec};
use types::*;

#[contract]
pub struct TreasuryContract;

#[contractimpl]
impl TreasuryContract {
    /// Initialize the Treasury Fee Engine with an admin address, initial fee BPS, and treasury wallet splits.
    ///
    /// # Arguments
    /// * `admin` - Address authorized for governance updates.
    /// * `fee_bps` - Initial fee represented in Basis Points (100 BPS = 1.00%). Max 500 BPS (5.00%).
    /// * `splits` - Vector of `TreasurySplit` structs whose `share_bps` must total 10,000 (100.00%).
    pub fn init(
        env: Env,
        admin: Address,
        fee_bps: u32,
        splits: Vec<TreasurySplit>,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        if fee_bps > MAX_FEE_BPS {
            return Err(Error::FeeExceedsLimit);
        }

        Self::validate_splits(&splits)?;

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
        env.storage()
            .instance()
            .set(&DataKey::TreasurySplits, &splits);
        env.storage().instance().set(&DataKey::Initialized, &true);

        Ok(())
    }

    /// Calculate the protocol fee for a given base amount using fixed-point arithmetic.
    ///
    /// Formula: `fee = (base_amount * fee_bps) / 10,000`
    /// Rounds down in favor of the user and prevents integer overflow.
    ///
    /// # Arguments
    /// * `base_amount` - The transaction base amount in smallest token units.
    pub fn calculate_fee(env: Env, base_amount: i128) -> Result<i128, Error> {
        if base_amount < 0 {
            return Err(Error::InvalidAmount);
        }
        if base_amount == 0 {
            return Ok(0);
        }

        let fee_bps: u32 = env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0);

        let fee_bps_i128 = fee_bps as i128;
        let denominator = BPS_DENOMINATOR as i128;

        let fee = base_amount
            .checked_mul(fee_bps_i128)
            .ok_or(Error::Overflow)?
            / denominator;

        if fee > base_amount {
            return Err(Error::Overflow);
        }

        Ok(fee)
    }

    /// Route collected fee amount across configured treasury wallets according to their split BPS.
    ///
    /// # Arguments
    /// * `asset` - Token address of the fee asset.
    /// * `amount` - Total fee amount to be distributed.
    pub fn route_fee(env: Env, asset: Address, amount: i128) -> Result<(), Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if !Self::is_supported_asset(env.clone(), asset.clone()) {
            return Err(Error::AssetNotSupported);
        }

        let splits: Vec<TreasurySplit> = env
            .storage()
            .instance()
            .get(&DataKey::TreasurySplits)
            .ok_or(Error::NotInitialized)?;

        let token_client = token::Client::new(&env, &asset);
        let contract_addr = env.current_contract_address();

        let mut distributed: i128 = 0;
        let n = splits.len();

        for i in 0..n {
            let split = splits.get(i).unwrap();
            let wallet_amount = if i == n - 1 {
                // Assign remaining rounding remainder to last wallet to ensure total equals amount
                amount - distributed
            } else {
                (amount * split.share_bps as i128) / (BPS_DENOMINATOR as i128)
            };

            if wallet_amount > 0 {
                token_client.transfer(&contract_addr, &split.wallet, &wallet_amount);
                distributed += wallet_amount;
            }
        }

        let timestamp = env.ledger().timestamp();
        events::emit_fee_collected(&env, contract_addr, asset, amount, timestamp);

        Ok(())
    }

    /// Update the global fee Basis Points (BPS).
    /// Must be called by authorized admin and cannot exceed `MAX_FEE_BPS` (500 BPS / 5.00%).
    pub fn update_fee_bps(env: Env, admin: Address, new_fee_bps: u32) -> Result<(), Error> {
        admin.require_auth();
        Self::ensure_admin(&env, &admin)?;

        if new_fee_bps > MAX_FEE_BPS {
            return Err(Error::FeeExceedsLimit);
        }

        let old_fee_bps: u32 = env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0);

        env.storage().instance().set(&DataKey::FeeBps, &new_fee_bps);

        let timestamp = env.ledger().timestamp();
        events::emit_fee_parameters_updated(
            &env,
            String::from_str(&env, "fee_bps"),
            old_fee_bps,
            new_fee_bps,
            timestamp,
        );

        Ok(())
    }

    /// Update the treasury wallet distribution splits.
    /// Must be called by authorized admin. Split share BPS must sum to exactly 10,000 (100.00%).
    pub fn update_treasury_splits(
        env: Env,
        admin: Address,
        new_splits: Vec<TreasurySplit>,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::ensure_admin(&env, &admin)?;

        Self::validate_splits(&new_splits)?;

        env.storage()
            .instance()
            .set(&DataKey::TreasurySplits, &new_splits);

        let timestamp = env.ledger().timestamp();
        events::emit_fee_parameters_updated(
            &env,
            String::from_str(&env, "treasury_splits"),
            0,
            BPS_DENOMINATOR,
            timestamp,
        );

        Ok(())
    }

    /// Register a token asset as supported by the treasury.
    pub fn add_supported_asset(env: Env, admin: Address, asset: Address) -> Result<(), Error> {
        admin.require_auth();
        Self::ensure_admin(&env, &admin)?;

        env.storage()
            .instance()
            .set(&DataKey::SupportedAsset(asset), &true);
        Ok(())
    }

    /// Check if a token asset is supported by the treasury.
    pub fn is_supported_asset(env: Env, asset: Address) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::SupportedAsset(asset))
            .unwrap_or(true) // Default to true if not explicitly restricted
    }

    /// Read the current configured fee BPS.
    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0)
    }

    /// Read the current configured treasury splits.
    pub fn get_treasury_splits(env: Env) -> Vec<TreasurySplit> {
        env.storage()
            .instance()
            .get(&DataKey::TreasurySplits)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Validate that treasury splits sum to exactly 10,000 (100.00%).
    fn validate_splits(splits: &Vec<TreasurySplit>) -> Result<(), Error> {
        if splits.is_empty() {
            return Err(Error::InvalidSplitTotal);
        }

        let mut total_bps: u32 = 0;
        for i in 0..splits.len() {
            let split = splits.get(i).unwrap();
            total_bps = total_bps
                .checked_add(split.share_bps)
                .ok_or(Error::Overflow)?;
        }

        if total_bps != BPS_DENOMINATOR {
            return Err(Error::InvalidSplitTotal);
        }

        Ok(())
    }

    /// Ensure caller is the registered admin.
    fn ensure_admin(env: &Env, caller: &Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if caller != &admin {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
