#![no_std]

pub mod events;
pub mod fractional;
pub mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, String};
use types::Error;

use crate::fractional::{
    buy_out, claim_proceeds, distribute_proceeds, fractionalize, get_listing_state,
    get_share_balance, record_sale, register_device, transfer_shares,
};
use crate::types::{DataKey, FractionListing};

#[contract]
pub struct DeviceNft;

#[contractimpl]
impl DeviceNft {
    /// Initialize the fractional NFT contract.
    ///
    /// * `admin` — contract administrator.
    /// * `usdc_asset` — accepted USDC token address.
    /// * `marketplace` — authorized caller that may record device sales.
    pub fn init(
        env: Env,
        admin: Address,
        usdc_asset: Address,
        marketplace: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::UsdcAsset, &usdc_asset);
        env.storage()
            .instance()
            .set(&DataKey::Marketplace, &marketplace);
        env.storage().instance().set(&DataKey::Initialized, &true);
        Ok(())
    }

    /// Register a device NFT listing with a USDC valuation used for buy-out pricing.
    pub fn register_device(
        env: Env,
        owner: Address,
        listing_id: String,
        valuation: i128,
    ) -> Result<(), Error> {
        register_device(&env, owner, listing_id, valuation)
    }

    /// Lock the device NFT in the contract and mint fractional share tokens to the owner.
    pub fn fractionalize(
        env: Env,
        owner: Address,
        listing_id: String,
        shares: u32,
    ) -> Result<(), Error> {
        fractionalize(&env, owner, listing_id, shares)
    }

    /// Transfer fractional shares between holders (standard fungible-token semantics).
    pub fn transfer_shares(
        env: Env,
        from: Address,
        to: Address,
        listing_id: String,
        amount: i128,
    ) -> Result<(), Error> {
        transfer_shares(&env, from, to, listing_id, amount)
    }

    /// Purchase all remaining shares at the outstanding share value and receive the device NFT.
    pub fn buy_out(env: Env, buyer: Address, listing_id: String) -> Result<(), Error> {
        buy_out(&env, buyer, listing_id)
    }

    /// Record a completed device sale and deposit USDC proceeds into the contract.
    pub fn record_sale(
        env: Env,
        caller: Address,
        listing_id: String,
        proceeds: i128,
    ) -> Result<(), Error> {
        record_sale(&env, caller, listing_id, proceeds)
    }

    /// Pull-based claim of proportional sale proceeds for the caller's shares.
    pub fn claim_proceeds(
        env: Env,
        holder: Address,
        listing_id: String,
    ) -> Result<i128, Error> {
        claim_proceeds(&env, holder, listing_id)
    }

    /// Issue-spec alias for pull-based proceeds claiming.
    pub fn distribute_proceeds(
        env: Env,
        holder: Address,
        listing_id: String,
    ) -> Result<i128, Error> {
        distribute_proceeds(&env, holder, listing_id)
    }

    pub fn get_share_balance(env: Env, listing_id: String, holder: Address) -> i128 {
        get_share_balance(&env, listing_id, holder)
    }

    pub fn get_listing(env: Env, listing_id: String) -> Result<FractionListing, Error> {
        get_listing_state(&env, listing_id)
    }
}

mod test;
