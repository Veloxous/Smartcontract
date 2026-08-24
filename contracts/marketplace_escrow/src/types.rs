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

/// Lifecycle states for a Dutch auction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AuctionStatus {
    /// Auction is live; price decays over time.
    Active = 0,
    /// A buyer called buy_now before expiry.
    Sold = 1,
    /// Duration elapsed without a buyer; listing returned to active.
    Expired = 2,
}

/// Persistent state for a single Dutch auction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionState {
    /// Address of the device / listing owner.
    pub seller: Address,
    /// USDC (or other) token accepted for payment.
    pub usdc_asset: Address,
    /// Starting (highest) price in token base units.
    pub start_price: i128,
    /// Ending (floor) price in token base units.
    pub end_price: i128,
    /// Auction start timestamp (ledger UNIX seconds).
    pub start_time: u64,
    /// Duration in seconds over which price decays from start → end.
    pub duration_secs: u64,
    /// Current lifecycle status.
    pub status: AuctionStatus,
    /// Address of the winning buyer (Some once Sold).
    pub buyer: Option<Address>,
    /// Final captured price paid by the buyer (set once Sold).
    pub final_price: Option<i128>,
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
    Auction(String),  // listing_id -> AuctionState

    // Temporary Storage
    Proposal(String, i128, i128), // (transaction_id, refund, payout) -> ProposalState
    AdminChangeProposal(Address, Address), // (old_admin, new_admin) -> Vec<Address>
}
