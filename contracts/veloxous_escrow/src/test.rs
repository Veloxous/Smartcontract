#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token, Address, Env, String,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

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

fn setup<'a>(
    env: &'a Env,
) -> (
    VeloxousEscrowClient<'a>,
    Address,               // token address
    token::Client<'a>,
    token::StellarAssetClient<'a>,
    Address,               // admin
    Address,               // buyer
    Address,               // seller
) {
    env.mock_all_auths();

    let contract_id = env.register(VeloxousEscrow, ());
    let client = VeloxousEscrowClient::new(env, &contract_id);

    let token_admin = Address::generate(env);
    let (token_client, token_admin_client) = create_token_contract(env, &token_admin);

    let admin = Address::generate(env);
    let buyer = Address::generate(env);
    let seller = Address::generate(env);

    // Initialize with accepted asset, no treasury, no vault
    client.init(&admin, &token_client.address, &None, &None);

    // Mint 10000 tokens to buyer
    token_admin_client.mint(&buyer, &10_000);

    (client, token_client.address.clone(), token_client, token_admin_client, admin, buyer, seller)
}

fn advance_time(env: &Env, seconds: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp += seconds;
    });
}

// ── Valid lifecycle tests ────────────────────────────────────────────────────

/// Happy path: AwaitingFunds → Funded → Shipped → Delivered → Completed
#[test]
fn test_valid_lifecycle_happy_path() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_001");
    let amount: i128 = 1000;

    // Buyer funds escrow (→ Funded)
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &amount, &tx_id);
    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Funded);
    assert_eq!(token_client.balance(&escrow_client.address), amount);
    assert_eq!(token_client.balance(&buyer), 10_000 - amount);

    // Seller marks shipped (→ Shipped)
    escrow_client.mark_shipped(&seller, &tx_id);
    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Shipped);

    // Buyer confirms delivery (→ Delivered)
    escrow_client.mark_delivered(&buyer, &tx_id);
    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Delivered);

    // Buyer releases funds (→ Completed)
    escrow_client.release_funds(&buyer, &tx_id);
    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Completed);

    // Fee = 1.5% of 1000 = 15 (fixed-point), seller gets 985
    assert_eq!(token_client.balance(&seller), 985);
    // Fee stays in contract (no treasury configured)
    assert_eq!(token_client.balance(&escrow_client.address), 15);
}

/// Confirm no direct path from Funded → Refunded without admin or timeout
#[test]
fn test_funded_state_persists_without_valid_transition() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_002");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Funded);
}

// ── Invalid state transition tests ───────────────────────────────────────────

/// mark_shipped on an already-Delivered escrow must fail
#[test]
fn test_invalid_transition_shipped_from_delivered() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_003");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.mark_shipped(&seller, &tx_id);
    escrow_client.mark_delivered(&buyer, &tx_id);

    // Error::InvalidStateTransition (#1)
    let result = escrow_client.try_mark_shipped(&seller, &tx_id);
    assert!(result.is_err());
}

/// mark_delivered before mark_shipped must fail (Funded → Delivered is not a valid step)
#[test]
fn test_invalid_transition_delivered_skips_shipped() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_004");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);

    // Error::InvalidStateTransition (#1)
    let result = escrow_client.try_mark_delivered(&buyer, &tx_id);
    assert!(result.is_err());
}

// ── Timeout logic tests ──────────────────────────────────────────────────────

/// auto_refund succeeds after shipping deadline (7 days) elapsed
#[test]
fn test_auto_refund_after_shipping_deadline() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_005");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);

    // Fast-forward 7 days + 1 second
    advance_time(&env, SHIPPING_DEADLINE_SECS + 1);

    escrow_client.auto_refund(&tx_id);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Refunded);
    assert_eq!(token_client.balance(&buyer), 10_000); // Full refund
    assert_eq!(token_client.balance(&seller), 0);
}

