#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, testutils::{Address as _, Ledger}, token, Address, BytesN, Env, String, Vec,
};

#[soroban_sdk::contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockReputationUpdate {
    #[topic]
    pub winner: Address,
    #[topic]
    pub loser: Address,
    pub transaction_id: String,
}

#[contract]
pub struct MockReputationContract;

#[contractimpl]
impl MockReputationContract {
    pub fn record_dispute_result(
        env: Env,
        winner: Address,
        loser: Address,
        transaction_id: String,
    ) {
        MockReputationUpdate {
            winner,
            loser,
            transaction_id,
        }
        .publish(&env);
    }
}

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
    MarketplaceEscrowClient<'a>,
    Address, // token
    token::Client<'a>,
    token::StellarAssetClient<'a>,
    Vec<Address>, // 5 admins
    Address,      // buyer
    Address,      // seller
    Address,      // reputation contract
) {
    env.mock_all_auths();

    let contract_id = env.register(MarketplaceEscrow, ());
    let escrow_client = MarketplaceEscrowClient::new(env, &contract_id);

    let token_admin = Address::generate(env);
    let (token_client, token_admin_client) = create_token_contract(env, &token_admin);

    let rep_id = env.register(MockReputationContract, ());

    let admin1 = Address::generate(env);
    let admin2 = Address::generate(env);
    let admin3 = Address::generate(env);
    let admin4 = Address::generate(env);
    let admin5 = Address::generate(env);

    let mut admins = Vec::new(env);
    admins.push_back(admin1);
    admins.push_back(admin2);
    admins.push_back(admin3);
    admins.push_back(admin4);
    admins.push_back(admin5);

    escrow_client.init(&admins, &3, &Some(rep_id.clone()), &None);

    let buyer = Address::generate(env);
    let seller = Address::generate(env);

    // Fund buyer with 1000 tokens
    token_admin_client.mint(&buyer, &1000);

    (
        escrow_client,
        token_client.address.clone(),
        token_client,
        token_admin_client,
        admins,
        buyer,
        seller,
        rep_id,
    )
}

#[test]
fn test_dispute_creation_and_lock_blocks_execution() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_100");
    escrow_client.deposit(&buyer, &seller, &token_addr, &500, &tx_id);

    assert_eq!(token_client.balance(&buyer), 500);
    assert_eq!(token_client.balance(&escrow_client.address), 500);

    let escrow = escrow_client.get_escrow(&tx_id);
    assert_eq!(escrow.status, EscrowStatus::Locked);

    // Raise dispute by buyer
    let reason = BytesN::from_array(&env, &[1u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    let escrow_after = escrow_client.get_escrow(&tx_id);
    assert_eq!(escrow_after.status, EscrowStatus::Disputed);

    let dispute = escrow_client.get_dispute(&tx_id);
    assert_eq!(dispute.buyer, buyer);
    assert_eq!(dispute.seller, seller);
    assert_eq!(dispute.raised_by, buyer);
    assert_eq!(dispute.reason_hash, reason);
}

#[test]
#[should_panic(expected = "escrow is disputed")]
fn test_release_blocked_during_dispute() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_101");
    escrow_client.deposit(&buyer, &seller, &token_addr, &500, &tx_id);

    let reason = BytesN::from_array(&env, &[2u8; 32]);
    escrow_client.raise_dispute(&seller, &tx_id, &reason);

    // Try release (should panic)
    escrow_client.release(&tx_id);
}

