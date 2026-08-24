use soroban_sdk::{Address, Env, String};
use crate::types::*;

/// Helper function to convert u64 transaction ID to soroban_sdk::String.
pub fn u64_to_string(env: &Env, mut val: u64) -> String {
    if val == 0 {
        return String::from_str(env, "0");
    }
    let mut buf = [0u8; 20];
    let mut pos = 20;
    while val > 0 {
        pos -= 1;
        buf[pos] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    let s = core::str::from_utf8(&buf[pos..]).unwrap();
    String::from_str(env, s)
}

/// Helper check for admin authorization
fn is_admin(env: &Env, address: &Address) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .map(|admin| admin == *address)
        .unwrap_or(false)
}