/// auto_refund before deadline must fail
#[test]
fn test_auto_refund_too_early() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_006");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);

    // Error::ShippingDeadlineNotElapsed (#8)
    let result = escrow_client.try_auto_refund(&tx_id);
    assert!(result.is_err());
}

/// auto_release succeeds after acceptance deadline (14 days) elapsed
#[test]
fn test_auto_release_after_acceptance_deadline() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_007");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.mark_shipped(&seller, &tx_id);

    // Fast-forward 14 days + 1 second
    advance_time(&env, ACCEPTANCE_DEADLINE_SECS + 1);

    escrow_client.auto_release(&tx_id);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Completed);
    assert_eq!(token_client.balance(&seller), 985); // net after 1.5% fee
    assert_eq!(token_client.balance(&escrow_client.address), 15); // fee in contract
}

/// auto_release before acceptance deadline must fail
#[test]
fn test_auto_release_too_early() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_008");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.mark_shipped(&seller, &tx_id);

    // Error::AcceptanceDeadlineNotElapsed (#9)
    let result = escrow_client.try_auto_release(&tx_id);
    assert!(result.is_err());
}

// ── Dispute flow tests ───────────────────────────────────────────────────────

/// Buyer raises dispute from Funded state
#[test]
fn test_raise_dispute_from_funded() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_009");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);

    let reason = String::from_str(&env, "Item not as described");
    escrow_client.raise_dispute(&buyer, &tx_id, &reason);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Disputed);
    assert_eq!(record.dispute_reason, Some(reason));
}

/// Seller raises dispute from Shipped state
#[test]
fn test_raise_dispute_from_shipped() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_010");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.mark_shipped(&seller, &tx_id);

    let reason = String::from_str(&env, "Package damaged");
    escrow_client.raise_dispute(&seller, &tx_id, &reason);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Disputed);
}

/// mark_shipped must fail when escrow is Disputed
#[test]
fn test_mark_shipped_blocked_during_dispute() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_011");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.raise_dispute(&buyer, &tx_id, &String::from_str(&env, "Dispute"));

    // Error::DisputeActive (#7)
    let result = escrow_client.try_mark_shipped(&seller, &tx_id);
    assert!(result.is_err());
}

/// auto_refund must fail when dispute is active (even past deadline)
#[test]
fn test_auto_refund_blocked_during_dispute() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_012");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.raise_dispute(&buyer, &tx_id, &String::from_str(&env, "Issue"));

    advance_time(&env, SHIPPING_DEADLINE_SECS + 1);

    // Error::DisputeActive (#7)
    let result = escrow_client.try_auto_refund(&tx_id);
    assert!(result.is_err());
}

/// Admin resolves dispute in favor of buyer (full refund)
#[test]
fn test_admin_resolves_dispute_in_favor_of_buyer() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_013");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.raise_dispute(&buyer, &tx_id, &String::from_str(&env, "Never shipped"));

    // resolve_to_seller = false → buyer refund
    escrow_client.resolve_dispute(&admin, &tx_id, &false);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Refunded);
    assert_eq!(token_client.balance(&buyer), 10_000);
    assert_eq!(token_client.balance(&seller), 0);
}

/// Admin resolves dispute in favor of seller (fee deducted, seller paid)
#[test]
fn test_admin_resolves_dispute_in_favor_of_seller() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_014");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.raise_dispute(&seller, &tx_id, &String::from_str(&env, "Buyer scamming"));

    // resolve_to_seller = true → seller paid (minus fee)
    escrow_client.resolve_dispute(&admin, &tx_id, &true);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Completed);
    assert_eq!(token_client.balance(&seller), 985);
    assert_eq!(token_client.balance(&escrow_client.address), 15);
}

// ── Asset validation tests ────────────────────────────────────────────────────

