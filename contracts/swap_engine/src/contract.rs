use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol,
};
use interfaces::oracle::OracleClient;
use crate::types::{SwapRecord, SwapState};

const ADMIN: Symbol = symbol_short!("ADMIN");
const USDC: Symbol = symbol_short!("USDC");
const ORACLE: Symbol = symbol_short!("ORACLE");
const TIMEOUT_SECONDS: u64 = 48 * 60 * 60; // 48 hours

#[contracttype]
pub enum DataKey {
    Admin,
    USDC,
    Oracle,
    Swap(u64),
    SwapCounter,
}

#[contract]
pub struct SwapEngine;

#[contractimpl]
impl SwapEngine {
    pub fn initialize(env: Env, deployer: Address, admin: Address, usdc: Address, oracle: Address) {
        deployer.require_auth();
        assert!(!env.storage().instance().has(&DataKey::Admin), "Already initialized");
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::USDC, &usdc);
        env.storage().instance().set(&DataKey::Oracle, &oracle);
        env.storage().instance().set(&DataKey::SwapCounter, &0u64);
    }

    pub fn propose_swap(
        env: Env,
        party_a: Address,
        party_b: Address,
        device_a: Address,
        device_b: Address,
    ) -> u64 {
        party_a.require_auth();

        let oracle_addr: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
        let oracle = OracleClient::new(&env, &oracle_addr);

        let price_a = oracle.get_price(&device_a);
        let price_b = oracle.get_price(&device_b);
        assert!(price_a > 0, "Price A must be positive");
        assert!(price_b > 0, "Price B must be positive");

        let swap_id: u64 = env.storage().instance().get(&DataKey::SwapCounter).unwrap();
        env.storage().instance().set(&DataKey::SwapCounter, &(swap_id + 1));

        let now = env.ledger().timestamp();

        let record = SwapRecord {
            swap_id,
            party_a,
            party_b,
            device_a,
            device_b,
            collateral_a_amount: price_b,
            collateral_b_amount: price_a,
            state: SwapState::Proposed,
            proposed_at: now,
            a_funded_at: 0,
            b_funded_at: 0,
        };

        env.storage().persistent().set(&DataKey::Swap(swap_id), &record);
        swap_id
    }

    pub fn deposit_collateral(env: Env, swap_id: u64, party: Address) {
        party.require_auth();
        let mut record: SwapRecord = env.storage().persistent().get(&DataKey::Swap(swap_id)).unwrap();
        
        assert!(
            record.state == SwapState::Proposed || record.state == SwapState::AFunded || record.state == SwapState::BFunded,
            "Invalid state for deposit"
        );

        let now = env.ledger().timestamp();
        let usdc_addr: Address = env.storage().instance().get(&DataKey::USDC).unwrap();
        let usdc = token::Client::new(&env, &usdc_addr);

        if party == record.party_a {
            assert!(record.state == SwapState::Proposed || record.state == SwapState::BFunded, "Already funded by A");
            
            // Check oracle price variance
            let oracle_addr: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
            let oracle = OracleClient::new(&env, &oracle_addr);
            let current_price_b = oracle.get_price(&record.device_b);
            let diff = (current_price_b - record.collateral_a_amount).abs();
            // 5% slippage
            assert!(diff * 100 <= record.collateral_a_amount * 5, "Price variance exceeded");

            usdc.transfer(&party, &env.current_contract_address(), &record.collateral_a_amount);
            record.a_funded_at = now;
            
            if record.state == SwapState::BFunded {
                record.state = SwapState::FullyFunded;
            } else {
                record.state = SwapState::AFunded;
            }
        } else if party == record.party_b {
            assert!(record.state == SwapState::Proposed || record.state == SwapState::AFunded, "Already funded by B");

            // Check oracle price variance
            let oracle_addr: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
            let oracle = OracleClient::new(&env, &oracle_addr);
            let current_price_a = oracle.get_price(&record.device_a);
            let diff = (current_price_a - record.collateral_b_amount).abs();
            // 5% slippage
            assert!(diff * 100 <= record.collateral_b_amount * 5, "Price variance exceeded");

            usdc.transfer(&party, &env.current_contract_address(), &record.collateral_b_amount);
            record.b_funded_at = now;

            if record.state == SwapState::AFunded {
                record.state = SwapState::FullyFunded;
            } else {
                record.state = SwapState::BFunded;
            }
        } else {
            panic!("Unauthorized");
        }

        env.storage().persistent().set(&DataKey::Swap(swap_id), &record);
    }

    pub fn withdraw_timeout(env: Env, swap_id: u64, party: Address) {
        party.require_auth();
        let mut record: SwapRecord = env.storage().persistent().get(&DataKey::Swap(swap_id)).unwrap();
        let now = env.ledger().timestamp();
        
        let usdc_addr: Address = env.storage().instance().get(&DataKey::USDC).unwrap();
        let usdc = token::Client::new(&env, &usdc_addr);

        if party == record.party_a {
            assert!(record.state == SwapState::AFunded, "Cannot withdraw");
            assert!(now >= record.a_funded_at + TIMEOUT_SECONDS, "Timeout not reached");
            
            record.state = SwapState::Completed;
            env.storage().persistent().set(&DataKey::Swap(swap_id), &record);
            
            usdc.transfer(&env.current_contract_address(), &party, &record.collateral_a_amount);
        } else if party == record.party_b {
            assert!(record.state == SwapState::BFunded, "Cannot withdraw");
            assert!(now >= record.b_funded_at + TIMEOUT_SECONDS, "Timeout not reached");

            record.state = SwapState::Completed;
            env.storage().persistent().set(&DataKey::Swap(swap_id), &record);

            usdc.transfer(&env.current_contract_address(), &party, &record.collateral_b_amount);
        } else {
            panic!("Unauthorized");
        }
    }

    pub fn confirm_receipt(env: Env, swap_id: u64, party: Address) {
        party.require_auth();
        let mut record: SwapRecord = env.storage().persistent().get(&DataKey::Swap(swap_id)).unwrap();
        
        assert!(
            record.state == SwapState::FullyFunded || record.state == SwapState::AConfirmed || record.state == SwapState::BConfirmed,
            "Invalid state for confirmation"
        );

        if party == record.party_a {
            assert!(record.state != SwapState::AConfirmed, "Already confirmed by A");
            if record.state == SwapState::BConfirmed {
                record.state = SwapState::Completed;
            } else {
                record.state = SwapState::AConfirmed;
            }
        } else if party == record.party_b {
            assert!(record.state != SwapState::BConfirmed, "Already confirmed by B");
            if record.state == SwapState::AConfirmed {
                record.state = SwapState::Completed;
            } else {
                record.state = SwapState::BConfirmed;
            }
        } else {
            panic!("Unauthorized");
        }

        let is_completed = record.state == SwapState::Completed;
        env.storage().persistent().set(&DataKey::Swap(swap_id), &record);

        if is_completed {
            let usdc_addr: Address = env.storage().instance().get(&DataKey::USDC).unwrap();
            let usdc = token::Client::new(&env, &usdc_addr);
            
            // Swap collateral returned: A gets A's collateral back, B gets B's collateral back
            usdc.transfer(&env.current_contract_address(), &record.party_a, &record.collateral_a_amount);
            usdc.transfer(&env.current_contract_address(), &record.party_b, &record.collateral_b_amount);
        }
    }

    pub fn raise_dispute(env: Env, swap_id: u64, party: Address) {
        party.require_auth();
        let mut record: SwapRecord = env.storage().persistent().get(&DataKey::Swap(swap_id)).unwrap();
        
        assert!(
            record.state == SwapState::FullyFunded || record.state == SwapState::AConfirmed || record.state == SwapState::BConfirmed,
            "Invalid state for dispute"
        );
        assert!(party == record.party_a || party == record.party_b, "Unauthorized");

        record.state = SwapState::Disputed;
        env.storage().persistent().set(&DataKey::Swap(swap_id), &record);
    }

    pub fn resolve_swap_dispute(env: Env, swap_id: u64, winner: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let mut record: SwapRecord = env.storage().persistent().get(&DataKey::Swap(swap_id)).unwrap();
        assert!(record.state == SwapState::Disputed, "Not disputed");

        record.state = SwapState::Completed;
        env.storage().persistent().set(&DataKey::Swap(swap_id), &record);

        let usdc_addr: Address = env.storage().instance().get(&DataKey::USDC).unwrap();
        let usdc = token::Client::new(&env, &usdc_addr);

        let total_collateral = record.collateral_a_amount + record.collateral_b_amount;
        usdc.transfer(&env.current_contract_address(), &winner, &total_collateral);
    }
}
