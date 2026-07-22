use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Locked,
    Released,
    Refunded,
    Disputed,
    Resolved,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowState {
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128, // total_locked
    pub status: EscrowStatus,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub transaction_id: String,
    pub buyer: Address,
    pub seller: Address,
    pub reason_hash: BytesN<32>,
    pub timestamp: u64,
    pub raised_by: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalState {
    pub votes: Vec<Address>,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    // Instance Storage
    Admins,
    Threshold,
    ReputationContract,
    TreasuryContract,
    Initialized,

    // Persistent Storage
    Escrow(String),   // transaction_id / listing_id -> EscrowState
    Dispute(String),  // transaction_id -> Dispute
    FeePool(Address), // asset address -> accumulated i128 fee pool

    // Temporary Storage
    Proposal(String, i128, i128), // (transaction_id, refund, payout) -> ProposalState
    AdminChangeProposal(Address, Address), // (old_admin, new_admin) -> Vec<Address>
}