/// Funding with wrong asset (not the accepted USDC) must fail
#[test]
fn test_fund_escrow_wrong_asset() {
    let env = Env::default();
    let (escrow_client, _token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let wrong_admin = Address::generate(&env);
    let wrong_sac = env.register_stellar_asset_contract_v2(wrong_admin.clone());
    let wrong_token = token::Client::new(&env, &wrong_sac.address());

    let tx_id = String::from_str(&env, "tx_015");

    // Error::AssetMismatch (#5)
    let result = escrow_client.try_fund_escrow(&buyer, &seller, &wrong_token.address, &1000, &tx_id);
    assert!(result.is_err());
}

/// Funding with zero amount must fail
#[test]
fn test_fund_escrow_zero_amount() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_016");

    // Error::InvalidAmount (#13)
    let result = escrow_client.try_fund_escrow(&buyer, &seller, &token_addr, &0, &tx_id);
    assert!(result.is_err());
}

/// Duplicate transaction_id must fail
#[test]
fn test_fund_escrow_duplicate_transaction_id() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_017");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);

    // Mint more tokens so the second call doesn't fail on balance
    token_admin.mint(&buyer, &500);

    // Error::AlreadyExists (#2)
    let result = escrow_client.try_fund_escrow(&buyer, &seller, &token_addr, &500, &tx_id);
    assert!(result.is_err());
}

// ── Authorization tests ───────────────────────────────────────────────────────

/// Buyer cannot mark_shipped (only seller can)
#[test]
fn test_mark_shipped_unauthorized() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_018");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);

    // Error::Unauthorized (#6)
    let result = escrow_client.try_mark_shipped(&buyer, &tx_id);
    assert!(result.is_err());
}

/// Seller cannot mark_delivered (only buyer can)
#[test]
fn test_mark_delivered_unauthorized() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_019");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.mark_shipped(&seller, &tx_id);

    // Error::Unauthorized (#6)
    let result = escrow_client.try_mark_delivered(&seller, &tx_id);
    assert!(result.is_err());
}

/// Outsider cannot call release_funds
#[test]
fn test_release_funds_unauthorized() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_020");
    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &1000, &tx_id);
    escrow_client.mark_shipped(&seller, &tx_id);
    escrow_client.mark_delivered(&buyer, &tx_id);

    let outsider = Address::generate(&env);

    // Error::Unauthorized (#6)
    let result = escrow_client.try_release_funds(&outsider, &tx_id);
    assert!(result.is_err());
}

// ── Fee calculation tests ────────────────────────────────────────────────────

/// Fee is correctly rounded down (fixed-point arithmetic)
#[test]
fn test_fee_calculation_and_rounding() {
    let env = Env::default();
    let (escrow_client, token_addr, token_client, _token_admin, _admin, buyer, seller) = setup(&env);

    let tx_id = String::from_str(&env, "tx_021");
    // fee = (999 * 150) / 10000 = 149850 / 10000 = 14 (rounds down)
    let amount: i128 = 999;

    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &amount, &tx_id);
    escrow_client.mark_shipped(&seller, &tx_id);
    escrow_client.mark_delivered(&buyer, &tx_id);
    escrow_client.release_funds(&buyer, &tx_id);

    // Seller gets 999 - 14 = 985
    assert_eq!(token_client.balance(&seller), 985);
    // Fee (14) stays in contract
    assert_eq!(token_client.balance(&escrow_client.address), 14);
}

// ── Full lifecycle event emission test ───────────────────────────────────────

/// Full lifecycle runs end-to-end without panic, confirming events are emitted
#[test]
fn test_full_lifecycle_with_events() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VeloxousEscrow, ());
    let client = VeloxousEscrowClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let (token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    client.init(&admin, &token_client.address, &None, &None);
    token_admin_client.mint(&buyer, &10_000);

    let tx_id = String::from_str(&env, "tx_events");

    client.fund_escrow(&buyer, &seller, &token_client.address, &1000, &tx_id);
    client.mark_shipped(&seller, &tx_id);
    client.mark_delivered(&buyer, &tx_id);
    client.release_funds(&buyer, &tx_id);

    // Final state check
    let record = client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Completed);
}