#[test]
#[should_panic(expected = "escrow is disputed")]
fn test_withdraw_blocked_during_dispute() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_102");
    escrow_client.deposit(&buyer, &seller, &token_addr, &500, &tx_id);

    let reason = BytesN::from_array(&env, &[3u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    escrow_client.withdraw(&tx_id);
}

#[test]
#[should_panic(expected = "escrow is disputed")]
fn test_auto_release_blocked_during_dispute() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_103");
    escrow_client.deposit(&buyer, &seller, &token_addr, &500, &tx_id);

    let reason = BytesN::from_array(&env, &[4u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    escrow_client.auto_release(&tx_id);
}

#[test]
#[should_panic(expected = "escrow is disputed")]
fn test_auto_refund_blocked_during_dispute() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_104");
    escrow_client.deposit(&buyer, &seller, &token_addr, &500, &tx_id);

    let reason = BytesN::from_array(&env, &[5u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    escrow_client.auto_refund(&tx_id);
}

#[test]
#[should_panic(expected = "escrow is disputed")]
fn test_milestone_approval_blocked_during_dispute() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_105");
    escrow_client.deposit(&buyer, &seller, &token_addr, &500, &tx_id);

    let reason = BytesN::from_array(&env, &[6u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    escrow_client.approve_milestone(&tx_id);
}

#[test]
fn test_resolution_voting_flow_and_reputation_update() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_200");
    escrow_client.deposit(&buyer, &seller, &token_addr, &1000, &tx_id);

    let reason = BytesN::from_array(&env, &[7u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    let admin1 = admins.get(0).unwrap();
    let admin2 = admins.get(1).unwrap();
    let admin3 = admins.get(2).unwrap();
    let admin4 = admins.get(3).unwrap();

    // Proposal A: 700 refund to buyer, 300 payout to seller
    // 2 admins vote Proposal A
    escrow_client.propose_resolution(&admin1, &tx_id, &700, &300);
    escrow_client.vote_resolution(&admin2, &tx_id, &700, &300);

    // 1 admin votes Proposal B: 500 refund, 500 payout
    escrow_client.propose_resolution(&admin3, &tx_id, &500, &500);

    // Nothing executes yet (Proposal A has 2 votes, Proposal B has 1 vote; threshold is 3)
    let escrow_mid = escrow_client.get_escrow(&tx_id);
    assert_eq!(escrow_mid.status, EscrowStatus::Disputed);

    // 3rd admin votes Proposal A (reaching threshold 3)
    escrow_client.vote_resolution(&admin4, &tx_id, &700, &300);

    // Resolution executes!
    let escrow_resolved = escrow_client.get_escrow(&tx_id);
    assert_eq!(escrow_resolved.status, EscrowStatus::Resolved);

    // Funds transferred correctly (buyer gets 700 refund => total 700; seller gets 300 payout)
    assert_eq!(token_client.balance(&buyer), 700);
    assert_eq!(token_client.balance(&seller), 300);
    assert_eq!(token_client.balance(&escrow_client.address), 0);
}

#[test]
#[should_panic(expected = "invalid resolution amounts")]
fn test_invalid_payout_total_panics() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_300");
    escrow_client.deposit(&buyer, &seller, &token_addr, &1000, &tx_id);

    let reason = BytesN::from_array(&env, &[8u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    let admin1 = admins.get(0).unwrap();
    let admin2 = admins.get(1).unwrap();
    let admin3 = admins.get(2).unwrap();

    // Invalid sum (700 + 400 = 1100 != 1000)
    escrow_client.vote_resolution(&admin1, &tx_id, &700, &400);
    escrow_client.vote_resolution(&admin2, &tx_id, &700, &400);
    escrow_client.vote_resolution(&admin3, &tx_id, &700, &400);
}

#[test]
#[should_panic(expected = "duplicate vote")]
fn test_duplicate_vote_rejected() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_400");
    escrow_client.deposit(&buyer, &seller, &token_addr, &1000, &tx_id);

    let reason = BytesN::from_array(&env, &[9u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    let admin1 = admins.get(0).unwrap();
    escrow_client.propose_resolution(&admin1, &tx_id, &600, &400);
    // Duplicate vote by admin1
    escrow_client.vote_resolution(&admin1, &tx_id, &600, &400);
}

#[test]
#[should_panic(expected = "only admins may vote")]
fn test_non_admin_vote_rejected() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let tx_id = String::from_str(&env, "tx_500");
    escrow_client.deposit(&buyer, &seller, &token_addr, &1000, &tx_id);

    let reason = BytesN::from_array(&env, &[10u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    let outsider = Address::generate(&env);
    escrow_client.propose_resolution(&outsider, &tx_id, &600, &400);
}

#[test]
fn test_admin_rotation_and_voting_rights() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, admins, buyer, seller, _rep) =
        setup_test(&env);

    let admin1 = admins.get(0).unwrap();
    let admin2 = admins.get(1).unwrap();
    let admin3 = admins.get(2).unwrap();
    let old_admin = admins.get(4).unwrap(); // admin5

    let new_admin = Address::generate(&env);

    // Propose admin rotation
    escrow_client.propose_admin_change(&admin1, &old_admin, &new_admin);
    escrow_client.propose_admin_change(&admin2, &old_admin, &new_admin);
    escrow_client.propose_admin_change(&admin3, &old_admin, &new_admin);

    let updated_admins = escrow_client.get_admins();
    assert!(!updated_admins.contains(&old_admin));
    assert!(updated_admins.contains(&new_admin));

    // Prepare disputed transaction
    let tx_id = String::from_str(&env, "tx_600");
    escrow_client.deposit(&buyer, &seller, &token_addr, &1000, &tx_id);
    let reason = BytesN::from_array(&env, &[11u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    // New admin can vote
    escrow_client.propose_resolution(&new_admin, &tx_id, &500, &500);
}

#[test]
#[should_panic(expected = "only admins may vote")]
fn test_old_admin_cannot_vote_after_rotation() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, admins, buyer, seller, _rep) =
        setup_test(&env);

    let admin1 = admins.get(0).unwrap();
    let admin2 = admins.get(1).unwrap();
    let admin3 = admins.get(2).unwrap();
    let old_admin = admins.get(4).unwrap(); // admin5

    let new_admin = Address::generate(&env);

    // Rotate admin5 -> new_admin
    escrow_client.propose_admin_change(&admin1, &old_admin, &new_admin);
    escrow_client.propose_admin_change(&admin2, &old_admin, &new_admin);
    escrow_client.propose_admin_change(&admin3, &old_admin, &new_admin);

    // Prepare disputed transaction
    let tx_id = String::from_str(&env, "tx_700");
    escrow_client.deposit(&buyer, &seller, &token_addr, &1000, &tx_id);
    let reason = BytesN::from_array(&env, &[12u8; 32]);
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    // Old admin attempts to vote (should panic)
    escrow_client.propose_resolution(&old_admin, &tx_id, &500, &500);
}

#[test]
fn test_fee_pool_accumulation_and_sweep() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, token_admin, _admins, _buyer, _seller, _rep) =
        setup_test(&env);

    // Accumulate fee into escrow fee pool
    escrow_client.accumulate_fee(&token_addr, &150);
    assert_eq!(escrow_client.get_fee_pool(&token_addr), 150);

    // Mint tokens to escrow contract so sweep_fees can transfer
    token_admin.mint(&escrow_client.address, &150);
    assert_eq!(token_client.balance(&escrow_client.address), 150);

    // Sweep fees (without treasury contract set, pool is reset to 0 and event emitted)
    escrow_client.sweep_fees(&token_addr);
    assert_eq!(escrow_client.get_fee_pool(&token_addr), 0);
}

#[test]
#[should_panic(expected = "fee pool empty")]
fn test_double_sweep_prevention() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, token_admin, _admins, _buyer, _seller, _rep) =
        setup_test(&env);

    escrow_client.accumulate_fee(&token_addr, &100);
    token_admin.mint(&escrow_client.address, &100);

    escrow_client.sweep_fees(&token_addr);
    // Second sweep should panic with "fee pool empty"
    escrow_client.sweep_fees(&token_addr);
}

// ===========================================================================
// Dutch Auction Tests
// ===========================================================================

/// Helper: advance ledger timestamp by `delta_secs` seconds.
fn advance_time(env: &Env, delta_secs: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp.saturating_add(delta_secs);
    });
}

