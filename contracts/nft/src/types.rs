use soroban_sdk::{contracterror, contracttype, Address, String};

/// Lifecycle of a fractionalized device listing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ListingState {
    /// Shares issued; device NFT locked in the contract.
    Active = 0,
    /// Device sold; holders may claim USDC proceeds.
    Sold = 1,
    /// A buyer acquired all outstanding shares and received the NFT.
    BoughtOut = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionListing {
    pub owner: Address,
    pub total_shares: u32,
    /// USDC price per share for buy-out calculations.
    pub share_price: i128,
    /// Total USDC proceeds deposited after a sale.
    pub sale_proceeds: i128,
    pub state: ListingState,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    UsdcAsset,
    Marketplace,
    Initialized,
    /// Device NFT owner; contract address means custodied/locked.
    DeviceOwner(String),
    /// USDC valuation recorded at device registration.
    DeviceValuation(String),
    /// Per-listing fractionalization metadata.
    Listing(String),
    /// Share balance for `(listing_id, holder)`.
    ShareBalance(String, Address),
    /// USDC already claimed by `(listing_id, holder)`.
    Claimed(String, Address),
    /// Shareholders for a listing (updated on mint/transfer).
    Holders(String),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    NotFound = 4,
    AlreadyExists = 5,
    InvalidShares = 6,
    InvalidAmount = 7,
    NotOwner = 8,
    AlreadyFractionalized = 9,
    NotFractionalized = 10,
    InvalidState = 11,
    InsufficientShares = 12,
    InsufficientBalance = 13,
    AssetMismatch = 14,
    AlreadyClaimed = 15,
    Overflow = 16,
}
