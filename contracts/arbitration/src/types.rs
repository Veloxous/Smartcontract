use soroban_sdk::{contracttype, Address, String, Vec};

/// Verdict options for a dispute case.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Buyer wins - funds returned to buyer
    BuyerWins = 0,
    /// Seller wins - funds released to seller
    SellerWins = 1,
}

/// Lifecycle states for an arbitration case.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CaseStatus {
    /// Case is active, awaiting juror votes
    Active = 0,
    /// Case has been resolved with majority verdict
    Resolved = 1,
}

/// Record of a juror's stake and participation history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JurorRecord {
    /// Juror's address
    pub address: Address,
    /// Amount of USDC staked
    pub staked_amount: i128,
    /// Number of arbitration cases participated in
    pub cases_participated: u32,
    /// Number of cases where juror voted with the majority
    pub cases_won: u32,
    /// Timestamp of last case participation (for lockup calculation)
    pub last_case_timestamp: u64,
}

/// Represents an arbitration case for a disputed transaction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbitrationCase {
    /// Unique case identifier
    pub case_id: String,
    /// Transaction ID from the escrow contract
    pub transaction_id: String,
    /// Buyer address from the disputed transaction
    pub buyer: Address,
    /// Seller address from the disputed transaction
    pub seller: Address,
    /// Amount in dispute
    pub amount: i128,
    /// Token asset address
    pub token: Address,
    /// Selected jurors (exactly 3)
    pub jurors: Vec<Address>,
    /// Current case status
    pub status: CaseStatus,
    /// Timestamp when case was created
    pub created_at: u64,
    /// Final verdict (set when resolved)
    pub final_verdict: Option<Verdict>,
}

/// Tracks votes for a specific case.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseVotes {
    /// Votes for BuyerWins verdict
    pub buyer_wins_votes: u32,
    /// Votes for SellerWins verdict
    pub seller_wins_votes: u32,
    /// List of jurors who have voted
    pub voted_jurors: Vec<Address>,
    /// Mapping of juror to their vote (stored separately)
    pub votes: Vec<JurorVote>,
}

/// Individual juror vote record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JurorVote {
    /// Juror's address
    pub juror: Address,
    /// Verdict cast by the juror
    pub verdict: Verdict,
}

/// Storage keys for the arbitration contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    // Instance Storage
    /// Admin address for contract governance
    Admin,
    /// USDC token contract address
    UsdcToken,
    /// Whether contract has been initialized
    Initialized,
    /// Minimum stake amount required to become a juror
    MinStakeAmount,
    /// Lockup period in seconds (default: 7 days)
    LockupPeriodSecs,

    // Persistent Storage
    /// Juror record by address
    Juror(Address),
    /// List of all staked juror addresses
    JurorPool,
    /// Arbitration case by case_id
    Case(String),
    /// Case votes by case_id
    CaseVotes(String),
    /// Escrow contract address (authorized to create cases)
    EscrowContract,
    /// Case counter for generating unique case IDs
    CaseCounter,
}
