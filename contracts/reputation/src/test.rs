#![cfg(test)]

use crate::{Error, ReputationContract, ReputationContractClient, Tier};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_test() -> (Env, ReputationContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ReputationContract);
    let client = ReputationContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    client.init(&admin);
    
    (env, client, admin, user)
}

#[test]
fn test_minting_and_metadata() {
    let (env, client, admin, user) = setup_test();
    
    assert_eq!(client.balance(&user), 0);
    client.mint(&admin, &user);
    assert_eq!(client.balance(&user), 1);
    
    let score = client.get_trust_score(&user);
    assert_eq!(score.total_transactions, 0);
    assert_eq!(score.score, 0);
    assert_eq!(score.score_tier, Tier::Unverified);
    
    assert_eq!(client.name(), String::from_str(&env, "Veloxous Reputation"));
    assert_eq!(client.symbol(), String::from_str(&env, "VREP"));
    assert_eq!(client.decimals(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_transfer_panics() {
    let (_env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let to = Address::generate(&_env);
    client.transfer(&user, &to, &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_transfer_from_panics() {
    let (_env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let spender = Address::generate(&_env);
    let to = Address::generate(&_env);
    client.transfer_from(&spender, &user, &to, &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_approve_panics() {
    let (_env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let spender = Address::generate(&_env);
    client.approve(&user, &spender, &1, &100);
}

#[test]
fn test_update_score() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let escrow_contract = Address::generate(&env);
    client.add_authorized_contract(&admin, &escrow_contract);
    
    // Successful transaction
    client.update_score(&escrow_contract, &user, &true, &false, &1000, &100);
    
    let mut score = client.get_trust_score(&user);
    assert_eq!(score.total_transactions, 1);
    assert_eq!(score.successful_transactions, 1);
    assert_eq!(score.disputes_lost, 0);
    assert_eq!(score.score, 1);
    assert_eq!(score.score_tier, Tier::Unverified);
    
    // Unsuccessful transaction (value too low)
    client.update_score(&escrow_contract, &user, &true, &false, &50, &100);
    score = client.get_trust_score(&user);
    assert_eq!(score.total_transactions, 1); // Should remain 1
    
    // Lost dispute
    client.update_score(&escrow_contract, &user, &false, &true, &1000, &100);
    score = client.get_trust_score(&user);
    assert_eq!(score.total_transactions, 2);
    assert_eq!(score.successful_transactions, 1);
    assert_eq!(score.disputes_lost, 1);
    assert_eq!(score.score, -49);
}

#[test]
fn test_tier_upgrades() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let escrow_contract = Address::generate(&env);
    client.add_authorized_contract(&admin, &escrow_contract);
    
    // Give +11 score for Trusted tier
    for _ in 0..11 {
        client.update_score(&escrow_contract, &user, &true, &false, &1000, &100);
    }
    
    let mut score = client.get_trust_score(&user);
    assert_eq!(score.score, 11);
    assert_eq!(score.score_tier, Tier::Trusted);
    assert_eq!(client.get_user_tier(&user), Tier::Trusted);
    
    // Give +90 score to reach 101 for Elite tier
    for _ in 0..90 {
        client.update_score(&escrow_contract, &user, &true, &false, &1000, &100);
    }
    
    score = client.get_trust_score(&user);
    assert_eq!(score.score, 101);
    assert_eq!(score.score_tier, Tier::Elite);
    assert_eq!(client.get_user_tier(&user), Tier::Elite);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_unauthorized_update() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let unauthorized_caller = Address::generate(&env);
    
    // Should panic because unauthorized_caller is not an authorized contract
    client.update_score(&unauthorized_caller, &user, &true, &false, &1000, &100);
}

// ── Phase 3 SBT Dynamic Metadata Tests ───────────────────────────────────────

#[test]
fn test_metadata_init_and_query() {
    let (env, client, admin, user) = setup_test();
    
    // Mint auto-initializes default metadata
    client.mint(&admin, &user);
    
    let metadata = client.get_metadata(&user);
    assert_eq!(metadata.user, user);
    assert_eq!(metadata.uri, String::from_str(&env, "ipfs://default_sbt_metadata"));
    assert_eq!(metadata.version, 1);
    assert_eq!(metadata.state, crate::metadata::SbtMetadataState::Active);
    
    assert_eq!(client.token_uri(&user), String::from_str(&env, "ipfs://default_sbt_metadata"));
}

#[test]
fn test_update_metadata_and_version_control() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let new_uri1 = String::from_str(&env, "ipfs://updated_metadata_v2");
    let updated1 = client.update_metadata(&admin, &user, &new_uri1, &1);
    
    assert_eq!(updated1.uri, new_uri1);
    assert_eq!(updated1.version, 2);
    assert_eq!(updated1.state, crate::metadata::SbtMetadataState::Active);
    assert_eq!(client.token_uri(&user), new_uri1);
    
    let new_uri2 = String::from_str(&env, "ipfs://updated_metadata_v3");
    let updated2 = client.update_metadata(&admin, &user, &new_uri2, &2);
    assert_eq!(updated2.version, 3);
    assert_eq!(updated2.uri, new_uri2);
}

#[test]
fn test_metadata_version_mismatch_race_condition() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let new_uri = String::from_str(&env, "ipfs://race_condition_update");
    // Passing outdated version 0 instead of expected version 1
    let res = client.try_update_metadata(&admin, &user, &new_uri, &0);
    assert_eq!(res, Err(Ok(Error::VersionMismatch)));
}

#[test]
fn test_metadata_state_machine_transitions() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    // Active -> Suspended
    let suspended = client.set_metadata_state(&admin, &user, &crate::metadata::SbtMetadataState::Suspended);
    assert_eq!(suspended.state, crate::metadata::SbtMetadataState::Suspended);
    
    // Updates while Suspended should fail with MetadataStateInvalid
    let new_uri = String::from_str(&env, "ipfs://invalid_update_while_suspended");
    let res = client.try_update_metadata(&admin, &user, &new_uri, &1);
    assert_eq!(res, Err(Ok(Error::MetadataStateInvalid)));
    
    // Suspended -> Active
    let reactivated = client.set_metadata_state(&admin, &user, &crate::metadata::SbtMetadataState::Active);
    assert_eq!(reactivated.state, crate::metadata::SbtMetadataState::Active);
    
    // Now updates succeed again
    let updated = client.update_metadata(&admin, &user, &new_uri, &1);
    assert_eq!(updated.version, 2);
}

#[test]
fn test_revoked_metadata_blocks_access() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    // Transition to Revoked (terminal state)
    let revoked = client.set_metadata_state(&admin, &user, &crate::metadata::SbtMetadataState::Revoked);
    assert_eq!(revoked.state, crate::metadata::SbtMetadataState::Revoked);
    
    // token_uri should fail on Revoked state
    let uri_res = client.try_token_uri(&user);
    assert_eq!(uri_res, Err(Ok(Error::MetadataStateInvalid)));
    
    // Updating revoked metadata should fail
    let new_uri = String::from_str(&env, "ipfs://update_revoked");
    let update_res = client.try_update_metadata(&admin, &user, &new_uri, &1);
    assert_eq!(update_res, Err(Ok(Error::MetadataStateInvalid)));
}

#[test]
fn test_unauthorized_metadata_update() {
    let (env, client, admin, user) = setup_test();
    client.mint(&admin, &user);
    
    let unauthorized = Address::generate(&env);
    let new_uri = String::from_str(&env, "ipfs://unauthorized_update");
    let res = client.try_update_metadata(&unauthorized, &user, &new_uri, &1);
    assert_eq!(res, Err(Ok(Error::Unauthorized)));
}

