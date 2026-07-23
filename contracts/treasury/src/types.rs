use soroban_sdk::{contracterror, contracttype, Address};

pub const MAX_FEE_BPS: u32 = 500; // 5.00%
pub const BPS_DENOMINATOR: u32 = 10_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasurySplit {
    pub wallet: Address,
    pub share_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    FeeBps,
    TreasurySplits,
    Admin,
    SupportedAsset(Address),
    Initialized,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    FeeExceedsLimit = 1,
    InvalidSplitTotal = 2,
    AssetNotSupported = 3,
    FeePoolEmpty = 4,
    AlreadyInitialized = 5,
    NotInitialized = 6,
    Unauthorized = 7,
    InvalidAmount = 8,
    Overflow = 9,
}
