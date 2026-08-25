#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token, Address, Env, String, Vec,
};

// ============================================================================
// Mock Contracts
// ============================================================================

#[contract]
pub struct MockEscrowContract;

#[contractimpl]
impl MockEscrowContract {
    pub fn create_arbitration_case(
        env: Env,
        arbitration_contract: Address,
        transaction_id: String,
        buyer: Address,
        seller: Address,
        amount: i128,
        token: Address,
    ) -> String {
        let client = ArbitrationContractClient::new(&env, &arbitration_contract);
        client.create_case(&env.current_contract_address(), &transaction_id, &buyer, &seller, &amount, &token)
    }
}

// ============================================================================
// Test Setup Helpers
// ============================================================================

fn create_token_contract<'a>(
    env: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    (
        token::Client::new(env, &sac.address()),
        token::StellarAssetClient::new(env, &sac.address()),
    )
}

fn setup_test<'a>(
    env: &'a Env,
) -> (
    ArbitrationContractClient<'a>,
    Address,                    // admin
    token::Client<'a>,          // USDC token client
    token::StellarAssetClient<'a>, // USDC admin client
    Address,                    // escrow contract
) {
    env.mock_all_auths();

    // Register contracts
    let arbitration_id = env.register(ArbitrationContract, ());
    let arbitration_client = ArbitrationContractClient::new(env, &arbitration_id);

    let escrow_id = env.register(MockEscrowContract, ());

    // Create USDC token
    let token_admin = Address::generate(env);
    let (token_client, token_admin_client) = create_token_contract(env, &token_admin);

    // Initialize arbitration contract
    let admin = Address::generate(env);
    arbitration_client.init(
        &admin,
        &token_client.address.clone(),
        &escrow_id,
        &Some(100_000_000i128), // 100 USDC min stake
        &Some(604800u64),       // 7 days lockup
    );

    (
        arbitration_client,
        admin,
        token_client,
        token_admin_client,
        escrow_id,
    )
}

/// Helper: advance ledger timestamp by `delta_secs` seconds.
fn advance_time(env: &Env, delta_secs: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp.saturating_add(delta_secs);
    });
}

// ============================================================================
// Juror Staking Tests
// ============================================================================

#[test]
fn test_stake_as_juror_success() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, _escrow) = setup_test(&env);

    // Create a potential juror and fund them
    let juror1 = Address::generate(&env);
    token_admin.mint(&juror1, &500_000_000); // 500 USDC

    // Stake
    arbitration_client.stake_as_juror(&juror1, &200_000_000); // 200 USDC

    // Verify balance
    assert_eq!(token_client.balance(&juror1), 300_000_000);
    assert_eq!(token_client.balance(&arbitration_client.address), 200_000_000);

    // Verify juror record
    let record = arbitration_client.get_juror_record(&juror1);
    assert!(record.is_some());
    let record = record.unwrap();
    assert_eq!(record.staked_amount, 200_000_000);
    assert_eq!(record.cases_participated, 0);
    assert_eq!(record.cases_won, 0);

    // Verify juror pool
    let pool = arbitration_client.get_juror_pool();
    assert_eq!(pool.len(), 1);
    assert!(pool.contains(&juror1));
}

#[test]
fn test_stake_multiple_jurors() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, _escrow) = setup_test(&env);

    // Create 5 jurors
    let mut jurors = Vec::new(&env);
    for i in 0..5 {
        let juror = Address::generate(&env);
        token_admin.mint(&juror, &300_000_000);
        arbitration_client.stake_as_juror(&juror, &150_000_000);
        jurors.push_back(juror);
    }

    // Verify all are in pool
    let pool = arbitration_client.get_juror_pool();
    assert_eq!(pool.len(), 5);

    // Verify each juror's record
    for juror in jurors.iter() {
        let record = arbitration_client.get_juror_record(&juror).unwrap();
        assert_eq!(record.staked_amount, 150_000_000);
    }
}

