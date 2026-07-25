#![cfg(test)]

use reputation::{ReputationContract, ReputationContractClient, Tier};
use soroban_sdk::{testutils::Address as _, Address, Env, contract, contractimpl};

// A dummy Escrow contract to test the interaction
#[contract]
pub struct DummyEscrowContract;

#[contractimpl]
impl DummyEscrowContract {
    pub fn complete_escrow(env: Env, reputation_contract: Address, user: Address, tx_value: i128) {
        let rep_client = ReputationContractClient::new(&env, &reputation_contract);
        let min_value = 100;
        
        rep_client.update_score(
            &env.current_contract_address(),
            &user,
            &true, // success
            &false, // dispute_lost
            &tx_value,
            &min_value,
        );
    }
}

#[test]
fn test_integration_with_escrow() {
    let env = Env::default();
    env.mock_all_auths();
    
    // 1. Deploy Reputation contract
    let rep_contract_id = env.register_contract(None, ReputationContract);
    let rep_client = ReputationContractClient::new(&env, &rep_contract_id);
    
    let admin = Address::generate(&env);
    rep_client.init(&admin);
    
    // 2. Deploy Dummy Escrow contract
    let escrow_contract_id = env.register_contract(None, DummyEscrowContract);
    let escrow_client = DummyEscrowContractClient::new(&env, &escrow_contract_id);
    
    // 3. Whitelist the Escrow contract
    rep_client.add_authorized_contract(&admin, &escrow_contract_id);
    
    // 4. Mint SBT to user
    let user = Address::generate(&env);
    rep_client.mint(&admin, &user);
    
    // 5. Complete a dummy escrow
    escrow_client.complete_escrow(&rep_contract_id, &user, &500);
    
    // 6. Verify Reputation state changes
    let score = rep_client.get_trust_score(&user);
    
    assert_eq!(score.total_transactions, 1);
    assert_eq!(score.successful_transactions, 1);
    assert_eq!(score.disputes_lost, 0);
    assert_eq!(score.score, 1);
    assert_eq!(score.score_tier, Tier::Unverified);
}