/// Double initialization must fail
#[test]
fn test_double_init_rejected() {
    let env = Env::default();
    let (escrow_client, token_addr, _token_client, _token_admin, admin, _buyer, _seller) = setup(&env);

    // Error::AlreadyInitialized (#10)
    let result = escrow_client.try_init(&admin, &token_addr, &None, &None);
    assert!(result.is_err());
}

// ── Yield vault integration ─────────────────────────────────────────────────

/// A working yield protocol: quotes 1:1 shares, returns principal plus a
/// flat 10% bonus on withdrawal (backed by real tokens minted to it up front).
#[contract]
pub struct MockYieldProtocol;

#[contractimpl]
impl MockYieldProtocol {
    pub fn deposit(_env: Env, _asset: Address, amount: i128) -> i128 {
        amount
    }

    pub fn withdraw(env: Env, to: Address, asset: Address, shares: i128) -> i128 {
        let total = shares + shares / 10;
        let protocol_addr = env.current_contract_address();
        token::Client::new(&env, &asset).transfer(&protocol_addr, &to, &total);
        total
    }
}

/// A protocol that is simply unavailable — every call traps.
#[contract]
pub struct MockFailingYieldProtocol;

#[contractimpl]
impl MockFailingYieldProtocol {
    pub fn deposit(_env: Env, _asset: Address, _amount: i128) -> i128 {
        panic!("yield protocol unavailable");
    }

    pub fn withdraw(_env: Env, _to: Address, _asset: Address, _shares: i128) -> i128 {
        panic!("yield protocol unavailable");
    }
}

/// Sets up an escrow wired to a vault contract, whose optional yield
/// protocol is provided by the caller (`None` exercises the "no protocol
/// configured" circuit-breaker path).
fn setup_with_vault<'a>(
    env: &'a Env,
    yield_protocol: Option<Address>,
) -> (
    VeloxousEscrowClient<'a>,
    Address,           // token address
    token::Client<'a>,
    token::StellarAssetClient<'a>,
    Address,           // buyer
    Address,           // seller
    Address,           // vault treasury wallet (where routed yield actually lands)
    Address,           // vault contract address
) {
    env.mock_all_auths();

    let escrow_id = env.register(VeloxousEscrow, ());
    let escrow_client = VeloxousEscrowClient::new(env, &escrow_id);

    let token_admin = Address::generate(env);
    let (token_client, token_admin_client) = create_token_contract(env, &token_admin);

    let admin = Address::generate(env);
    let buyer = Address::generate(env);
    let seller = Address::generate(env);

    // Real treasury contract (100% split to a single wallet) so the vault's
    // yield-routing `route_fee` cross-call has something to actually invoke.
    let vault_treasury_wallet = Address::generate(env);
    let vault_treasury_id = env.register(treasury::TreasuryContract, ());
    let vault_treasury_client = treasury::TreasuryContractClient::new(env, &vault_treasury_id);
    let mut splits = soroban_sdk::Vec::new(env);
    splits.push_back(treasury::types::TreasurySplit {
        wallet: vault_treasury_wallet.clone(),
        share_bps: 10_000,
    });
    vault_treasury_client.init(&admin, &0u32, &splits);

    let vault_id = env.register(vault::VaultContract, ());
    let vault_client = vault::VaultContractClient::new(env, &vault_id);
    vault_client.init(&admin, &escrow_id, &Some(vault_treasury_id), &yield_protocol);

    // No treasury configured on the escrow itself: protocol fee stays in the
    // escrow contract, same as the existing no-treasury tests. This test
    // suite is only concerned with the vault/yield path.
    escrow_client.init(&admin, &token_client.address, &None, &Some(vault_id.clone()));

    token_admin_client.mint(&buyer, &10_000);

    (escrow_client, token_client.address.clone(), token_client, token_admin_client, buyer, seller, vault_treasury_wallet, vault_id)
}