#[test]
#[should_panic(expected = "stake amount below minimum")]
fn test_stake_below_minimum_panics() {
    let env = Env::default();
    let (arbitration_client, _admin, _token_client, token_admin, _escrow) = setup_test(&env);

    let juror = Address::generate(&env);
    token_admin.mint(&juror, &200_000_000);

    // Try to stake less than minimum (100 USDC)
    arbitration_client.stake_as_juror(&juror, &50_000_000); // 50 USDC
}

#[test]
fn test_unstake_after_lockup_period() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, _escrow) = setup_test(&env);

    let juror = Address::generate(&env);
    token_admin.mint(&juror, &300_000_000);
    arbitration_client.stake_as_juror(&juror, &200_000_000);

    // Advance time past lockup period (7 days)
    advance_time(&env, 7 * 24 * 60 * 60 + 1);

    // Unstake
    arbitration_client.unstake(&juror);

    // Verify tokens returned
    assert_eq!(token_client.balance(&juror), 300_000_000);
    assert_eq!(token_client.balance(&arbitration_client.address), 0);

    // Verify juror removed from pool
    let pool = arbitration_client.get_juror_pool();
    assert_eq!(pool.len(), 0);

    // Verify record removed
    let record = arbitration_client.get_juror_record(&juror);
    assert!(record.is_none());
}

#[test]
#[should_panic(expected = "lockup period not elapsed")]
fn test_unstake_before_lockup_panics() {
    let env = Env::default();
    let (arbitration_client, _admin, _token_client, token_admin, _escrow) = setup_test(&env);

    let juror = Address::generate(&env);
    token_admin.mint(&juror, &300_000_000);
    arbitration_client.stake_as_juror(&juror, &200_000_000);

    // Advance only 3 days (less than 7 day lockup)
    advance_time(&env, 3 * 24 * 60 * 60);

    // Try to unstake - should panic
    arbitration_client.unstake(&juror);
}

// ============================================================================
// Case Assignment & Voting Tests
// ============================================================================

/// Issue requirement: Stake 5 jurors, raise a dispute, and assert exactly 3 are randomly selected.
#[test]
fn test_case_creation_selects_3_jurors_from_pool() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 5 jurors
    let mut jurors = Vec::new(&env);
    for i in 0..5 {
        let juror = Address::generate(&env);
        token_admin.mint(&juror, &300_000_000);
        arbitration_client.stake_as_juror(&juror, &150_000_000);
        jurors.push_back(juror);
    }

    // Verify all 5 are in pool
    let pool = arbitration_client.get_juror_pool();
    assert_eq!(pool.len(), 5);

    // Create arbitration case via mock escrow
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_001");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    // Get the case
    let case = arbitration_client.get_case(&case_id);

    // Assert exactly 3 jurors were selected
    assert_eq!(case.jurors.len(), 3);
    assert_eq!(case.status, CaseStatus::Active);
    assert_eq!(case.buyer, buyer);
    assert_eq!(case.seller, seller);
    assert_eq!(case.amount, 1000);

    // Verify all selected jurors are from the original pool
    for juror in case.jurors.iter() {
        assert!(pool.contains(&juror));
    }
}

