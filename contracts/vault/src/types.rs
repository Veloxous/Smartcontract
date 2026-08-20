use soroban_sdk::{contracterror, contracttype, Address, String};

// ── Core record ───────────────────────────────────────────────────────────────

/// Tracks a single escrow's deposit while it sits idle inside the vault.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultRecord {
    pub transaction_id: String,
    pub asset: Address,
    /// Original amount deposited by the escrow contract.
    pub principal: i128,
    pub deposited_at: u64,
    /// Yield collected on withdrawal (0 until `withdraw` is called).
    pub yield_earned: i128,
    /// Whether the principal was actually forwarded into the external yield
    /// protocol. False when the circuit breaker fell back to holding the
    /// funds directly in this contract.
    pub in_yield_protocol: bool,
    /// Opaque receipt/share amount returned by the yield protocol on
    /// deposit; only meaningful when `in_yield_protocol` is true.
    pub shares: i128,
    pub withdrawn: bool,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    /// Only this address may call `deposit` / `withdraw`.
    EscrowContract,
    /// Optional external yield-bearing protocol. `None` means the circuit
    /// breaker is permanently tripped and deposits are always held directly.
    YieldProtocol,
    /// Optional treasury that collects yield on withdrawal.
    TreasuryContract,
    Initialized,
    Vault(String),
}

// ── Error codes ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    AlreadyExists = 5,
    NotFound = 6,
    AlreadyWithdrawn = 7,
    Overflow = 8,
    /// The yield protocol failed (or was unreachable) while withdrawing
    /// funds that had previously been deposited into it.
    YieldProtocolUnavailable = 9,
}
