#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn test_admin_init_and_getters() {
    let env = Env::default();
    let contract_id = env.register(AdminContract, ());
    let client = AdminContractClient::new(&env, &contract_id);

    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);

    let mut admins = Vec::new(&env);
    admins.push_back(admin1.clone());
    admins.push_back(admin2.clone());
    admins.push_back(admin3.clone());

    client.init(&admins, &2);

    assert_eq!(client.get_threshold(), 2);
    assert_eq!(client.get_admins().len(), 3);
    assert!(client.is_admin(&admin1));
    assert!(client.is_admin(&admin2));
    assert!(client.is_admin(&admin3));
    assert!(!client.is_admin(&Address::generate(&env)));
}

#[test]
#[should_panic(expected = "invalid threshold")]
fn test_admin_invalid_threshold() {
    let env = Env::default();
    let contract_id = env.register(AdminContract, ());
    let client = AdminContractClient::new(&env, &contract_id);

    let admin1 = Address::generate(&env);
    let mut admins = Vec::new(&env);
    admins.push_back(admin1);

    client.init(&admins, &2);
}

#[test]
fn test_admin_rotation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AdminContract, ());
    let client = AdminContractClient::new(&env, &contract_id);

    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);

    let mut admins = Vec::new(&env);
    admins.push_back(admin1.clone());
    admins.push_back(admin2.clone());
    admins.push_back(admin3.clone());

    client.init(&admins, &2);

    let new_admin = Address::generate(&env);

    // Vote 1
    client.propose_admin_change(&admin1, &admin3, &new_admin);
    assert!(client.is_admin(&admin3)); // not changed yet

    // Vote 2 (threshold reached)
    client.propose_admin_change(&admin2, &admin3, &new_admin);

    assert!(!client.is_admin(&admin3));
    assert!(client.is_admin(&new_admin));
}