/// Issue requirement: Simulate 2 jurors voting for Verdict A and 1 for Verdict B.
/// Assert Verdict A wins and the losing juror receives a reputation penalty.
#[test]
fn test_majority_vote_wins_and_losing_juror_penalty() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Create case
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_002");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    // Get selected jurors
    let case = arbitration_client.get_case(&case_id);
    let selected_jurors = case.jurors;

    // Jurors 1 and 2 vote BuyerWins, Juror 3 votes SellerWins
    arbitration_client.juror_vote(&selected_jurors.get(0).unwrap(), &case_id, &Verdict::BuyerWins);
    arbitration_client.juror_vote(&selected_jurors.get(1).unwrap(), &case_id, &Verdict::BuyerWins);

    // At this point (2 votes for BuyerWins), the case should be resolved
    let resolved_case = arbitration_client.get_case(&case_id);
    assert_eq!(resolved_case.status, CaseStatus::Resolved);
    assert_eq!(resolved_case.final_verdict, Some(Verdict::BuyerWins));

    // Get votes
    let votes = arbitration_client.get_case_votes(&case_id);
    assert_eq!(votes.buyer_wins_votes, 2);
    assert_eq!(votes.seller_wins_votes, 0);

    // Now have the third juror vote (dissenting vote)
    arbitration_client.juror_vote(&selected_jurors.get(2).unwrap(), &case_id, &Verdict::SellerWins);

    // Verify final votes
    let final_votes = arbitration_client.get_case_votes(&case_id);
    assert_eq!(final_votes.buyer_wins_votes, 2);
    assert_eq!(final_votes.seller_wins_votes, 1);

    // Verify juror records were updated
    // Winning jurors (voted BuyerWins) should have cases_participated=1, cases_won=1
    let record1 = arbitration_client.get_juror_record(&selected_jurors.get(0).unwrap()).unwrap();
    let record2 = arbitration_client.get_juror_record(&selected_jurors.get(1).unwrap()).unwrap();
    assert_eq!(record1.cases_participated, 1);
    assert_eq!(record1.cases_won, 1);
    assert_eq!(record2.cases_participated, 1);
    assert_eq!(record2.cases_won, 1);

    // Losing juror (voted SellerWins) should have cases_participated=1, cases_won=0
    let record3 = arbitration_client.get_juror_record(&selected_jurors.get(2).unwrap()).unwrap();
    assert_eq!(record3.cases_participated, 1);
    assert_eq!(record3.cases_won, 0); // Lost the case
}

#[test]
fn test_seller_wins_majority() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Create case
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_003");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    // Get selected jurors
    let case = arbitration_client.get_case(&case_id);
    let selected_jurors = case.jurors;

    // Jurors 1 and 2 vote SellerWins, Juror 3 votes BuyerWins
    arbitration_client.juror_vote(&selected_jurors.get(0).unwrap(), &case_id, &Verdict::SellerWins);
    arbitration_client.juror_vote(&selected_jurors.get(1).unwrap(), &case_id, &Verdict::SellerWins);

    // Case should be resolved with SellerWins
    let resolved_case = arbitration_client.get_case(&case_id);
    assert_eq!(resolved_case.status, CaseStatus::Resolved);
    assert_eq!(resolved_case.final_verdict, Some(Verdict::SellerWins));
}

#[test]
#[should_panic(expected = "not a selected juror for this case")]
fn test_non_selected_juror_cannot_vote() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);
    let outsider = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Create case
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_004");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    // Outsider tries to vote
    arbitration_client.juror_vote(&outsider, &case_id, &Verdict::BuyerWins);
}

#[test]
#[should_panic(expected = "juror has already voted")]
fn test_duplicate_vote_rejected() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Create case
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_005");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    // Get selected jurors
    let case = arbitration_client.get_case(&case_id);
    let juror = case.jurors.get(0).unwrap();

    // First vote succeeds
    arbitration_client.juror_vote(&juror, &case_id, &Verdict::BuyerWins);

    // Duplicate vote should panic
    arbitration_client.juror_vote(&juror, &case_id, &Verdict::SellerWins);
}

#[test]
#[should_panic(expected = "case is not active")]
fn test_cannot_vote_on_resolved_case() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Create case
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_006");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    // Get selected jurors
    let case = arbitration_client.get_case(&case_id);

    // Vote to resolve the case (2 votes for majority)
    arbitration_client.juror_vote(&case.jurors.get(0).unwrap(), &case_id, &Verdict::BuyerWins);
    arbitration_client.juror_vote(&case.jurors.get(1).unwrap(), &case_id, &Verdict::BuyerWins);

    // Try to vote again on resolved case
    // Reset the case to try voting
    arbitration_client.juror_vote(&case.jurors.get(2).unwrap(), &case_id, &Verdict::SellerWins);
}