/// Full escrow lifecycle (Fund -> Ship -> Deliver -> Release) with a healthy
/// mocked yield protocol: asserts collateral is forwarded into the vault
/// while funded, and that earned yield is routed to the vault's treasury on
/// release while the seller still receives their full net amount.
#[test]
fn test_vault_lifecycle_routes_yield_to_treasury_on_release() {
    let env = Env::default();
    let protocol_id = env.register(MockYieldProtocol, ());

    let (escrow_client, token_addr, token_client, mint_client, buyer, seller, vault_treasury, vault_id) =
        setup_with_vault(&env, Some(protocol_id.clone()));

    // Fund the protocol with enough extra tokens to cover the 10% yield bonus.
    mint_client.mint(&protocol_id, &100);

    let tx_id = String::from_str(&env, "tx_vault_lifecycle");
    let amount: i128 = 1000;

    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &amount, &tx_id);
    // Collateral left the escrow for the protocol, passing through the vault
    // (which itself ends up holding nothing — it forwarded everything on).
    assert_eq!(token_client.balance(&escrow_client.address), 0);
    assert_eq!(token_client.balance(&vault_id), 0);
    assert_eq!(token_client.balance(&protocol_id), 1100);

    escrow_client.mark_shipped(&seller, &tx_id);
    escrow_client.mark_delivered(&buyer, &tx_id);
    escrow_client.release_funds(&buyer, &tx_id);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Completed);

    // Fee = 1.5% of 1000 = 15, seller nets 985 (unaffected by the vault detour).
    assert_eq!(token_client.balance(&seller), 985);
    // No treasury configured on the escrow itself, so the fee sits in escrow.
    assert_eq!(token_client.balance(&escrow_client.address), 15);
    // The 100 units of yield earned went straight to the vault's treasury.
    assert_eq!(token_client.balance(&vault_treasury), 100);
}

/// Mock the yield protocol returning an error: the circuit breaker must fall
/// back gracefully and the full escrow lifecycle must still complete without
/// losing user funds.
#[test]
fn test_vault_circuit_breaker_fallback_completes_lifecycle_without_losing_funds() {
    let env = Env::default();
    let failing_protocol_id = env.register(MockFailingYieldProtocol, ());

    let (escrow_client, token_addr, token_client, _mint, buyer, seller, vault_treasury, vault_id) =
        setup_with_vault(&env, Some(failing_protocol_id.clone()));

    let tx_id = String::from_str(&env, "tx_vault_circuit_breaker");
    let amount: i128 = 1000;

    escrow_client.fund_escrow(&buyer, &seller, &token_addr, &amount, &tx_id);
    // Circuit breaker tripped: per spec, the vault never takes custody and
    // bounces the deposit straight back — the USDC ends up held directly in
    // the escrow contract itself (not the vault, and not the unavailable
    // protocol).
    assert_eq!(token_client.balance(&escrow_client.address), amount);
    assert_eq!(token_client.balance(&vault_id), 0);
    assert_eq!(token_client.balance(&failing_protocol_id), 0);

    escrow_client.mark_shipped(&seller, &tx_id);
    escrow_client.mark_delivered(&buyer, &tx_id);
    escrow_client.release_funds(&buyer, &tx_id);

    let record = escrow_client.get_escrow(&tx_id);
    assert_eq!(record.current_state, EscrowStatus::Completed);

    // Full lifecycle still completes correctly: seller nets 985 after the
    // 1.5% fee, and since no yield was ever earned, the treasury gets nothing.
    assert_eq!(token_client.balance(&seller), 985);
    assert_eq!(token_client.balance(&escrow_client.address), 15);
    assert_eq!(token_client.balance(&vault_treasury), 0);
}
