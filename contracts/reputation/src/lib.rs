#![no_std]

use shared::auth;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Env, String
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    TokenNonTransferable = 1,
    InvalidMinimumValue = 5,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    Unverified = 0,
    Trusted = 1,
    Elite = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustScore {
    pub total_transactions: u32,
    pub successful_transactions: u32,
    pub disputes_raised: u32,
    pub disputes_lost: u32,
    pub score: i32,
    pub score_tier: Tier,
}

#[contracttype]
pub enum DataKey {
    Score(Address),
    Balance(Address),
}

// 30 days assuming ~5s per ledger
const DAY_IN_LEDGERS: u32 = 17280;
const BUMP_LEDGERS: u32 = 30 * DAY_IN_LEDGERS;
const BUMP_THRESHOLD: u32 = 15 * DAY_IN_LEDGERS;

fn bump_instance(env: &Env) {
    env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_LEDGERS);
}

fn bump_score(env: &Env, user: &Address) {
    env.storage().persistent().extend_ttl(
        &DataKey::Score(user.clone()),
        BUMP_THRESHOLD,
        BUMP_LEDGERS,
    );
    env.storage().persistent().extend_ttl(
        &DataKey::Balance(user.clone()),
        BUMP_THRESHOLD,
        BUMP_LEDGERS,
    );
}

#[contract]
pub struct ReputationContract;

#[contractimpl]
impl ReputationContract {
    pub fn init(env: Env, admin: Address) {
        auth::init_admin(&env, &admin);
        bump_instance(&env);
    }

    pub fn add_authorized_contract(env: Env, admin: Address, contract: Address) {
        auth::add_authorized_contract(&env, &admin, &contract);
        bump_instance(&env);
    }

    pub fn remove_authorized_contract(env: Env, admin: Address, contract: Address) {
        auth::remove_authorized_contract(&env, &admin, &contract);
        bump_instance(&env);
    }

    // --- SBT Logic ---

    pub fn mint(env: Env, caller: Address, user: Address) {
        caller.require_auth();
        bump_instance(&env);

        let admin = auth::get_admin(&env);
        let is_auth = auth::is_authorized_contract(&env, &caller);

        if caller != admin && !is_auth {
            panic_with_error!(&env, auth::AuthError::Unauthorized);
        }

        // Only mint if not already minted
        if !env.storage().persistent().has(&DataKey::Balance(user.clone())) {
            env.storage().persistent().set(&DataKey::Balance(user.clone()), &1_i128);
            env.storage().persistent().set(&DataKey::Score(user.clone()), &TrustScore {
                total_transactions: 0,
                successful_transactions: 0,
                disputes_raised: 0,
                disputes_lost: 0,
                score: 0,
                score_tier: Tier::Unverified,
            });
            bump_score(&env, &user);
        }
    }

    // --- Dynamic Trust Scoring ---

    pub fn update_score(
        env: Env,
        caller: Address,
        user: Address,
        success: bool,
        dispute_lost: bool,
        tx_value: i128,
        min_value: i128,
    ) {
        bump_instance(&env);

        auth::require_authorized_contract(&env, &caller);

        if tx_value < min_value {
            // Ignore transaction for score if below minimum value to prevent sybil farming
            return;
        }

        let mut score_data: TrustScore = env
            .storage()
            .persistent()
            .get(&DataKey::Score(user.clone()))
            .unwrap_or(TrustScore {
                total_transactions: 0,
                successful_transactions: 0,
                disputes_raised: 0,
                disputes_lost: 0,
                score: 0,
                score_tier: Tier::Unverified,
            });

        score_data.total_transactions += 1;

        if success {
            score_data.successful_transactions += 1;
            score_data.score += 1;
        } else if dispute_lost {
            score_data.disputes_lost += 1;
            score_data.score -= 50;
        }

        // Determine tier based on score
        if score_data.score >= 101 {
            score_data.score_tier = Tier::Elite;
        } else if score_data.score >= 11 {
            score_data.score_tier = Tier::Trusted;
        } else {
            score_data.score_tier = Tier::Unverified;
        }

        env.storage().persistent().set(&DataKey::Score(user.clone()), &score_data);
        bump_score(&env, &user);
    }

    pub fn get_user_tier(env: Env, user: Address) -> Tier {
        let score_data: TrustScore = env
            .storage()
            .persistent()
            .get(&DataKey::Score(user))
            .unwrap_or(TrustScore {
                total_transactions: 0,
                successful_transactions: 0,
                disputes_raised: 0,
                disputes_lost: 0,
                score: 0,
                score_tier: Tier::Unverified,
            });
        score_data.score_tier
    }

    pub fn get_trust_score(env: Env, user: Address) -> TrustScore {
        env.storage()
            .persistent()
            .get(&DataKey::Score(user))
            .unwrap_or(TrustScore {
                total_transactions: 0,
                successful_transactions: 0,
                disputes_raised: 0,
                disputes_lost: 0,
                score: 0,
                score_tier: Tier::Unverified,
            })
    }

    // --- Token Standard Interface Exceptions (SEP-41) ---

    pub fn allowance(_env: Env, _from: Address, _spender: Address) -> i128 {
        0
    }

    pub fn approve(env: Env, _from: Address, _spender: Address, _amount: i128, _expiration_ledger: u32) {
        panic_with_error!(&env, Error::TokenNonTransferable);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Balance(id)).unwrap_or(0)
    }

    pub fn transfer(env: Env, _from: Address, _to: Address, _amount: i128) {
        panic_with_error!(&env, Error::TokenNonTransferable);
    }

    pub fn transfer_from(env: Env, _spender: Address, _from: Address, _to: Address, _amount: i128) {
        panic_with_error!(&env, Error::TokenNonTransferable);
    }

    pub fn burn(env: Env, _from: Address, _amount: i128) {
        panic_with_error!(&env, Error::TokenNonTransferable);
    }

    pub fn burn_from(env: Env, _spender: Address, _from: Address, _amount: i128) {
        panic_with_error!(&env, Error::TokenNonTransferable);
    }

    pub fn decimals(_env: Env) -> u32 {
        0
    }

    pub fn name(env: Env) -> String {
        String::from_str(&env, "Veloxous Reputation")
    }

    pub fn symbol(env: Env) -> String {
        String::from_str(&env, "VREP")
    }
}

#[cfg(test)]
mod test;
