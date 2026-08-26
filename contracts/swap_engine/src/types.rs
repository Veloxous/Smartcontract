use soroban_sdk::{contracttype, Address, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SwapState {
    Proposed,
    AFunded,
    BFunded,
    FullyFunded,
    AConfirmed,
    BConfirmed,
    Completed,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapRecord {
    pub swap_id: u64,
    pub party_a: Address,
    pub party_b: Address,
    pub device_a: Address, // Representation of the device A (could be NFT or an ID mapped to Oracle)
    pub device_b: Address, // Representation of the device B
    pub collateral_a_amount: i128,
    pub collateral_b_amount: i128,
    pub state: SwapState,
    pub proposed_at: u64,
    pub a_funded_at: u64,
    pub b_funded_at: u64,
}
