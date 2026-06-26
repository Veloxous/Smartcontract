use soroban_sdk::{contracttype, Address};

#[contracttype]
pub enum VaultKey {
    UsdcSac,
    Registry,
    TotalInvestments,
    ProjectInvestment(u32),
    /// Global yield-per-share accumulator, scaled by YIELD_SCALE (#125).
    YieldPerShareAccum,
    /// Per-shareholder checkpoint: yield-per-share value at last claim (#125).
    YieldDebt(Address),
    /// Insurance fund USDC balance (#135).
    InsuranceFund,
    /// Whether a project default claim has been paid out (#135).
    InsuranceClaimed(u32),
    /// Lifetime USDC deposited by an investor — used in portfolio analytics (#132).
    TotalDeposited(Address),
}

/// On-chain portfolio snapshot for a single investor (#132).
#[contracttype]
pub struct PortfolioInfo {
    /// HBS shares currently held.
    pub shares: i128,
    /// Current USDC redemption value of those shares.
    pub usdc_value: i128,
    /// Unclaimed yield in USDC.
    pub claimable_yield: i128,
    /// Shares as a fraction of total supply, in basis points (0-10 000).
    pub share_of_pool_bps: i128,
    /// Lifetime USDC deposited by this investor.
    pub total_deposited: i128,
}
