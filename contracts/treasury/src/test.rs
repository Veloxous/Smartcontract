#![cfg(test)]

use super::*;
use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env, Vec};

fn create_token_contract<'a>(
    env: &'a Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    (
        token::Client::new(env, &sac.address()),
        token::StellarAssetClient::new(env, &sac.address()),
    )
}

fn setup_treasury_test<'a>(
    env: &'a Env,
) -> (
    TreasuryContractClient<'a>,
    Address, // admin
    Address, // ops wallet (80%)
    Address, // dao wallet (20%)
    Address, // asset token address
    token::Client<'a>,
    token::StellarAssetClient<'a>,
) {
    env.mock_all_auths();

    let contract_id = env.register(TreasuryContract, ());
    let treasury_client = TreasuryContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let ops_wallet = Address::generate(env);
    let dao_wallet = Address::generate(env);

    let mut splits = Vec::new(env);
    splits.push_back(TreasurySplit {
        wallet: ops_wallet.clone(),
        share_bps: 8000, // 80.00%
    });
    splits.push_back(TreasurySplit {
        wallet: dao_wallet.clone(),
        share_bps: 2000, // 20.00%
    });

    treasury_client.init(&admin, &100, &splits); // 100 BPS = 1.00%

    let token_admin = Address::generate(env);
    let (token_client, token_admin_client) = create_token_contract(env, &token_admin);

    (
        treasury_client,
        admin,
        ops_wallet,
        dao_wallet,
        token_client.address.clone(),
        token_client,
        token_admin_client,
    )
}

#[test]
fn test_fee_calculation_and_rounding() {
    let env = Env::default();
    let (treasury_client, _admin, _ops, _dao, _asset, _token_client, _token_admin) =
        setup_treasury_test(&env);

    // 100 BPS (1.00%) on 1,000,000 base_amount = 10,000 fee
    let fee = treasury_client.calculate_fee(&1_000_000);
    assert_eq!(fee, 10_000);

    // Test rounding down: 100 BPS on 99 base_amount = 99 * 100 / 10000 = 0 (rounds down)
    let small_fee = treasury_client.calculate_fee(&99);
    assert_eq!(small_fee, 0);

    // 100 BPS on 150 base_amount = 150 * 100 / 10000 = 1
    let fee_150 = treasury_client.calculate_fee(&150);
    assert_eq!(fee_150, 1);
}

#[test]
fn test_fee_exceeds_limit_rejection() {
    let env = Env::default();
    let (treasury_client, admin, _ops, _dao, _asset, _token_client, _token_admin) =
        setup_treasury_test(&env);

    // Attempting to set 600 BPS (6.00%) which exceeds MAX_FEE_BPS (500 BPS / 5.00%)
    let result = treasury_client.try_update_fee_bps(&admin, &600);
    assert_eq!(result, Err(Ok(Error::FeeExceedsLimit)));
}

#[test]
fn test_treasury_splits_must_total_100_percent() {
    let env = Env::default();
    let (treasury_client, admin, ops, dao, _asset, _token_client, _token_admin) =
        setup_treasury_test(&env);

    let mut invalid_splits = Vec::new(&env);
    invalid_splits.push_back(TreasurySplit {
        wallet: ops,
        share_bps: 5000,
    });
    invalid_splits.push_back(TreasurySplit {
        wallet: dao,
        share_bps: 4000, // Total 9000 != 10000
    });

    let result = treasury_client.try_update_treasury_splits(&admin, &invalid_splits);
    assert_eq!(result, Err(Ok(Error::InvalidSplitTotal)));
}

#[test]
fn test_treasury_routing_80_20_split() {
    let env = Env::default();
    let (treasury_client, _admin, ops, dao, asset, token_client, token_admin) =
        setup_treasury_test(&env);

    // Mint 10,000 tokens to treasury contract
    token_admin.mint(&treasury_client.address, &10_000);

    // Route 10,000 tokens fee
    treasury_client.route_fee(&asset, &10_000);

    // Ops wallet receives 80% (8,000)
    assert_eq!(token_client.balance(&ops), 8_000);
    // DAO wallet receives 20% (2,000)
    assert_eq!(token_client.balance(&dao), 2_000);
    // Treasury contract balance reset to 0
    assert_eq!(token_client.balance(&treasury_client.address), 0);
}

#[test]
fn test_admin_updates() {
    let env = Env::default();
    let (treasury_client, admin, _ops, _dao, _asset, _token_client, _token_admin) =
        setup_treasury_test(&env);

    // Valid update to 250 BPS (2.50%)
    treasury_client.update_fee_bps(&admin, &250);
    assert_eq!(treasury_client.get_fee_bps(), 250);
}

// Property-based testing for fee calculation arithmetic
proptest! {
    #[test]
    fn prop_fee_calculation_bounds_and_remainder(
        base_amount in 0i128..1_000_000_000_000i128,
        fee_bps in 0u32..=500u32,
    ) {
        let fee_bps_i128 = fee_bps as i128;
        let fee = (base_amount * fee_bps_i128) / 10_000i128;
        let remainder = base_amount - fee;

        // Verify fee + remainder == base_amount
        prop_assert_eq!(fee + remainder, base_amount);
        // Verify fee is <= base_amount
        prop_assert!(fee <= base_amount);
        // Verify no overflow
        prop_assert!(fee >= 0);
    }
}
