use soroban_sdk::{token, Address, Env, String, Vec};

use crate::events::{
    emit_buy_out, emit_fractionalized, emit_proceeds_claimed, emit_sale_recorded,
    emit_shares_transferred,
};
use crate::types::{DataKey, Error, FractionListing, ListingState};

pub fn require_initialized(env: &Env) -> Result<(), Error> {
    if env.storage().instance().has(&DataKey::Initialized) {
        Ok(())
    } else {
        Err(Error::NotInitialized)
    }
}

pub fn get_listing(env: &Env, listing_id: &String) -> Result<FractionListing, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Listing(listing_id.clone()))
        .ok_or(Error::NotFound)
}

pub fn set_listing(env: &Env, listing_id: &String, listing: &FractionListing) {
    env.storage()
        .persistent()
        .set(&DataKey::Listing(listing_id.clone()), listing);
}

pub fn share_balance(env: &Env, listing_id: &String, holder: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::ShareBalance(listing_id.clone(), holder.clone()))
        .unwrap_or(0)
}

fn set_share_balance(env: &Env, listing_id: &String, holder: &Address, amount: i128) {
    if amount == 0 {
        env.storage()
            .persistent()
            .remove(&DataKey::ShareBalance(listing_id.clone(), holder.clone()));
    } else {
        env.storage().persistent().set(
            &DataKey::ShareBalance(listing_id.clone(), holder.clone()),
            &amount,
        );
    }
}

fn claimed_amount(env: &Env, listing_id: &String, holder: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Claimed(listing_id.clone(), holder.clone()))
        .unwrap_or(0)
}

fn set_claimed_amount(env: &Env, listing_id: &String, holder: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::Claimed(listing_id.clone(), holder.clone()), &amount);
}

fn get_holders(env: &Env, listing_id: &String) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::Holders(listing_id.clone()))
        .unwrap_or(Vec::new(env))
}

fn set_holders(env: &Env, listing_id: &String, holders: &Vec<Address>) {
    env.storage()
        .persistent()
        .set(&DataKey::Holders(listing_id.clone()), holders);
}

fn track_holder(env: &Env, listing_id: &String, holder: &Address) {
    let mut holders = get_holders(env, listing_id);
    for i in 0..holders.len() {
        if holders.get(i).unwrap() == *holder {
            return;
        }
    }
    holders.push_back(holder.clone());
    set_holders(env, listing_id, &holders);
}

fn refresh_holder(env: &Env, listing_id: &String, holder: &Address) {
    if share_balance(env, listing_id, holder) == 0 {
        let holders = get_holders(env, listing_id);
        let mut updated = Vec::new(env);
        for i in 0..holders.len() {
            let addr = holders.get(i).unwrap();
            if addr != *holder {
                updated.push_back(addr);
            }
        }
        set_holders(env, listing_id, &updated);
    }
}

pub fn fractionalize(
    env: &Env,
    owner: Address,
    listing_id: String,
    shares: u32,
) -> Result<(), Error> {
    require_initialized(env)?;
    owner.require_auth();

    if shares == 0 {
        return Err(Error::InvalidShares);
    }

    if env
        .storage()
        .persistent()
        .has(&DataKey::Listing(listing_id.clone()))
    {
        return Err(Error::AlreadyFractionalized);
    }

    let device_owner: Address = env
        .storage()
        .persistent()
        .get(&DataKey::DeviceOwner(listing_id.clone()))
        .ok_or(Error::NotFound)?;

    if device_owner != owner {
        return Err(Error::NotOwner);
    }

    let valuation: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::DeviceValuation(listing_id.clone()))
        .ok_or(Error::NotFound)?;

    let share_price = valuation
        .checked_div(shares as i128)
        .ok_or(Error::InvalidAmount)?;

    if share_price <= 0 {
        return Err(Error::InvalidAmount);
    }

    let contract_address = env.current_contract_address();
    env.storage()
        .persistent()
        .set(&DataKey::DeviceOwner(listing_id.clone()), &contract_address);

    let listing = FractionListing {
        owner: owner.clone(),
        total_shares: shares,
        share_price,
        sale_proceeds: 0,
        state: ListingState::Active,
    };
    set_listing(env, &listing_id, &listing);

    set_share_balance(env, &listing_id, &owner, shares as i128);
    track_holder(env, &listing_id, &owner);

    emit_fractionalized(env, listing_id, owner, shares, share_price);
    Ok(())
}

pub fn transfer_shares(
    env: &Env,
    from: Address,
    to: Address,
    listing_id: String,
    amount: i128,
) -> Result<(), Error> {
    require_initialized(env)?;
    from.require_auth();

    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }

    let listing = get_listing(env, &listing_id)?;
    if listing.state != ListingState::Active {
        return Err(Error::InvalidState);
    }

    let from_balance = share_balance(env, &listing_id, &from);
    if from_balance < amount {
        return Err(Error::InsufficientShares);
    }

    set_share_balance(env, &listing_id, &from, from_balance - amount);
    refresh_holder(env, &listing_id, &from);

    set_share_balance(
        env,
        &listing_id,
        &to,
        share_balance(env, &listing_id, &to) + amount,
    );
    track_holder(env, &listing_id, &to);

    emit_shares_transferred(env, listing_id, from, to, amount);
    Ok(())
}

