use soroban_sdk::{contractclient, Address, Env};

/// Abstract interface for an external yield-bearing protocol.
///
/// The vault forwards idle escrow collateral here via cross-contract call.
/// Real implementations (e.g. a lending pool) settle `deposit`/`withdraw`
/// against their own accounting; this vault only needs the amount handed
/// back on withdrawal to know how much yield accrued.
#[contractclient(name = "YieldProtocolClient")]
pub trait YieldProtocolTrait {
    /// Quote (and register) a deposit of `amount` of `asset`, returning an
    /// opaque receipt/share amount the vault must present again on
    /// withdrawal. Does *not* move any tokens — the vault only transfers
    /// funds in after this call succeeds.
    ///
    /// Splitting the quote from the transfer is what makes the vault's
    /// circuit breaker safe: `try_deposit` catches any trap here (protocol
    /// unavailable, paused, etc.) before a single token has moved, so a
    /// failure can never leave funds stuck mid-transfer.
    fn deposit(env: Env, asset: Address, amount: i128) -> i128;

    /// Return the deposited principal plus any accrued yield for `shares`,
    /// pushing `asset` back to `to` (the vault) from the protocol's own
    /// balance in the same call.
    fn withdraw(env: Env, to: Address, asset: Address, shares: i128) -> i128;
}
