#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, token};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Escrow(String), // listing_id maps to EscrowState
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowState {
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128,
    pub status: EscrowStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Locked,
    Released,
    Refunded,
    Disputed,
}

#[contract]
pub struct VeloxousEscrow;

#[contractimpl]
impl VeloxousEscrow {
    /// Initialize the contract with an admin address
    pub fn init(env: Env, admin: Address) {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Buyer locks funds in escrow for a specific listing
    pub fn deposit(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        listing_id: String,
    ) {
        buyer.require_auth();

        if amount <= 0 {
            panic!("amount must be positive");
        }

        let key = DataKey::Escrow(listing_id.clone());
        if env.storage().persistent().has(&key) {
            panic!("escrow already exists for this listing");
        }

        // Transfer tokens from buyer to contract
        let client = token::Client::new(&env, &token);
        client.transfer(&buyer, &env.current_contract_address(), &amount);

        // Save escrow state
        let state = EscrowState {
            buyer,
            seller,
            token,
            amount,
            status: EscrowStatus::Locked,
        };
        env.storage().persistent().set(&key, &state);
    }

    /// Buyer confirms receipt of item; funds released to seller
    pub fn release(env: Env, listing_id: String) {
        let key = DataKey::Escrow(listing_id);
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        state.buyer.require_auth();

        // Transfer funds to seller
        let client = token::Client::new(&env, &state.token);
        client.transfer(&env.current_contract_address(), &state.seller, &state.amount);

        state.status = EscrowStatus::Released;
        env.storage().persistent().set(&key, &state);
    }

    /// Buyer flags the transaction as disputed, halting release/refund until admin resolves
    pub fn dispute(env: Env, listing_id: String) {
        let key = DataKey::Escrow(listing_id);
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        state.buyer.require_auth();

        state.status = EscrowStatus::Disputed;
        env.storage().persistent().set(&key, &state);
    }

    /// Admin or dispute resolution refunds the buyer
    pub fn refund(env: Env, listing_id: String) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let key = DataKey::Escrow(listing_id);
        let mut state: EscrowState = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"));

        if state.status != EscrowStatus::Locked {
            panic!("escrow not locked");
        }

        // Refund buyer
        let client = token::Client::new(&env, &state.token);
        client.transfer(&env.current_contract_address(), &state.buyer, &state.amount);

        state.status = EscrowStatus::Refunded;
        env.storage().persistent().set(&key, &state);
    }

    /// Read the current state of an escrow
    pub fn get_escrow(env: Env, listing_id: String) -> EscrowState {
        let key = DataKey::Escrow(listing_id);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("escrow not found"))
    }
}
