use soroban_sdk::{contractevent, Address, Env, String};

use crate::types::ListingState;

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceRegistered {
    #[topic]
    pub listing_id: String,
    pub owner: Address,
    pub valuation: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fractionalized {
    #[topic]
    pub listing_id: String,
    pub owner: Address,
    pub shares: u32,
    pub share_price: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharesTransferred {
    #[topic]
    pub listing_id: String,
    pub from: Address,
    pub to: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuyOut {
    #[topic]
    pub listing_id: String,
    pub buyer: Address,
    pub shares_acquired: i128,
    pub amount_paid: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaleRecorded {
    #[topic]
    pub listing_id: String,
    pub proceeds: i128,
    pub new_state: ListingState,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProceedsClaimed {
    #[topic]
    pub listing_id: String,
    pub holder: Address,
    pub amount: i128,
}

pub fn emit_device_registered(env: &Env, listing_id: String, owner: Address, valuation: i128) {
    DeviceRegistered {
        listing_id,
        owner,
        valuation,
    }
    .publish(env);
}

pub fn emit_fractionalized(
    env: &Env,
    listing_id: String,
    owner: Address,
    shares: u32,
    share_price: i128,
) {
    Fractionalized {
        listing_id,
        owner,
        shares,
        share_price,
    }
    .publish(env);
}

pub fn emit_shares_transferred(
    env: &Env,
    listing_id: String,
    from: Address,
    to: Address,
    amount: i128,
) {
    SharesTransferred {
        listing_id,
        from,
        to,
        amount,
    }
    .publish(env);
}

pub fn emit_buy_out(env: &Env, listing_id: String, buyer: Address, shares_acquired: i128, amount_paid: i128) {
    BuyOut {
        listing_id,
        buyer,
        shares_acquired,
        amount_paid,
    }
    .publish(env);
}

pub fn emit_sale_recorded(env: &Env, listing_id: String, proceeds: i128, new_state: ListingState) {
    SaleRecorded {
        listing_id,
        proceeds,
        new_state,
    }
    .publish(env);
}

pub fn emit_proceeds_claimed(env: &Env, listing_id: String, holder: Address, amount: i128) {
    ProceedsClaimed {
        listing_id,
        holder,
        amount,
    }
    .publish(env);
}
