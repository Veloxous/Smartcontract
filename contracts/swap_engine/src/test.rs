#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as TokenAdminClient;

// Mock Oracle
use interfaces::oracle::OracleTrait;
use soroban_sdk::contractimpl;

#[soroban_sdk::contract]
pub struct MockOracle;

#[contractimpl]
impl OracleTrait for MockOracle {
    fn get_price(_env: Env, _asset: Address) -> i128 {
        1000 * 10_000_000 // 1000 USD
    }
}

fn setup() -> (Env, SwapEngineClient<'static>, Address, TokenClient<'static>, Address, Address, Address, Address, Address, TokenAdminClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let party_a = Address::generate(&env);
    let party_b = Address::generate(&env);
    
    let device_a = Address::generate(&env);
    let device_b = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let usdc_addr = env.register_stellar_asset_contract_v2(token_admin.clone());
    let usdc = TokenClient::new(&env, &usdc_addr.address());
    let usdc_admin = TokenAdminClient::new(&env, &usdc_addr.address());

    let oracle_id = env.register_contract(None, MockOracle);

    let contract_id = env.register_contract(None, SwapEngine);
    let client = SwapEngineClient::new(&env, &contract_id);
    
    client.initialize(&admin, &admin, &usdc.address, &oracle_id);

    (env, client, admin, usdc, party_a, party_b, device_a, device_b, oracle_id, usdc_admin)
}

#[test]
fn test_happy_path() {
    let (env, client, _admin, usdc, party_a, party_b, device_a, device_b, _, usdc_admin) = setup();

    // Mint USDC
    usdc_admin.mint(&party_a, &(1000 * 10_000_000));
    usdc_admin.mint(&party_b, &(1000 * 10_000_000));

    let swap_id = client.propose_swap(&party_a, &party_b, &device_a, &device_b);
    
    client.deposit_collateral(&swap_id, &party_a);
    assert_eq!(usdc.balance(&party_a), 0);
    
    client.deposit_collateral(&swap_id, &party_b);
    assert_eq!(usdc.balance(&party_b), 0);
    
    assert_eq!(usdc.balance(&client.address), 2000 * 10_000_000);

    client.confirm_receipt(&swap_id, &party_a);
    client.confirm_receipt(&swap_id, &party_b);

    assert_eq!(usdc.balance(&party_a), 1000 * 10_000_000);
    assert_eq!(usdc.balance(&party_b), 1000 * 10_000_000);
    assert_eq!(usdc.balance(&client.address), 0);
}

#[test]
fn test_dispute_and_slash() {
    let (env, client, admin, usdc, party_a, party_b, device_a, device_b, _, usdc_admin) = setup();

    // Mint USDC
    usdc_admin.mint(&party_a, &(1000 * 10_000_000));
    usdc_admin.mint(&party_b, &(1000 * 10_000_000));

    let swap_id = client.propose_swap(&party_a, &party_b, &device_a, &device_b);
    client.deposit_collateral(&swap_id, &party_a);
    client.deposit_collateral(&swap_id, &party_b);
    
    client.raise_dispute(&swap_id, &party_a);

    // Admin resolves in favor of party_a
    client.resolve_swap_dispute(&swap_id, &party_a);

    assert_eq!(usdc.balance(&party_a), 2000 * 10_000_000);
    assert_eq!(usdc.balance(&party_b), 0);
    assert_eq!(usdc.balance(&client.address), 0);
}

#[test]
fn test_timeout() {
    let (env, client, _admin, usdc, party_a, party_b, device_a, device_b, _, usdc_admin) = setup();

    // Mint USDC
    usdc_admin.mint(&party_a, &(1000 * 10_000_000));

    let swap_id = client.propose_swap(&party_a, &party_b, &device_a, &device_b);
    client.deposit_collateral(&swap_id, &party_a);

    // Fast forward 49 hours
    env.ledger().set_timestamp(49 * 60 * 60);

    client.withdraw_timeout(&swap_id, &party_a);

    assert_eq!(usdc.balance(&party_a), 1000 * 10_000_000);
    assert_eq!(usdc.balance(&client.address), 0);
}
