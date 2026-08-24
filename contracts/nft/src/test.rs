#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token, Address, Env, String,
};

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

#[allow(dead_code)]
struct TestContext<'a> {
    client: DeviceNftClient<'a>,
    usdc: token::Client<'a>,
    usdc_admin: token::StellarAssetClient<'a>,
    /// Kept for future tests that require admin-gated operations.
    admin: Address,
    marketplace: Address,
}

fn setup<'a>(env: &'a Env) -> TestContext<'a> {
    env.mock_all_auths();

    let contract_id = env.register(DeviceNft, ());
    let client = DeviceNftClient::new(env, &contract_id);

    let token_admin = Address::generate(env);
    let (usdc, usdc_admin) = create_token_contract(env, &token_admin);

    let admin = Address::generate(env);
    let marketplace = Address::generate(env);

    // The generated client returns () for Result<(), Error> — panics on error.
    client.init(&admin, &usdc.address, &marketplace);

    TestContext {
        client,
        usdc,
        usdc_admin,
        admin,
        marketplace,
    }
}

fn listing_id(env: &Env, id: &str) -> String {
    String::from_str(env, id)
}

/// Issue requirement: Fractionalize a device into 100 shares.
/// Have 3 users buy varying amounts.
/// Sell the device and assert each user receives the correct proportional payout.
#[test]
fn test_fractionalize_sell_distribute_proportional_payouts() {
    let env = Env::default();
    let ctx = setup(&env);

    let owner = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let user_c = Address::generate(&env);
    let id = listing_id(&env, "device_001");

    // Register the device with a $10,000 USDC valuation → 100 shares × $100/share.
    ctx.client.register_device(&owner, &id, &10_000);
    ctx.client.fractionalize(&owner, &id, &100);

    assert_eq!(ctx.client.get_share_balance(&id, &owner), 100);

    // Simulate users acquiring shares via transfer (standard fungible-token semantics).
    ctx.client.transfer_shares(&owner, &user_a, &id, &40);
    ctx.client.transfer_shares(&owner, &user_b, &id, &35);
    ctx.client.transfer_shares(&owner, &user_c, &id, &25);

    // Owner transferred all shares (40 + 35 + 25 = 100).
    assert_eq!(ctx.client.get_share_balance(&id, &user_a), 40);
    assert_eq!(ctx.client.get_share_balance(&id, &user_b), 35);
    assert_eq!(ctx.client.get_share_balance(&id, &user_c), 25);
    assert_eq!(ctx.client.get_share_balance(&id, &owner), 0);

    // Marketplace records the sale and deposits $10,000 USDC proceeds.
    ctx.usdc_admin.mint(&ctx.marketplace, &10_000);
    ctx.client.record_sale(&ctx.marketplace, &id, &10_000);

    // Verify listing state is Sold with the correct proceeds.
    let listing = ctx.client.get_listing(&id);
    assert_eq!(listing.state, types::ListingState::Sold);
    assert_eq!(listing.sale_proceeds, 10_000);

    // Each holder claims their proportional share of proceeds.
    // user_a: 40/100 × 10_000 = 4_000
    // user_b: 35/100 × 10_000 = 3_500
    // user_c: 25/100 × 10_000 = 2_500
    let payout_a = ctx.client.claim_proceeds(&user_a, &id);
    let payout_b = ctx.client.claim_proceeds(&user_b, &id);
    let payout_c = ctx.client.claim_proceeds(&user_c, &id);

    assert_eq!(payout_a, 4_000);
    assert_eq!(payout_b, 3_500);
    assert_eq!(payout_c, 2_500);

    // Verify USDC balances were actually transferred.
    assert_eq!(ctx.usdc.balance(&user_a), 4_000);
    assert_eq!(ctx.usdc.balance(&user_b), 3_500);
    assert_eq!(ctx.usdc.balance(&user_c), 2_500);
}