/// Issue requirement: fast-forward ledger to 50% of auction duration and
/// assert the current price is the midpoint between start_price and end_price.
#[test]
fn test_dutch_auction_midpoint_price() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, _buyer, seller, _rep) =
        setup_test(&env);

    let listing_id = String::from_str(&env, "auction_001");
    let start_price: i128 = 10_000;
    let end_price: i128 = 2_000;
    let duration_secs: u64 = 1_000;

    escrow_client.create_auction(
        &listing_id,
        &seller,
        &token_addr,
        &start_price,
        &end_price,
        &duration_secs,
    );

    // Price at t=0 must equal start_price.
    let price_at_start = escrow_client.get_current_price(&listing_id);
    assert_eq!(price_at_start, start_price);

    // Advance to exactly 50% of the duration.
    advance_time(&env, duration_secs / 2); // +500 s

    let price_mid = escrow_client.get_current_price(&listing_id);
    // midpoint = 10_000 - ((10_000 - 2_000) * 500 / 1_000) = 10_000 - 4_000 = 6_000
    let expected_mid = start_price - ((start_price - end_price) * (duration_secs / 2) as i128
        / duration_secs as i128);
    assert_eq!(price_mid, expected_mid, "price at 50% should be midpoint");
    assert_eq!(price_mid, 6_000);
}

