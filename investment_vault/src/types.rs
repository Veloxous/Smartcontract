use soroban_sdk::{contracttype, Address, String};

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
    /// Optional management fee in basis points, admin-set, hard-capped (#7).
    ManagementFeeBps,
    /// Recipient address for management fee transfers (#7).
    ManagementFeeRecipient,
    /// Whether secondary market trading of HBS is active (#126).
    TradingEnabled,
}

/// Metadata returned for DEX listing and secondary market integration (#126).
#[contracttype]
#[derive(Clone, Debug)]
pub struct HBSTokenInfo {
    /// Human-readable token name.
    pub name: String,
    /// Ticker symbol.
    pub symbol: String,
    /// Number of decimal places (7 for USDC-parity denominations).
    pub decimals: u32,
    /// Whether the admin has enabled secondary trading.
    pub trading_enabled: bool,
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