pub fn buy_out(env: &Env, buyer: Address, listing_id: String) -> Result<(), Error> {
    require_initialized(env)?;
    buyer.require_auth();

    let listing = get_listing(env, &listing_id)?;
    if listing.state != ListingState::Active {
        return Err(Error::InvalidState);
    }

    let usdc: Address = env
        .storage()
        .instance()
        .get(&DataKey::UsdcAsset)
        .ok_or(Error::NotInitialized)?;

    let buyer_balance = share_balance(env, &listing_id, &buyer);
    let total_outstanding = (listing.total_shares as i128) - buyer_balance;

    if total_outstanding <= 0 {
        return Err(Error::InsufficientShares);
    }

    let amount_due = listing
        .share_price
        .checked_mul(total_outstanding)
        .ok_or(Error::Overflow)?;

    let token_client = token::Client::new(env, &usdc);
    token_client.transfer(&buyer, &env.current_contract_address(), &amount_due);

    let holders = get_holders(env, &listing_id);
    for i in 0..holders.len() {
        let holder = holders.get(i).unwrap();
        if holder == buyer {
            continue;
        }
        let balance = share_balance(env, &listing_id, &holder);
        if balance <= 0 {
            continue;
        }
        let payout = listing
            .share_price
            .checked_mul(balance)
            .ok_or(Error::Overflow)?;
        token_client.transfer(&env.current_contract_address(), &holder, &payout);
        set_share_balance(env, &listing_id, &holder, 0);
        refresh_holder(env, &listing_id, &holder);
    }

    set_share_balance(env, &listing_id, &buyer, listing.total_shares as i128);
    track_holder(env, &listing_id, &buyer);

    env.storage()
        .persistent()
        .set(&DataKey::DeviceOwner(listing_id.clone()), &buyer);

    let updated = FractionListing {
        state: ListingState::BoughtOut,
        ..listing.clone()
    };
    set_listing(env, &listing_id, &updated);

    emit_buy_out(
        env,
        listing_id,
        buyer,
        listing.total_shares as i128,
        amount_due,
    );
    Ok(())
}

pub fn record_sale(
    env: &Env,
    caller: Address,
    listing_id: String,
    proceeds: i128,
) -> Result<(), Error> {
    require_initialized(env)?;
    caller.require_auth();

    if proceeds <= 0 {
        return Err(Error::InvalidAmount);
    }

    let marketplace: Address = env
        .storage()
        .instance()
        .get(&DataKey::Marketplace)
        .ok_or(Error::Unauthorized)?;

    if caller != marketplace {
        return Err(Error::Unauthorized);
    }

    let mut listing = get_listing(env, &listing_id)?;
    if listing.state != ListingState::Active {
        return Err(Error::InvalidState);
    }

    let usdc: Address = env
        .storage()
        .instance()
        .get(&DataKey::UsdcAsset)
        .ok_or(Error::NotInitialized)?;

    let token_client = token::Client::new(env, &usdc);
    token_client.transfer(&caller, &env.current_contract_address(), &proceeds);

    listing.sale_proceeds = proceeds;
    listing.state = ListingState::Sold;
    set_listing(env, &listing_id, &listing);

    emit_sale_recorded(env, listing_id, proceeds, ListingState::Sold);
    Ok(())
}

pub fn claim_proceeds(env: &Env, holder: Address, listing_id: String) -> Result<i128, Error> {
    require_initialized(env)?;
    holder.require_auth();

    let listing = get_listing(env, &listing_id)?;
    if listing.state != ListingState::Sold {
        return Err(Error::InvalidState);
    }

    if listing.sale_proceeds <= 0 {
        return Err(Error::InvalidAmount);
    }

    let balance = share_balance(env, &listing_id, &holder);
    if balance <= 0 {
        return Err(Error::InsufficientShares);
    }

    let total_entitled = listing
        .sale_proceeds
        .checked_mul(balance)
        .ok_or(Error::Overflow)?
        .checked_div(listing.total_shares as i128)
        .ok_or(Error::InvalidAmount)?;

    let already_claimed = claimed_amount(env, &listing_id, &holder);
    if already_claimed >= total_entitled {
        return Err(Error::AlreadyClaimed);
    }

    let payout = total_entitled - already_claimed;
    if payout <= 0 {
        return Err(Error::AlreadyClaimed);
    }

    let usdc: Address = env
        .storage()
        .instance()
        .get(&DataKey::UsdcAsset)
        .ok_or(Error::NotInitialized)?;

    let token_client = token::Client::new(env, &usdc);
    token_client.transfer(&env.current_contract_address(), &holder, &payout);

    set_claimed_amount(env, &listing_id, &holder, total_entitled);

    emit_proceeds_claimed(env, listing_id, holder, payout);
    Ok(payout)
}

/// Pull-based proceeds distribution alias required by the issue spec.
pub fn distribute_proceeds(env: &Env, holder: Address, listing_id: String) -> Result<i128, Error> {
    claim_proceeds(env, holder, listing_id)
}

pub fn register_device(
    env: &Env,
    owner: Address,
    listing_id: String,
    valuation: i128,
) -> Result<(), Error> {
    require_initialized(env)?;
    owner.require_auth();

    if valuation <= 0 {
        return Err(Error::InvalidAmount);
    }

    if env
        .storage()
        .persistent()
        .has(&DataKey::DeviceOwner(listing_id.clone()))
    {
        return Err(Error::AlreadyExists);
    }

    env.storage()
        .persistent()
        .set(&DataKey::DeviceOwner(listing_id.clone()), &owner);
    env.storage()
        .persistent()
        .set(&DataKey::DeviceValuation(listing_id.clone()), &valuation);

    crate::events::emit_device_registered(env, listing_id, owner, valuation);
    Ok(())
}

pub fn get_share_balance(env: &Env, listing_id: String, holder: Address) -> i128 {
    share_balance(env, &listing_id, &holder)
}

pub fn get_listing_state(env: &Env, listing_id: String) -> Result<FractionListing, Error> {
    get_listing(env, &listing_id)
}
