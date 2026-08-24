#![no_std]

pub mod batch;
pub mod events;
pub mod types;

use soroban_sdk::{contract, contractimpl, token, Address, Env, String, Vec};
use types::*;

// ── Treasury client stub ──────────────────────────────────────────────────────

/// Thin cross-contract client for the Veloxous Treasury.
/// Only the `route_fee` entry point is needed here.
mod treasury_client {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "TreasuryClient")]
    pub trait TreasuryTrait {
        fn route_fee(env: Env, asset: Address, amount: i128);
    }
}

use treasury_client::TreasuryClient;

// ── Vault client stub ────────────────────────────────────────────────────────

/// Thin cross-contract client for the Veloxous yield Vault.
///
/// Declared locally (rather than depending on the `vault` crate directly)
/// so this contract's own `#[contract]` build never links against another
/// contract's exported entry points — two `#[contractimpl]` blocks sharing a
/// function name like `init` in the same wasm binary is a hard link error.
mod vault_client {
    use soroban_sdk::{contractclient, contracterror, Address, Env, String};

    #[contracterror]
    #[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
    #[repr(u32)]
    pub enum VaultError {
        Unauthorized = 3,
        NotFound = 6,
        AlreadyWithdrawn = 7,
        YieldProtocolUnavailable = 9,
    }

    #[contractclient(name = "VaultClient")]
    pub trait VaultTrait {
        fn deposit(
            env: Env,
            escrow: Address,
            asset: Address,
            amount: i128,
            transaction_id: String,
        ) -> Result<(), VaultError>;

        fn withdraw(env: Env, escrow: Address, transaction_id: String) -> Result<(i128, i128), VaultError>;
    }
}

use vault_client::VaultClient;

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct VeloxousEscrow;

#[contractimpl]
impl VeloxousEscrow {
    // ── Initialization ────────────────────────────────────────────────────────

    /// Initialize the contract with an admin address, the sole accepted asset (USDC), and an
    /// optional treasury contract address for fee routing.
    ///
    /// # Arguments
    /// * `admin`             - Multisig or single admin address.
    /// * `accepted_asset`    - Strictly accepted USDC token address on Stellar.
    /// * `treasury_contract` - Optional treasury contract for protocol fee routing.
    /// * `vault_contract`    - Optional yield vault that holds idle collateral between
    ///                         `fund_escrow` and the escrow's eventual release/refund.
    pub fn init(
        env: Env,
        admin: Address,
        accepted_asset: Address,
        treasury_contract: Option<Address>,
        vault_contract: Option<Address>,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::AcceptedAsset, &accepted_asset);

        if let Some(treasury) = treasury_contract {
            env.storage().instance().set(&DataKey::TreasuryContract, &treasury);
        }
        if let Some(vault) = vault_contract {
            env.storage().instance().set(&DataKey::VaultContract, &vault);
        }

