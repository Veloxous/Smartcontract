use soroban_sdk::{contracterror, contracttype, panic_with_error, Address, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AuthError {
    Unauthorized = 2,
    AlreadyInitialized = 3,
    NotInitialized = 4,
}

#[contracttype]
pub enum AuthDataKey {
    Admin,
    AuthorizedContract(Address),
}

pub fn init_admin(env: &Env, admin: &Address) {
    if env.storage().instance().has(&AuthDataKey::Admin) {
        panic_with_error!(env, AuthError::AlreadyInitialized);
    }
    env.storage().instance().set(&AuthDataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&AuthDataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, AuthError::NotInitialized))
}

pub fn require_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    let admin = get_admin(env);
    if *caller != admin {
        panic_with_error!(env, AuthError::Unauthorized);
    }
}

pub fn add_authorized_contract(env: &Env, admin: &Address, contract: &Address) {
    require_admin(env, admin);
    env.storage()
        .instance()
        .set(&AuthDataKey::AuthorizedContract(contract.clone()), &true);
}

pub fn remove_authorized_contract(env: &Env, admin: &Address, contract: &Address) {
    require_admin(env, admin);
    env.storage()
        .instance()
        .remove(&AuthDataKey::AuthorizedContract(contract.clone()));
}

pub fn is_authorized_contract(env: &Env, contract: &Address) -> bool {
    env.storage()
        .instance()
        .get(&AuthDataKey::AuthorizedContract(contract.clone()))
        .unwrap_or(false)
}

/// require_auth() on the caller + whitelist check, in one call.
pub fn require_authorized_contract(env: &Env, caller: &Address) {
    caller.require_auth();
    if !is_authorized_contract(env, caller) {
        panic_with_error!(env, AuthError::Unauthorized);
    }
}