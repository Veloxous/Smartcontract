#![cfg(test)]

use super::*;
use soroban_sdk::{contract, contractimpl, testutils::Address as _, token, Address, Env, String, Vec};

// ── Mock yield protocols ────────────────────────────────────────────────────

/// A working yield protocol: quotes 1:1 shares and, on withdrawal, returns
/// principal plus a flat 10% yield bonus (the mock is pre-funded with the
/// bonus amount by the test so the payout is backed by real tokens).
#[contract]
pub struct MockYieldProtocol;

#[contractimpl]
impl MockYieldProtocol {
    pub fn deposit(_env: Env, _asset: Address, amount: i128) -> i128 {
        amount
    }

    pub fn withdraw(env: Env, to: Address, asset: Address, shares: i128) -> i128 {
        let bonus = shares / 10; // flat 10% simulated yield
        let total = shares + bonus;
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

/// Sets up a vault whose "escrow" caller is just a generated address (no
/// real escrow contract needed to exercise the vault in isolation), with an
/// optional yield protocol wired in, and a real `treasury` contract instance
/// (100% split to a single wallet) so `withdraw`'s `route_fee` cross-call has
/// something to actually invoke, matching how `veloxous_escrow` already
/// routes its own protocol fee.
fn setup<'a>(
    env: &'a Env,
    yield_protocol: Option<Address>,
) -> (
    VaultContractClient<'a>,
    Address,          // token address
    token::Client<'a>,
    token::StellarAssetClient<'a>,
    Address,           // admin
    Address,           // escrow (caller)
    Address,           // treasury wallet (where routed yield actually lands)
) {
    env.mock_all_auths();

    let contract_id = env.register(VaultContract, ());
    let client = VaultContractClient::new(env, &contract_id);

    let token_admin = Address::generate(env);
    let (token_client, token_admin_client) = create_token_contract(env, &token_admin);

    let admin = Address::generate(env);
    let escrow = Address::generate(env);

    let treasury_wallet = Address::generate(env);
    let treasury_id = env.register(treasury::TreasuryContract, ());
    let treasury_client = treasury::TreasuryContractClient::new(env, &treasury_id);
    let mut splits = Vec::new(env);
    splits.push_back(treasury::types::TreasurySplit {
        wallet: treasury_wallet.clone(),
        share_bps: 10_000,
    });
    treasury_client.init(&admin, &0u32, &splits);

    client.init(&admin, &escrow, &Some(treasury_id), &yield_protocol);

    // Fund the "escrow" so it can push deposits into the vault, mirroring
    // how the real escrow contract holds buyer funds before forwarding them.
    token_admin_client.mint(&escrow, &10_000);

    (client, token_client.address.clone(), token_client, token_admin_client, admin, escrow, treasury_wallet)
}

// ── Deposit / withdraw without a yield protocol ─────────────────────────────

#[test]
fn test_deposit_and_withdraw_no_yield_protocol() {
    let env = Env::default();
    let (client, token_addr, token_client, _mint, _admin, escrow, _treasury) = setup(&env, None);

    let tx_id = String::from_str(&env, "tx_vault_001");
    token_client.transfer(&escrow, &client.address, &1000);
    client.deposit(&escrow, &token_addr, &1000, &tx_id);

    let record = client.get_vault_record(&tx_id);
    assert_eq!(record.principal, 1000);
    assert!(!record.in_yield_protocol);
    assert!(!record.withdrawn);
    // No protocol configured: this vault never takes custody, it bounces the
    // deposit straight back — the escrow ends up holding its own collateral.
    assert_eq!(token_client.balance(&client.address), 0);
    assert_eq!(token_client.balance(&escrow), 10_000);

    let (principal, yield_earned) = client.withdraw(&escrow, &tx_id);
    assert_eq!(principal, 1000);
    assert_eq!(yield_earned, 0);
    assert_eq!(token_client.balance(&escrow), 10_000); // unchanged, still held by escrow
    assert_eq!(token_client.balance(&client.address), 0);

    let record = client.get_vault_record(&tx_id);
    assert!(record.withdrawn);
}

// ── Deposit / withdraw with a healthy yield protocol ────────────────────────

#[test]
fn test_deposit_routes_into_yield_protocol_and_yield_goes_to_treasury() {
    let env = Env::default();
    let protocol_id = env.register(MockYieldProtocol, ());

    let (client, token_addr, token_client, mint_client, _admin, escrow, treasury) =
        setup(&env, Some(protocol_id.clone()));

    // Fund the protocol with enough extra tokens to cover the 10% yield bonus.
    mint_client.mint(&protocol_id, &100);

    let tx_id = String::from_str(&env, "tx_vault_002");
    token_client.transfer(&escrow, &client.address, &1000);
    client.deposit(&escrow, &token_addr, &1000, &tx_id);

    let record = client.get_vault_record(&tx_id);
    assert!(record.in_yield_protocol);
    assert_eq!(record.shares, 1000);
    // Funds were forwarded to the protocol, not held in the vault.
    assert_eq!(token_client.balance(&client.address), 0);
    assert_eq!(token_client.balance(&protocol_id), 1100);

    let (principal, yield_earned) = client.withdraw(&escrow, &tx_id);
    assert_eq!(principal, 1000);
    assert_eq!(yield_earned, 100);

    // Principal back to escrow, yield routed straight to treasury.
    assert_eq!(token_client.balance(&escrow), 10_000);
    assert_eq!(token_client.balance(&treasury), 100);
    assert_eq!(token_client.balance(&client.address), 0);
}

// ── Circuit breaker ──────────────────────────────────────────────────────────

/// Mock the yield protocol returning an error (trapping): the circuit
/// breaker must fall back gracefully without losing user funds.
#[test]
fn test_circuit_breaker_falls_back_when_yield_protocol_fails() {
    let env = Env::default();
    let failing_protocol_id = env.register(MockFailingYieldProtocol, ());

    let (client, token_addr, token_client, _mint, _admin, escrow, _treasury) =
        setup(&env, Some(failing_protocol_id.clone()));

    let tx_id = String::from_str(&env, "tx_vault_003");
    token_client.transfer(&escrow, &client.address, &1000);

    // Deposit must succeed despite the protocol being unavailable.
    client.deposit(&escrow, &token_addr, &1000, &tx_id);

    let record = client.get_vault_record(&tx_id);
    assert!(!record.in_yield_protocol);
    assert_eq!(record.principal, 1000);
    // The circuit breaker bounced the deposit straight back — this vault
    // never took custody, and the escrow ends up holding its own collateral
    // directly (per spec), not this vault.
    assert_eq!(token_client.balance(&client.address), 0);
    assert_eq!(token_client.balance(&escrow), 10_000);
    assert_eq!(token_client.balance(&failing_protocol_id), 0);

    // Withdrawal still works normally, with zero yield, since funds were
    // never actually exposed to the protocol.
    let (principal, yield_earned) = client.withdraw(&escrow, &tx_id);
    assert_eq!(principal, 1000);
    assert_eq!(yield_earned, 0);
    assert_eq!(token_client.balance(&escrow), 10_000);
}

#[test]
fn test_no_yield_protocol_configured_behaves_like_circuit_breaker() {
    let env = Env::default();
    let (client, token_addr, token_client, _mint, _admin, escrow, _treasury) = setup(&env, None);

    let tx_id = String::from_str(&env, "tx_vault_004");
    token_client.transfer(&escrow, &client.address, &500);
    client.deposit(&escrow, &token_addr, &500, &tx_id);

    let record = client.get_vault_record(&tx_id);
    assert!(!record.in_yield_protocol);
    assert_eq!(record.principal, 500);
    // Bounced straight back: escrow holds its own collateral, vault holds nothing.
    assert_eq!(token_client.balance(&client.address), 0);
    assert_eq!(token_client.balance(&escrow), 10_000);
}

// ── Guard rails ───────────────────────────────────────────────────────────

#[test]
fn test_deposit_rejects_non_escrow_caller() {
    let env = Env::default();
    let (client, token_addr, token_client, _mint, _admin, escrow, _treasury) = setup(&env, None);

    let stranger = Address::generate(&env);
    let tx_id = String::from_str(&env, "tx_vault_005");
    token_client.transfer(&escrow, &stranger, &200);

    let result = client.try_deposit(&stranger, &token_addr, &200, &tx_id);
    assert!(result.is_err());
}

#[test]
fn test_deposit_rejects_duplicate_transaction_id() {
    let env = Env::default();
    let (client, token_addr, token_client, _mint, _admin, escrow, _treasury) = setup(&env, None);

    let tx_id = String::from_str(&env, "tx_vault_006");
    token_client.transfer(&escrow, &client.address, &600);
    client.deposit(&escrow, &token_addr, &300, &tx_id);

    let result = client.try_deposit(&escrow, &token_addr, &300, &tx_id);
    assert!(result.is_err());
}

#[test]
fn test_withdraw_rejects_double_withdrawal() {
    let env = Env::default();
    let (client, token_addr, token_client, _mint, _admin, escrow, _treasury) = setup(&env, None);

    let tx_id = String::from_str(&env, "tx_vault_007");
    token_client.transfer(&escrow, &client.address, &400);
    client.deposit(&escrow, &token_addr, &400, &tx_id);
    client.withdraw(&escrow, &tx_id);

    let result = client.try_withdraw(&escrow, &tx_id);
    assert!(result.is_err());
}

#[test]
fn test_withdraw_rejects_unknown_transaction_id() {
    let env = Env::default();
    let (client, _token_addr, _token_client, _mint, _admin, escrow, _treasury) = setup(&env, None);

    let tx_id = String::from_str(&env, "tx_vault_does_not_exist");
    let result = client.try_withdraw(&escrow, &tx_id);
    assert!(result.is_err());
}