/// buy_out: buyer pays for all outstanding shares at share_price × outstanding_count,
/// sellers are immediately paid out, and the listing moves to BoughtOut state.
#[test]
fn test_buy_out_transfers_nft_and_pays_shareholders() {
    let env = Env::default();
    let ctx = setup(&env);

    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let buyer = Address::generate(&env);
    let id = listing_id(&env, "device_002");

    // Valuation = 10_000, 100 shares → share_price = 100.
    ctx.client.register_device(&owner, &id, &10_000);
    ctx.client.fractionalize(&owner, &id, &100);

    // holder acquires 60 shares; owner retains 40.
    ctx.client.transfer_shares(&owner, &holder, &id, &60);

    // buyer has 0 shares, so outstanding = 100. buy_out price = 100 × 100 = 10_000.
    ctx.usdc_admin.mint(&buyer, &10_000);
    ctx.client.buy_out(&buyer, &id);

    // Buyer receives all shares; holder's shares are zeroed after being paid out.
    assert_eq!(ctx.client.get_share_balance(&id, &buyer), 100);
    assert_eq!(ctx.client.get_share_balance(&id, &holder), 0);
    // holder (60 shares × 100/share) = 6_000 USDC.
    assert_eq!(ctx.usdc.balance(&holder), 6_000);

    let listing = ctx.client.get_listing(&id);
    assert_eq!(listing.state, types::ListingState::BoughtOut);
}

/// Fractionalizing the same listing twice must be rejected.
#[test]
fn test_double_fractionalize_rejected() {
    let env = Env::default();
    let ctx = setup(&env);

    let owner = Address::generate(&env);
    let id = listing_id(&env, "device_003");

    ctx.client.register_device(&owner, &id, &5_000);
    ctx.client.fractionalize(&owner, &id, &50);

    // try_ prefix returns Result<T, Result<Error, InvokeError>> — use to assert errors.
    let err = ctx.client.try_fractionalize(&owner, &id, &50);
    assert_eq!(err, Err(Ok(Error::AlreadyFractionalized)));
}

/// Claiming proceeds before a sale is recorded must be rejected.
#[test]
fn test_claim_before_sale_rejected() {
    let env = Env::default();
    let ctx = setup(&env);

    let owner = Address::generate(&env);
    let id = listing_id(&env, "device_004");

    ctx.client.register_device(&owner, &id, &1_000);
    ctx.client.fractionalize(&owner, &id, &10);

    let err = ctx.client.try_claim_proceeds(&owner, &id);
    assert_eq!(err, Err(Ok(Error::InvalidState)));
}

/// distribute_proceeds is an alias for claim_proceeds and must behave identically.
#[test]
fn test_distribute_proceeds_alias_matches_claim() {
    let env = Env::default();
    let ctx = setup(&env);

    let owner = Address::generate(&env);
    let id = listing_id(&env, "device_006");

    ctx.client.register_device(&owner, &id, &1_000);
    ctx.client.fractionalize(&owner, &id, &10);

    ctx.usdc_admin.mint(&ctx.marketplace, &1_000);
    ctx.client.record_sale(&ctx.marketplace, &id, &1_000);

    // distribute_proceeds(holder) should pay out 100% since owner holds all 10 shares.
    let payout = ctx.client.distribute_proceeds(&owner, &id);
    assert_eq!(payout, 1_000);
}

/// A holder may not claim proceeds twice.
#[test]
fn test_double_claim_rejected() {
    let env = Env::default();
    let ctx = setup(&env);

    let owner = Address::generate(&env);
    let id = listing_id(&env, "device_007");

    ctx.client.register_device(&owner, &id, &2_000);
    ctx.client.fractionalize(&owner, &id, &10);

    ctx.usdc_admin.mint(&ctx.marketplace, &2_000);
    ctx.client.record_sale(&ctx.marketplace, &id, &2_000);

    // First claim succeeds.
    let payout = ctx.client.claim_proceeds(&owner, &id);
    assert_eq!(payout, 2_000);

    // Second claim must be rejected.
    let err = ctx.client.try_claim_proceeds(&owner, &id);
    assert_eq!(err, Err(Ok(Error::AlreadyClaimed)));
}

/// Only the marketplace address may record a sale.
#[test]
fn test_unauthorized_record_sale_rejected() {
    let env = Env::default();
    let ctx = setup(&env);

    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);
    let id = listing_id(&env, "device_008");

    ctx.client.register_device(&owner, &id, &1_000);
    ctx.client.fractionalize(&owner, &id, &10);

    ctx.usdc_admin.mint(&attacker, &1_000);
    let err = ctx.client.try_record_sale(&attacker, &id, &1_000);
    assert_eq!(err, Err(Ok(Error::Unauthorized)));
}