#[test]
#[should_panic(expected = "insufficient jurors in pool")]
fn test_create_case_with_insufficient_jurors_panics() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Only stake 2 jurors (need at least 3)
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);

    // Try to create case - should panic
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_007");

    escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );
}

#[test]
#[should_panic(expected = "unauthorized: only escrow contract can create cases")]
fn test_unauthorized_cannot_create_case() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, _escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Unauthorized caller tries to create case
    let unauthorized = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_008");

    arbitration_client.create_case(
        &unauthorized,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );
}

// ============================================================================
// Lockup Period After Case Participation Tests
// ============================================================================

#[test]
#[should_panic(expected = "lockup period not elapsed")]
fn test_lockup_period_resets_after_case_participation() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Create and vote on a case
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_009");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    let case = arbitration_client.get_case(&case_id);

    // All jurors vote
    arbitration_client.juror_vote(&case.jurors.get(0).unwrap(), &case_id, &Verdict::BuyerWins);
    arbitration_client.juror_vote(&case.jurors.get(1).unwrap(), &case_id, &Verdict::BuyerWins);
    arbitration_client.juror_vote(&case.jurors.get(2).unwrap(), &case_id, &Verdict::SellerWins);

    // Advance only 3 days (less than 7 day lockup after case participation)
    advance_time(&env, 3 * 24 * 60 * 60);

    // Try to unstake - should fail because lockup period reset after case
    let juror_record = arbitration_client.get_juror_record(&case.jurors.get(0).unwrap()).unwrap();
    assert!(juror_record.last_case_timestamp > 0);
    
    arbitration_client.unstake(&case.jurors.get(0).unwrap());
}

#[test]
fn test_can_unstake_after_lockup_from_case() {
    let env = Env::default();
    let (arbitration_client, _admin, token_client, token_admin, escrow) = setup_test(&env);

    // Stake 3 jurors
    let juror1 = Address::generate(&env);
    let juror2 = Address::generate(&env);
    let juror3 = Address::generate(&env);

    token_admin.mint(&juror1, &300_000_000);
    token_admin.mint(&juror2, &300_000_000);
    token_admin.mint(&juror3, &300_000_000);

    arbitration_client.stake_as_juror(&juror1, &150_000_000);
    arbitration_client.stake_as_juror(&juror2, &150_000_000);
    arbitration_client.stake_as_juror(&juror3, &150_000_000);

    // Create and vote on a case
    let escrow_client = MockEscrowContractClient::new(&env, &escrow);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_010");

    let case_id = escrow_client.create_arbitration_case(
        &arbitration_client.address,
        &tx_id,
        &buyer,
        &seller,
        &1000,
        &token_client.address,
    );

    let case = arbitration_client.get_case(&case_id);

    // All jurors vote
    arbitration_client.juror_vote(&case.jurors.get(0).unwrap(), &case_id, &Verdict::BuyerWins);
    arbitration_client.juror_vote(&case.jurors.get(1).unwrap(), &case_id, &Verdict::BuyerWins);

    // Advance 7 days + 1 second after case participation
    advance_time(&env, 7 * 24 * 60 * 60 + 1);

    // Should be able to unstake now
    arbitration_client.unstake(&case.jurors.get(0).unwrap());

    // Verify unstake succeeded
    assert_eq!(token_client.balance(&case.jurors.get(0).unwrap()), 300_000_000);
}

// ============================================================================
// Getter Function Tests
// ============================================================================

#[test]
fn test_get_min_stake_amount() {
    let env = Env::default();
    let (arbitration_client, _admin, _token_client, _token_admin, _escrow) = setup_test(&env);

    assert_eq!(arbitration_client.get_min_stake_amount(), 100_000_000);
}

#[test]
fn test_get_lockup_period() {
    let env = Env::default();
    let (arbitration_client, _admin, _token_client, _token_admin, _escrow) = setup_test(&env);

    assert_eq!(arbitration_client.get_lockup_period(), 604800); // 7 days
}