        env.storage().instance().set(&DataKey::Initialized, &true);
        Ok(())
    }

    // ── Phase 1 — Deposit & Lock ──────────────────────────────────────────────

    /// Buyer deposits the exact expected USDC amount into escrow.
    ///
    /// Validates:
    /// - `buyer.require_auth()` — transaction must be signed by the buyer.
    /// - Asset must match the accepted USDC address configured at init.
    /// - Amount must be > 0.
    /// - No escrow may already exist for `transaction_id`.
    ///
    /// Transitions: (none → created) AwaitingFunds → Funded
    pub fn fund_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        asset: Address,
        amount: i128,
        transaction_id: String,
    ) -> Result<(), Error> {
        buyer.require_auth();

        // Validate asset strictly matches accepted USDC
        let accepted: Address = env
            .storage()
            .instance()
            .get(&DataKey::AcceptedAsset)
            .ok_or(Error::NotInitialized)?;
        if asset != accepted {
            return Err(Error::AssetMismatch);
        }

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey::Escrow(transaction_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }

        let now = env.ledger().timestamp();

        // ── Check-Effects-Interactions ────────────────────────────────────────
        // 1. Effects: write state before any external call
        let record = EscrowRecord {
            transaction_id: transaction_id.clone(),
            buyer: buyer.clone(),
            seller,
            amount,
            asset: asset.clone(),
            current_state: EscrowStatus::Funded,
            created_at: now,
            updated_at: now,
            dispute_reason: None,
            funded_at: now,
            shipped_at: 0,
        };
        env.storage().persistent().set(&key, &record);

        // 2. Interactions: external token transfer
        let contract_addr = env.current_contract_address();
        token::Client::new(&env, &asset).transfer(&buyer, &contract_addr, &amount);

        // Forward idle collateral into the yield vault, if one is configured.
        Self::vault_deposit_if_configured(&env, &asset, amount, &transaction_id)?;

        // 3. Events
        events::emit_funded(&env, transaction_id.clone(), buyer, amount, asset);
        events::emit_status_changed(
            &env,
            transaction_id,
            EscrowStatus::AwaitingFunds,
            EscrowStatus::Funded,
            now,
        );

        Ok(())
    }

    // ── Phase 2 — Shipping ────────────────────────────────────────────────────

    /// Seller marks the item as shipped.
    ///
    /// Transition: Funded → Shipped
    pub fn mark_shipped(env: Env, seller: Address, transaction_id: String) -> Result<(), Error> {
        seller.require_auth();

        let key = DataKey::Escrow(transaction_id.clone());
        let mut record: EscrowRecord = Self::load_escrow(&env, &key)?;

        Self::assert_not_disputed(&record)?;
        Self::assert_state(&record, EscrowStatus::Funded, EscrowStatus::Shipped)?;

        if record.seller != seller {
            return Err(Error::Unauthorized);
        }

        let now = env.ledger().timestamp();
        let old = record.current_state.clone();
        record.current_state = EscrowStatus::Shipped;
        record.shipped_at = now;
        record.updated_at = now;

        env.storage().persistent().set(&key, &record);
        events::emit_status_changed(&env, transaction_id, old, EscrowStatus::Shipped, now);
        Ok(())
    }

    // ── Phase 3 — Delivery confirmation ───────────────────────────────────────

    /// Buyer confirms physical receipt of the item.
    ///
    /// Transition: Shipped → Delivered
    pub fn mark_delivered(env: Env, buyer: Address, transaction_id: String) -> Result<(), Error> {
        buyer.require_auth();

        let key = DataKey::Escrow(transaction_id.clone());
        let mut record: EscrowRecord = Self::load_escrow(&env, &key)?;

        Self::assert_not_disputed(&record)?;
        Self::assert_state(&record, EscrowStatus::Shipped, EscrowStatus::Delivered)?;

        if record.buyer != buyer {
            return Err(Error::Unauthorized);
        }

        let now = env.ledger().timestamp();
        let old = record.current_state.clone();
        record.current_state = EscrowStatus::Delivered;
        record.updated_at = now;

        env.storage().persistent().set(&key, &record);
        events::emit_status_changed(&env, transaction_id, old, EscrowStatus::Delivered, now);
        Ok(())
    }

    // ── Phase 4 — Release & Treasury Routing ──────────────────────────────────

    /// Release funds to the seller.
    ///
    /// Callable by:
    /// - The Buyer (satisfied with delivery) when state == Delivered.
    /// - The Admin (dispute resolution) when state == Disputed.
    ///
    /// Calculates protocol fee using fixed-point arithmetic (PROTOCOL_FEE_BPS / BPS_DENOMINATOR).
    /// Routes fee to treasury (if configured) and sends net amount to seller.
    ///
    /// Transition: Delivered | Disputed → Completed
    ///
    /// Follows Check-Effects-Interactions pattern.
    pub fn release_funds(env: Env, caller: Address, transaction_id: String) -> Result<(), Error> {
        caller.require_auth();

        let key = DataKey::Escrow(transaction_id.clone());
        let mut record: EscrowRecord = Self::load_escrow(&env, &key)?;

        // Authorisation: buyer (happy path) or admin (dispute resolution)
        let is_buyer = record.buyer == caller;
        let is_admin = Self::is_admin(&env, &caller);

        if !is_buyer && !is_admin {
            return Err(Error::Unauthorized);
        }

        // State guard
        match &record.current_state {
            EscrowStatus::Delivered => {
                // Normal path — only buyer or admin
                if !is_buyer && !is_admin {
                    return Err(Error::Unauthorized);
                }
            }
            EscrowStatus::Disputed => {
                // Dispute resolution — only admin
                if !is_admin {
                    return Err(Error::Unauthorized);
                }
            }
            _ => {
                return Err(Error::InvalidStateTransition);
            }
        }

        let now = env.ledger().timestamp();
        let old = record.current_state.clone();

        // ── Fixed-point fee calculation ───────────────────────────────────────
        // fee = (amount * PROTOCOL_FEE_BPS) / BPS_DENOMINATOR  (rounds down, favouring user)
        let fee = record
            .amount
            .checked_mul(PROTOCOL_FEE_BPS)
            .ok_or(Error::Overflow)?
            / BPS_DENOMINATOR;

        let seller_amount = record.amount - fee;

        // ── Check-Effects-Interactions ────────────────────────────────────────
        // 1. Effects: update state before any external call
        record.current_state = EscrowStatus::Completed;
        record.updated_at = now;
        env.storage().persistent().set(&key, &record);

        // 2. Interactions: pull collateral back from the vault (if any), then transfer
        Self::vault_withdraw_if_configured(&env, &transaction_id)?;

        let contract_addr = env.current_contract_address();
        let token_client = token::Client::new(&env, &record.asset);

        // Transfer net amount to seller
        if seller_amount > 0 {
            token_client.transfer(&contract_addr, &record.seller, &seller_amount);
        }

        // Route fee to treasury if configured
        if fee > 0 {
            if let Some(treasury_addr) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::TreasuryContract)
            {
                token_client.transfer(&contract_addr, &treasury_addr, &fee);
                TreasuryClient::new(&env, &treasury_addr).route_fee(&record.asset, &fee);
            }
            // If no treasury configured, fee stays in contract (collectible via sweep)
        }

        // 3. Events
        events::emit_status_changed(&env, transaction_id.clone(), old, EscrowStatus::Completed, now);
        events::emit_funds_released(&env, transaction_id, record.seller, seller_amount, fee, now);

        Ok(())
    }

    // ── Timeouts ──────────────────────────────────────────────────────────────

    /// Auto-refund the buyer if the seller never marks the item as Shipped within
    /// SHIPPING_DEADLINE_SECS after funding.
    ///
    /// Callable by: anyone (typically the buyer).
    /// Transition: Funded → Refunded
    pub fn auto_refund(env: Env, transaction_id: String) -> Result<(), Error> {
        let key = DataKey::Escrow(transaction_id.clone());
        let mut record: EscrowRecord = Self::load_escrow(&env, &key)?;

        Self::assert_not_disputed(&record)?;

        if record.current_state != EscrowStatus::Funded {
            return Err(Error::InvalidStateTransition);
        }

        let now = env.ledger().timestamp();
        let deadline = record
            .funded_at
            .checked_add(SHIPPING_DEADLINE_SECS)
            .ok_or(Error::Overflow)?;

        if now < deadline {
            return Err(Error::ShippingDeadlineNotElapsed);
        }

        // ── Check-Effects-Interactions ────────────────────────────────────────
        let old = record.current_state.clone();
        record.current_state = EscrowStatus::Refunded;
        record.updated_at = now;
        env.storage().persistent().set(&key, &record);

        Self::vault_withdraw_if_configured(&env, &transaction_id)?;

        token::Client::new(&env, &record.asset).transfer(
            &env.current_contract_address(),
            &record.buyer,
            &record.amount,
        );

        events::emit_status_changed(&env, transaction_id.clone(), old, EscrowStatus::Refunded, now);
        events::emit_funds_refunded(&env, transaction_id, record.buyer, record.amount, now);
        Ok(())
    }

    /// Auto-release funds to the seller if the buyer never confirms delivery within
    /// ACCEPTANCE_DEADLINE_SECS after the item was marked Shipped (buyer went AFK).
    ///
    /// Callable by: anyone (typically the seller).
    /// Transition: Shipped → Completed  (full amount to seller, fee deducted)
    pub fn auto_release(env: Env, transaction_id: String) -> Result<(), Error> {
        let key = DataKey::Escrow(transaction_id.clone());
        let mut record: EscrowRecord = Self::load_escrow(&env, &key)?;

        Self::assert_not_disputed(&record)?;

        if record.current_state != EscrowStatus::Shipped {
            return Err(Error::InvalidStateTransition);
        }

        let now = env.ledger().timestamp();
        let deadline = record
            .shipped_at
            .checked_add(ACCEPTANCE_DEADLINE_SECS)
            .ok_or(Error::Overflow)?;

        if now < deadline {
            return Err(Error::AcceptanceDeadlineNotElapsed);
        }

        let fee = record
            .amount
            .checked_mul(PROTOCOL_FEE_BPS)
            .ok_or(Error::Overflow)?
            / BPS_DENOMINATOR;
        let seller_amount = record.amount - fee;

        // ── Check-Effects-Interactions ────────────────────────────────────────
        let old = record.current_state.clone();
        record.current_state = EscrowStatus::Completed;
        record.updated_at = now;
        env.storage().persistent().set(&key, &record);

        Self::vault_withdraw_if_configured(&env, &transaction_id)?;

        let contract_addr = env.current_contract_address();
        let token_client = token::Client::new(&env, &record.asset);

        if seller_amount > 0 {
            token_client.transfer(&contract_addr, &record.seller, &seller_amount);
        }

        if fee > 0 {
            if let Some(treasury_addr) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::TreasuryContract)
            {
                token_client.transfer(&contract_addr, &treasury_addr, &fee);
                TreasuryClient::new(&env, &treasury_addr).route_fee(&record.asset, &fee);
            }
        }

        events::emit_status_changed(&env, transaction_id.clone(), old, EscrowStatus::Completed, now);
        events::emit_funds_released(&env, transaction_id, record.seller, seller_amount, fee, now);
        Ok(())
    }

    // ── Dispute ───────────────────────────────────────────────────────────────

    /// Buyer or seller raises a dispute, halting all normal execution paths.
    ///
    /// Callable from: Funded, Shipped, or Delivered states.
    /// Transition: Funded | Shipped | Delivered → Disputed
    pub fn raise_dispute(
        env: Env,
        caller: Address,
        transaction_id: String,
        reason: String,
    ) -> Result<(), Error> {
        caller.require_auth();

        let key = DataKey::Escrow(transaction_id.clone());
        let mut record: EscrowRecord = Self::load_escrow(&env, &key)?;

        // Only buyer or seller may raise a dispute
        if caller != record.buyer && caller != record.seller {
            return Err(Error::Unauthorized);
        }

        // Dispute is allowed from any active (non-terminal) state
        match &record.current_state {
            EscrowStatus::Funded | EscrowStatus::Shipped | EscrowStatus::Delivered => {}
            EscrowStatus::Disputed => return Err(Error::DisputeActive),
            _ => return Err(Error::InvalidStateTransition),
        }

        let now = env.ledger().timestamp();
        let old = record.current_state.clone();
        record.current_state = EscrowStatus::Disputed;
        record.dispute_reason = Some(reason);
        record.updated_at = now;

        env.storage().persistent().set(&key, &record);
        events::emit_status_changed(&env, transaction_id, old, EscrowStatus::Disputed, now);
        Ok(())
    }

    /// Admin resolves a dispute by choosing to either release funds to the seller or refund the buyer.
    ///
    /// `release_to_seller = true`  → Disputed → Completed (seller paid, fee deducted)
    /// `release_to_seller = false` → Disputed → Refunded  (buyer fully refunded)
    pub fn resolve_dispute(
        env: Env,
        admin: Address,
        transaction_id: String,
        release_to_seller: bool,
    ) -> Result<(), Error> {
        admin.require_auth();

        if !Self::is_admin(&env, &admin) {
            return Err(Error::Unauthorized);
        }

        let key = DataKey::Escrow(transaction_id.clone());
        let mut record: EscrowRecord = Self::load_escrow(&env, &key)?;

        if record.current_state != EscrowStatus::Disputed {
            return Err(Error::InvalidStateTransition);
        }

        let now = env.ledger().timestamp();
        let contract_addr = env.current_contract_address();
        let token_client = token::Client::new(&env, &record.asset);

        Self::vault_withdraw_if_configured(&env, &transaction_id)?;

        if release_to_seller {
            let fee = record
                .amount
                .checked_mul(PROTOCOL_FEE_BPS)
                .ok_or(Error::Overflow)?
                / BPS_DENOMINATOR;
            let seller_amount = record.amount - fee;

            record.current_state = EscrowStatus::Completed;
            record.updated_at = now;
            env.storage().persistent().set(&key, &record);

            if seller_amount > 0 {
                token_client.transfer(&contract_addr, &record.seller, &seller_amount);
            }
            if fee > 0 {
                if let Some(treasury_addr) = env
                    .storage()
                    .instance()
                    .get::<DataKey, Address>(&DataKey::TreasuryContract)
                {
                    token_client.transfer(&contract_addr, &treasury_addr, &fee);
                    TreasuryClient::new(&env, &treasury_addr).route_fee(&record.asset, &fee);
                }
            }

            events::emit_status_changed(
                &env,
                transaction_id.clone(),
                EscrowStatus::Disputed,
                EscrowStatus::Completed,
                now,
            );
            events::emit_funds_released(&env, transaction_id, record.seller, seller_amount, fee, now);
        } else {
            record.current_state = EscrowStatus::Refunded;
            record.updated_at = now;
            env.storage().persistent().set(&key, &record);

            token_client.transfer(&contract_addr, &record.buyer, &record.amount);

            events::emit_status_changed(
                &env,
                transaction_id.clone(),
                EscrowStatus::Disputed,
                EscrowStatus::Refunded,
                now,
            );
            events::emit_funds_refunded(&env, transaction_id, record.buyer, record.amount, now);
        }

        Ok(())
    }

    // ── Getters ───────────────────────────────────────────────────────────────

    pub fn get_escrow(env: Env, transaction_id: String) -> EscrowRecord {
        let key = DataKey::Escrow(transaction_id);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"))
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("not initialized"))
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn load_escrow(env: &Env, key: &DataKey) -> Result<EscrowRecord, Error> {
        env.storage()
            .persistent()
            .get(key)
            .ok_or(Error::NotFound)
    }

    /// Enforce that a transition from `expected_current` to `next` is the only valid one.
    fn assert_state(
        record: &EscrowRecord,
        expected_current: EscrowStatus,
        _next: EscrowStatus,
    ) -> Result<(), Error> {
        if record.current_state != expected_current {
            return Err(Error::InvalidStateTransition);
        }
        Ok(())
    }

    fn assert_not_disputed(record: &EscrowRecord) -> Result<(), Error> {
        if record.current_state == EscrowStatus::Disputed {
            return Err(Error::DisputeActive);
        }
        Ok(())
    }

    fn is_admin(env: &Env, address: &Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
            .map(|admin| admin == *address)
            .unwrap_or(false)
    }

    /// If a yield vault is configured, forward `amount` of `asset` (already
    /// sitting in this contract's own balance) into it. The vault's own
    /// circuit breaker handles the case where the external yield protocol is
    /// unavailable; a failure here only means the vault contract itself
    /// (misconfiguration, etc.) is unreachable, which is surfaced as
    /// `Error::VaultCallFailed` rather than silently swallowed.
    fn vault_deposit_if_configured(
        env: &Env,
        asset: &Address,
        amount: i128,
        transaction_id: &String,
    ) -> Result<(), Error> {
        if let Some(vault_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::VaultContract)
        {
            let contract_addr = env.current_contract_address();
            token::Client::new(env, asset).transfer(&contract_addr, &vault_addr, &amount);
            VaultClient::new(env, &vault_addr)
                .try_deposit(&contract_addr, asset, &amount, transaction_id)
                .map_err(|_| Error::VaultCallFailed)?
                .map_err(|_| Error::VaultCallFailed)?;
        }
        Ok(())
    }

    /// If a yield vault is configured, pull this escrow's principal (plus
    /// any earned yield, which the vault routes straight to the treasury)
    /// back into this contract's own balance so the existing seller/buyer
    /// transfer logic can run unchanged.
    fn vault_withdraw_if_configured(env: &Env, transaction_id: &String) -> Result<(), Error> {
        if let Some(vault_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::VaultContract)
        {
            let contract_addr = env.current_contract_address();
            VaultClient::new(env, &vault_addr)
                .try_withdraw(&contract_addr, transaction_id)
                .map_err(|_| Error::VaultCallFailed)?
                .map_err(|_| Error::VaultCallFailed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