/// Issue requirement: assert that buy_now after auction expiry panics with "AuctionExpired".
#[test]
#[should_panic(expected = "AuctionExpired")]
fn test_buy_now_after_expiry_panics() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let listing_id = String::from_str(&env, "auction_002");
    let start_price: i128 = 5_000;
    let end_price: i128 = 1_000;
    let duration_secs: u64 = 600;

    escrow_client.create_auction(
        &listing_id,
        &seller,
        &token_addr,
        &start_price,
        &end_price,
        &duration_secs,
    );

    // Fund buyer generously.
    token_admin.mint(&buyer, &10_000);

    // Fast-forward past the full duration.
    advance_time(&env, duration_secs + 1); // 601 s — auction is expired

    // This must panic with "AuctionExpired".
    escrow_client.buy_now(&listing_id, &buyer);
}

/// Happy path: buyer calls buy_now before expiry; USDC is locked in the contract
/// at the exact captured price, and auction moves to Sold.
#[test]
fn test_buy_now_success_transfers_usdc_and_closes_auction() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let listing_id = String::from_str(&env, "auction_003");
    let start_price: i128 = 8_000;
    let end_price: i128 = 2_000;
    let duration_secs: u64 = 1_000;

    escrow_client.create_auction(
        &listing_id,
        &seller,
        &token_addr,
        &start_price,
        &end_price,
        &duration_secs,
    );

    // Advance to 25% of duration → price = 8_000 - (6_000 * 250 / 1_000) = 8_000 - 1_500 = 6_500.
    advance_time(&env, 250);
    let expected_price: i128 = 6_500;
    assert_eq!(escrow_client.get_current_price(&listing_id), expected_price);

    token_admin.mint(&buyer, &expected_price);

    escrow_client.buy_now(&listing_id, &buyer);

    // Buyer's USDC balance should be 1000 (initial mint from setup_test, while the minted price amount is transferred to the contract).
    assert_eq!(token_client.balance(&buyer), 1000);
    // Contract holds the locked funds.
    assert_eq!(token_client.balance(&escrow_client.address), expected_price);

    // Auction state should be Sold with correct final_price.
    let state = escrow_client.get_auction(&listing_id);
    assert_eq!(state.status, types::AuctionStatus::Sold);
    assert_eq!(state.final_price, Some(expected_price));
    assert_eq!(state.buyer, Some(buyer));
}

/// Buying an already-sold auction must be rejected.
#[test]
#[should_panic(expected = "auction is not active")]
fn test_cannot_buy_already_sold_auction() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, token_admin, _admins, buyer, seller, _rep) =
        setup_test(&env);

    let listing_id = String::from_str(&env, "auction_004");

    escrow_client.create_auction(
        &listing_id,
        &seller,
        &token_addr,
        &4_000,
        &1_000,
        &500,
    );

    token_admin.mint(&buyer, &8_000);

    // First purchase succeeds.
    escrow_client.buy_now(&listing_id, &buyer);

    // Second purchase on the same, now-closed auction must panic.
    let buyer2 = Address::generate(&env);
    token_admin.mint(&buyer2, &8_000);
    escrow_client.buy_now(&listing_id, &buyer2);
}

/// cancel_expired sets the auction to Expired and can be called by anyone.
#[test]
fn test_cancel_expired_sets_expired_state() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admins, _buyer, seller, _rep) =
        setup_test(&env);

    let listing_id = String::from_str(&env, "auction_005");

    escrow_client.create_auction(
        &listing_id,
        &seller,
        &token_addr,
        &3_000,
        &500,
        &200,
    );

    advance_time(&env, 201); // past deadline

    // A random third party can call cancel_expired.
    let anyone = Address::generate(&env);
    let _ = anyone; // suppress unused-variable warning; auth is mocked globally
    escrow_client.cancel_expired(&listing_id);

    let state = escrow_client.get_auction(&listing_id);
    assert_eq!(state.status, types::AuctionStatus::Expired);
}

