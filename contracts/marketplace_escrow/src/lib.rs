#![no_std]

pub mod events;
pub mod reputation;
pub mod types;

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, String, Vec};
use types::*;

#[contract]
pub struct MarketplaceEscrow;

#[contractimpl]
impl MarketplaceEscrow {
    /// Initialize the marketplace escrow contract with admins, threshold, and optional reputation contract address.
    ///
    /// # Arguments
    /// * `admins` - Vector of 5 initial admin addresses (or N >= threshold).
    /// * `threshold` - Multisig voting threshold M (default 3). Must be <= N and > 0.
    /// * `reputation_contract` - Optional address of an off-chain/on-chain reputation contract.
    pub fn init(
        env: Env,
        admins: Vec<Address>,
        threshold: u32,
        reputation_contract: Option<Address>,
    ) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("already initialized");
        }

        let n = admins.len();
        if n == 0 {
            panic!("admins list cannot be empty");
        }
        if threshold == 0 || threshold > n {
            panic!("invalid threshold: M must be <= N and > 0");
        }

        // Check duplicate admins
        for i in 0..n {
            for j in (i + 1)..n {
                if admins.get(i).unwrap() == admins.get(j).unwrap() {
                    panic!("duplicate admin detected");
                }
            }
        }

        env.storage().instance().set(&DataKey::Admins, &admins);
        env.storage()
            .instance()
            .set(&DataKey::Threshold, &threshold);

        if let Some(rep) = reputation_contract {
            env.storage()
                .instance()
                .set(&DataKey::ReputationContract, &rep);
        }

        env.storage().instance().set(&DataKey::Initialized, &true);
    }

    /// Buyer locks funds in escrow for a transaction.
    pub fn deposit(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        transaction_id: String,
    ) {
        buyer.require_auth();

        if amount <= 0 {
            panic!("amount must be positive");
        }

        let key = DataKey::Escrow(transaction_id.clone());
        if env.storage().persistent().has(&key) {
            panic!("escrow already exists for this transaction");
        }

        let contract_addr = env.current_contract_address();
        let client = token::Client::new(&env, &token);
        client.transfer(&buyer, &contract_addr, &amount);

        let state = EscrowState {
            buyer,
            seller,
            token,
            amount,
            status: EscrowStatus::Locked,
            created_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&key, &state);
    }

    /// Buyer confirms receipt or completion; funds released to seller.
    /// Blocked if escrow is in Disputed status.
    pub fn release(env: Env, transaction_id: String) {
        let key = DataKey::Escrow(transaction_id);
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status == EscrowStatus::Disputed {
            panic!("escrow is disputed");
        }
        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        state.buyer.require_auth();

        let contract_addr = env.current_contract_address();
        let client = token::Client::new(&env, &state.token);
        client.transfer(&contract_addr, &state.seller, &state.amount);

        state.status = EscrowStatus::Released;
        env.storage().persistent().set(&key, &state);
    }

    /// Withdraw funds prior to lock/fulfillment if allowable.
    /// Blocked if escrow is in Disputed status.
    pub fn withdraw(env: Env, transaction_id: String) {
        let key = DataKey::Escrow(transaction_id);
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status == EscrowStatus::Disputed {
            panic!("escrow is disputed");
        }
        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        state.buyer.require_auth();

        let contract_addr = env.current_contract_address();
        let client = token::Client::new(&env, &state.token);
        client.transfer(&contract_addr, &state.buyer, &state.amount);

        state.status = EscrowStatus::Refunded;
        env.storage().persistent().set(&key, &state);
    }

    /// Auto release funds after timeout. Blocked if escrow is in Disputed status.
    pub fn auto_release(env: Env, transaction_id: String) {
        let key = DataKey::Escrow(transaction_id);
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status == EscrowStatus::Disputed {
            panic!("escrow is disputed");
        }
        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        let contract_addr = env.current_contract_address();
        let client = token::Client::new(&env, &state.token);
        client.transfer(&contract_addr, &state.seller, &state.amount);

        state.status = EscrowStatus::Released;
        env.storage().persistent().set(&key, &state);
    }

    /// Auto refund funds after timeout. Blocked if escrow is in Disputed status.
    pub fn auto_refund(env: Env, transaction_id: String) {
        let key = DataKey::Escrow(transaction_id);
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status == EscrowStatus::Disputed {
            panic!("escrow is disputed");
        }
        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        let contract_addr = env.current_contract_address();
        let client = token::Client::new(&env, &state.token);
        client.transfer(&contract_addr, &state.buyer, &state.amount);

        state.status = EscrowStatus::Refunded;
        env.storage().persistent().set(&key, &state);
    }

    /// Approve milestone completion. Blocked if escrow is in Disputed status.
    pub fn approve_milestone(env: Env, transaction_id: String) {
        let key = DataKey::Escrow(transaction_id);
        let state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status == EscrowStatus::Disputed {
            panic!("escrow is disputed");
        }
        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        state.buyer.require_auth();
    }

    /// Complete milestone. Blocked if escrow is in Disputed status.
    pub fn complete_milestone(env: Env, transaction_id: String) {
        let key = DataKey::Escrow(transaction_id);
        let state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status == EscrowStatus::Disputed {
            panic!("escrow is disputed");
        }
        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        state.seller.require_auth();
    }

    /// Either buyer or seller raises a dispute for a locked escrow.
    /// Halts all normal execution paths and stores dispute metadata.
    pub fn raise_dispute(
        env: Env,
        caller: Address,
        transaction_id: String,
        reason_hash: BytesN<32>,
    ) {
        caller.require_auth();

        let escrow_key = DataKey::Escrow(transaction_id.clone());
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&escrow_key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        // Only buyer or seller may call raise_dispute
        if caller != state.buyer && caller != state.seller {
            panic!("unauthorized caller");
        }

        state.status = EscrowStatus::Disputed;
        env.storage().persistent().set(&escrow_key, &state);

        let timestamp = env.ledger().timestamp();
        let dispute = Dispute {
            transaction_id: transaction_id.clone(),
            buyer: state.buyer.clone(),
            seller: state.seller.clone(),
            reason_hash: reason_hash.clone(),
            timestamp,
            raised_by: caller,
        };

        let dispute_key = DataKey::Dispute(transaction_id.clone());
        env.storage().persistent().set(&dispute_key, &dispute);

        events::emit_dispute_raised(
            &env,
            transaction_id,
            state.buyer,
            state.seller,
            reason_hash,
            timestamp,
        );
    }

    /// Admin proposes a resolution split for a disputed transaction.
    /// Store proposal inside temporary storage. Identical proposals accumulate votes.
    pub fn propose_resolution(
        env: Env,
        admin: Address,
        transaction_id: String,
        buyer_refund_amount: i128,
        seller_payout_amount: i128,
    ) {
        admin.require_auth();
        Self::ensure_admin(&env, &admin);

        let escrow_key = DataKey::Escrow(transaction_id.clone());
        let state: EscrowState = env
            .storage()
            .persistent()
            .get(&escrow_key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status != EscrowStatus::Disputed {
            panic!("escrow not disputed");
        }

        if buyer_refund_amount < 0 || seller_payout_amount < 0 {
            panic!("payout amounts must be non-negative");
        }

        let prop_key = DataKey::Proposal(
            transaction_id.clone(),
            buyer_refund_amount,
            seller_payout_amount,
        );

        let mut proposal =
            env.storage()
                .temporary()
                .get(&prop_key)
                .unwrap_or_else(|| ProposalState {
                    votes: Vec::new(&env),
                    created_at: env.ledger().timestamp(),
                });

        if proposal.votes.contains(&admin) {
            panic!("duplicate vote");
        }

        proposal.votes.push_back(admin.clone());
        env.storage().temporary().set(&prop_key, &proposal);

        events::emit_resolution_proposed(
            &env,
            transaction_id.clone(),
            admin.clone(),
            buyer_refund_amount,
            seller_payout_amount,
        );

        events::emit_resolution_vote_cast(
            &env,
            transaction_id.clone(),
            admin,
            buyer_refund_amount,
            seller_payout_amount,
            proposal.votes.len(),
        );

        let threshold: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Threshold)
            .unwrap_or(0);

        if proposal.votes.len() >= threshold {
            Self::execute_resolution(
                &env,
                transaction_id,
                buyer_refund_amount,
                seller_payout_amount,
                state,
                prop_key,
            );
        }
    }

    /// Admin votes for an existing or new resolution proposal.
    /// Identical proposal values accumulate votes. When threshold is reached, execution triggers.
    pub fn vote_resolution(
        env: Env,
        admin: Address,
        transaction_id: String,
        buyer_refund_amount: i128,
        seller_payout_amount: i128,
    ) {
        Self::propose_resolution(
            env,
            admin,
            transaction_id,
            buyer_refund_amount,
            seller_payout_amount,
        );
    }

    /// Internal execution helper when multisig threshold is reached for a resolution.
    fn execute_resolution(
        env: &Env,
        transaction_id: String,
        buyer_refund_amount: i128,
        seller_payout_amount: i128,
        mut state: EscrowState,
        prop_key: DataKey,
    ) {
        // Validate total payout matches total locked
        if buyer_refund_amount + seller_payout_amount != state.amount {
            panic!("invalid resolution amounts");
        }

        let contract_addr = env.current_contract_address();
        let token_client = token::Client::new(env, &state.token);

        if buyer_refund_amount > 0 {
            token_client.transfer(&contract_addr, &state.buyer, &buyer_refund_amount);
        }

        if seller_payout_amount > 0 {
            token_client.transfer(&contract_addr, &state.seller, &seller_payout_amount);
        }

        state.status = EscrowStatus::Resolved;
        let escrow_key = DataKey::Escrow(transaction_id.clone());
        env.storage().persistent().set(&escrow_key, &state);

        // Delete temporary proposal
        env.storage().temporary().remove(&prop_key);

        let timestamp = env.ledger().timestamp();
        events::emit_resolution_executed(
            env,
            transaction_id.clone(),
            buyer_refund_amount,
            seller_payout_amount,
            timestamp,
        );

        // Notify reputation contract if configured
        if let Some(rep_contract) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ReputationContract)
        {
            if buyer_refund_amount > seller_payout_amount {
                reputation::notify_reputation(
                    env,
                    &rep_contract,
                    &state.buyer,
                    &state.seller,
                    &transaction_id,
                );
            } else if seller_payout_amount > buyer_refund_amount {
                reputation::notify_reputation(
                    env,
                    &rep_contract,
                    &state.seller,
                    &state.buyer,
                    &transaction_id,
                );
            }
        }
    }

    /// Admin proposes/votes for replacing an existing admin with a new admin.
    pub fn propose_admin_change(
        env: Env,
        proposer: Address,
        old_admin: Address,
        new_admin: Address,
    ) {
        proposer.require_auth();
        Self::ensure_admin(&env, &proposer);

        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Admins)
            .unwrap_or_else(|| panic!("not initialized"));
        let threshold: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Threshold)
            .unwrap_or_else(|| panic!("not initialized"));

        if !admins.contains(&old_admin) {
            panic!("old admin not found");
        }
        if admins.contains(&new_admin) {
            panic!("new admin already exists");
        }

        let key = DataKey::AdminChangeProposal(old_admin.clone(), new_admin.clone());
        let mut voters: Vec<Address> = env
            .storage()
            .temporary()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        if voters.contains(&proposer) {
            panic!("duplicate vote");
        }

        voters.push_back(proposer.clone());
        env.storage().temporary().set(&key, &voters);

        events::emit_admin_change_proposed(&env, old_admin.clone(), new_admin.clone(), proposer);

        if voters.len() >= threshold {
            let mut updated_admins = Vec::new(&env);
            for a in admins.iter() {
                if a == old_admin {
                    updated_admins.push_back(new_admin.clone());
                } else {
                    updated_admins.push_back(a);
                }
            }

            if threshold > updated_admins.len() {
                panic!("threshold invalid for updated admins");
            }

            env.storage()
                .instance()
                .set(&DataKey::Admins, &updated_admins);
            env.storage().temporary().remove(&key);

            events::emit_admin_changed(&env, old_admin, new_admin, env.ledger().timestamp());
        }
    }

    /// Helper check for admin membership
    fn ensure_admin(env: &Env, address: &Address) {
        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Admins)
            .unwrap_or_else(|| panic!("not initialized"));
        if !admins.contains(address) {
            panic!("only admins may vote");
        }
    }

    /// Read escrow state from persistent storage
    pub fn get_escrow(env: Env, transaction_id: String) -> EscrowState {
        let key = DataKey::Escrow(transaction_id);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"))
    }

    /// Read dispute metadata from persistent storage
    pub fn get_dispute(env: Env, transaction_id: String) -> Dispute {
        let key = DataKey::Dispute(transaction_id);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("dispute not found"))
    }

    /// Get current list of admin addresses
    pub fn get_admins(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Admins)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get current threshold M
    pub fn get_threshold(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::Threshold)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
