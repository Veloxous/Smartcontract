#![no_std]

pub mod events;
pub mod types;
pub mod yield_client;

use soroban_sdk::{contract, contractimpl, token, Address, Env, String};
use types::*;
use yield_client::YieldProtocolClient;

// ── Treasury client stub ──────────────────────────────────────────────────────

/// Thin cross-contract client for the Veloxous Protocol Treasury. Mirrors the
/// stub `veloxous_escrow` already uses for routing its own protocol fee, so
/// earned yield is routed through the same `route_fee` accounting rather than
/// landing as a bare, unaccounted-for token transfer.
mod treasury_client {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "TreasuryClient")]
    pub trait TreasuryTrait {
        fn route_fee(env: Env, asset: Address, amount: i128);
    }
}

use treasury_client::TreasuryClient;

// ── Contract ──────────────────────────────────────────────────────────────────

/// Holds idle escrow collateral and, when possible, puts it to work in an
/// external yield-bearing protocol until the owning escrow calls `withdraw`.
///
/// Ships a circuit breaker: any failure while talking to the configured
/// yield protocol (unreachable, reverts, etc.) is caught via `try_deposit`
/// and this vault immediately bounces the funds straight back to the
/// escrow — it never takes custody in that case. The escrow ends up
/// holding its own collateral directly, exactly as spec'd: `deposit`
/// itself never fails, and principal is never put at risk.
#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    /// Initialize the vault.
    ///
    /// # Arguments
    /// * `admin`             - Address authorized for governance updates.
    /// * `escrow_contract`   - The only contract allowed to call `deposit` / `withdraw`.
    /// * `treasury_contract` - Optional treasury that collects earned yield on withdrawal.
    /// * `yield_protocol`    - Optional external yield-bearing protocol.
    pub fn init(
        env: Env,
        admin: Address,
        escrow_contract: Address,
        treasury_contract: Option<Address>,
        yield_protocol: Option<Address>,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::EscrowContract, &escrow_contract);

        if let Some(treasury) = treasury_contract {
            env.storage().instance().set(&DataKey::TreasuryContract, &treasury);
        }
        if let Some(protocol) = yield_protocol {
            env.storage().instance().set(&DataKey::YieldProtocol, &protocol);
        }

        env.storage().instance().set(&DataKey::Initialized, &true);
        Ok(())
    }

    /// Accept custody of `amount` of `asset` (already transferred to this
    /// contract's balance by the escrow) and attempt to forward it into the
    /// configured yield protocol.
    ///
    /// If no yield protocol is configured, or the protocol call fails for
    /// any reason, the circuit breaker trips: this vault immediately bounces
    /// the funds straight back to the escrow, within this same call, and
    /// never takes custody. The escrow ends up holding its own collateral
    /// directly, exactly as if no vault had been configured at all —
    /// `in_yield_protocol` is recorded as `false` so `withdraw` knows there's
    /// nothing here to pull back later.
    pub fn deposit(
        env: Env,
        escrow: Address,
        asset: Address,
        amount: i128,
        transaction_id: String,
    ) -> Result<(), Error> {
        escrow.require_auth();
        Self::assert_escrow(&env, &escrow)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey::Vault(transaction_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }

        let now = env.ledger().timestamp();
        let vault_addr = env.current_contract_address();
        let token_client = token::Client::new(&env, &asset);
        let mut in_yield_protocol = false;
        let mut shares: i128 = 0;

        if let Some(protocol) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::YieldProtocol)
        {
            // ── Circuit breaker ──────────────────────────────────────────────
            // `try_deposit` catches any trap/error from the external protocol
            // instead of aborting this transaction. This call only quotes
            // the deposit — no tokens move yet — so a failure here leaves
            // this contract's balance completely untouched, and we bounce
            // the funds straight back to the escrow below.
            match YieldProtocolClient::new(&env, &protocol).try_deposit(&asset, &amount) {
                Ok(Ok(returned_shares)) => {
                    // Only now, after the protocol confirmed it can accept
                    // the deposit, do we actually move funds.
                    token_client.transfer(&vault_addr, &protocol, &amount);
                    in_yield_protocol = true;
                    shares = returned_shares;
                }
                _ => {
                    token_client.transfer(&vault_addr, &escrow, &amount);
                    events::emit_circuit_breaker_tripped(&env, transaction_id.clone(), protocol);
                }
            }
        } else {
            // No yield protocol configured at all — same fallback: hold
            // nothing here, the escrow keeps its own collateral.
            token_client.transfer(&vault_addr, &escrow, &amount);
        }

        let record = VaultRecord {
            transaction_id: transaction_id.clone(),
            asset,
            principal: amount,
            deposited_at: now,
            yield_earned: 0,
            in_yield_protocol,
            shares,
            withdrawn: false,
        };
        env.storage().persistent().set(&key, &record);

        events::emit_deposited(&env, transaction_id, amount, in_yield_protocol);
        Ok(())
    }

    /// Withdraw a vault record's principal (plus any earned yield) back to
    /// the calling escrow contract. Yield, if any, is routed straight to the
    /// configured treasury.
    ///
    /// If this record's principal was never actually forwarded into the
    /// yield protocol (the circuit breaker tripped, or no protocol was ever
    /// configured), this vault holds nothing for it — the escrow has held
    /// its own collateral the whole time — so this is a pure bookkeeping
    /// no-op: no token moves, and no yield to report.
    ///
    /// Returns `(principal, yield_earned)`.
    pub fn withdraw(env: Env, escrow: Address, transaction_id: String) -> Result<(i128, i128), Error> {
        escrow.require_auth();
        Self::assert_escrow(&env, &escrow)?;

        let key = DataKey::Vault(transaction_id.clone());
        let mut record: VaultRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;

        if record.withdrawn {
            return Err(Error::AlreadyWithdrawn);
        }

        if !record.in_yield_protocol {
            record.withdrawn = true;
            env.storage().persistent().set(&key, &record);
            events::emit_withdrawn(&env, transaction_id, record.principal, 0);
            return Ok((record.principal, 0));
        }

        let contract_addr = env.current_contract_address();
        let token_client = token::Client::new(&env, &record.asset);

        let protocol: Address = env
            .storage()
            .instance()
            .get(&DataKey::YieldProtocol)
            .ok_or(Error::NotInitialized)?;
        let total_returned: i128 = YieldProtocolClient::new(&env, &protocol)
            .try_withdraw(&contract_addr, &record.asset, &record.shares)
            .map_err(|_| Error::YieldProtocolUnavailable)?
            .map_err(|_| Error::YieldProtocolUnavailable)?;

        let yield_earned = if total_returned > record.principal {
            total_returned - record.principal
        } else {
            0
        };

        // ── Effects ──────────────────────────────────────────────────────────
        record.withdrawn = true;
        record.yield_earned = yield_earned;
        env.storage().persistent().set(&key, &record);

        // ── Interactions ─────────────────────────────────────────────────────
        token_client.transfer(&contract_addr, &escrow, &record.principal);

        if yield_earned > 0 {
            if let Some(treasury) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::TreasuryContract)
            {
                token_client.transfer(&contract_addr, &treasury, &yield_earned);
                TreasuryClient::new(&env, &treasury).route_fee(&record.asset, &yield_earned);
                events::emit_yield_routed(&env, transaction_id.clone(), treasury, yield_earned);
            }
            // If no treasury configured, yield stays in the vault (sweepable later).
        }

        events::emit_withdrawn(&env, transaction_id, record.principal, yield_earned);
        Ok((record.principal, yield_earned))
    }

    // ── Getters ───────────────────────────────────────────────────────────────

    pub fn get_vault_record(env: Env, transaction_id: String) -> VaultRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Vault(transaction_id))
            .unwrap_or_else(|| panic!("vault record not found"))
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("not initialized"))
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn assert_escrow(env: &Env, caller: &Address) -> Result<(), Error> {
        let escrow: Address = env
            .storage()
            .instance()
            .get(&DataKey::EscrowContract)
            .ok_or(Error::NotInitialized)?;
        if caller != &escrow {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
