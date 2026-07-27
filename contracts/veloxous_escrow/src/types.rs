use soroban_sdk::{contracterror, contracttype, Address, String};

// ── Timing constants ──────────────────────────────────────────────────────────
/// Seconds after Funded that the seller must mark Shipped, otherwise auto_refund is callable.
pub const SHIPPING_DEADLINE_SECS: u64 = 7 * 24 * 60 * 60; // 7 days

/// Seconds after Shipped that the buyer must confirm Delivered, otherwise auto_release is callable.
pub const ACCEPTANCE_DEADLINE_SECS: u64 = 14 * 24 * 60 * 60; // 14 days

/// Protocol fee in basis points (150 BPS = 1.50%).
pub const PROTOCOL_FEE_BPS: i128 = 150;
pub const BPS_DENOMINATOR: i128 = 10_000;

// ── State machine ─────────────────────────────────────────────────────────────

/// Strict linear escrow lifecycle.
/// Numeric discriminants are stored to guarantee zero-ambiguity state checks.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowStatus {
    AwaitingFunds = 0,
    Funded = 1,
    Shipped = 2,
    Delivered = 3,
    Completed = 4,
    Disputed = 5,
    Refunded = 6,
}

// ── Core record ───────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRecord {
    pub transaction_id: String,
    pub buyer: Address,
    pub seller: Address,
    pub amount: i128,
    pub asset: Address,
    pub current_state: EscrowStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub dispute_reason: Option<String>,
    /// Ledger timestamp at which the escrow entered Funded state; used for shipping deadline.
    pub funded_at: u64,
    /// Ledger timestamp at which the escrow entered Shipped state; used for acceptance deadline.
    pub shipped_at: u64,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Instance-scoped singleton config
    Admin,
    TreasuryContract,
    AcceptedAsset,
    Initialized,
    /// Persistent per-escrow record keyed by transaction_id
    Escrow(String),
}

// ── Error codes ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// The requested state transition is not allowed from the current state.
    InvalidStateTransition = 1,
    /// An escrow with this transaction_id already exists.
    AlreadyExists = 2,
    /// No escrow was found for the given transaction_id.
    NotFound = 3,
    /// The amount transferred does not match the required price.
    AmountMismatch = 4,
    /// The asset does not match the accepted USDC token address.
    AssetMismatch = 5,
    /// The caller is not authorised to perform this action.
    Unauthorized = 6,
    /// The requested action is blocked while the escrow is in Disputed state.
    DisputeActive = 7,
    /// The shipping deadline has not yet elapsed.
    ShippingDeadlineNotElapsed = 8,
    /// The acceptance deadline has not yet elapsed.
    AcceptanceDeadlineNotElapsed = 9,
    /// Contract has already been initialized.
    AlreadyInitialized = 10,
    /// Contract has not been initialized.
    NotInitialized = 11,
    /// Integer overflow detected.
    Overflow = 12,
    /// The provided amount is invalid (≤ 0).
    InvalidAmount = 13,
}
