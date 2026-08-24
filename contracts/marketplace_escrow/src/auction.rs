//! Dutch Auction implementation for high-value device listings.
//!
//! # Price Decay Formula
//! ```text
//! current_price = start_price - ((start_price - end_price) * elapsed / duration)
//! ```
//! All arithmetic is integer `i128`. The result is clamped to `[end_price, start_price]`
//! so boundary conditions (t=0 and t>=duration) are handled safely.
//!
//! # Lifecycle
//! `Active` → (buyer calls `buy_now`) → `Sold`
//! `Active` → (duration elapsed, `cancel_expired` called) → `Expired`

use soroban_sdk::{token, Address, Env, String};

use crate::events::{emit_auction_bought, emit_auction_created, emit_auction_expired};
use crate::types::{AuctionState, AuctionStatus, DataKey};

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn get_auction(env: &Env, listing_id: &String) -> AuctionState {
    env.storage()
        .persistent()
        .get(&DataKey::Auction(listing_id.clone()))
        .unwrap_or_else(|| panic!("auction not found"))
}

fn set_auction(env: &Env, listing_id: &String, state: &AuctionState) {
    env.storage()
        .persistent()
        .set(&DataKey::Auction(listing_id.clone()), state);
}

// ---------------------------------------------------------------------------
// Price calculation
// ---------------------------------------------------------------------------

/// Compute the current Dutch-auction price at a given `now` timestamp.
///
/// Formula: `start_price - ((start_price - end_price) * elapsed / duration)`
///
/// Returns `start_price` if `elapsed == 0`, and `end_price` if `elapsed >= duration`.
/// All intermediate arithmetic uses `i128` with checked operations to avoid overflow.
pub fn compute_current_price(
    start_price: i128,
    end_price: i128,
    start_time: u64,
    duration_secs: u64,
    now: u64,
) -> i128 {
    if now <= start_time {
        return start_price;
    }

    let elapsed = now - start_time;

    if elapsed >= duration_secs {
        return end_price;
    }

    let price_drop = start_price - end_price; // guaranteed >= 0 (validated at creation)
    let decay = price_drop
        .checked_mul(elapsed as i128)
        .unwrap_or(i128::MAX)
        .checked_div(duration_secs as i128)
        .unwrap_or(0);

    let price = start_price - decay;

    // Clamp defensively.
    price.max(end_price).min(start_price)
}

// ---------------------------------------------------------------------------
// Public entrypoints (called from lib.rs)
// ---------------------------------------------------------------------------

/// Create a Dutch auction for `listing_id`.
///
/// * `listing_id` — unique identifier for the device listing.
/// * `seller` — owner of the listing; must authorise this call.
/// * `usdc_asset` — token contract address used for payment.
/// * `start_price` — initial (highest) price.
/// * `end_price` — floor price (must be <= start_price and > 0).
/// * `duration_secs` — seconds over which price decays to `end_price`.
pub fn create_auction(
    env: &Env,
    listing_id: String,
    seller: Address,
    usdc_asset: Address,
    start_price: i128,
    end_price: i128,
    duration_secs: u64,
) {
    seller.require_auth();

    if start_price <= 0 {
        panic!("start_price must be positive");
    }
    if end_price <= 0 {
        panic!("end_price must be positive");
    }
    if end_price > start_price {
        panic!("end_price must be <= start_price");
    }
    if duration_secs == 0 {
        panic!("duration_secs must be > 0");
    }

    // Reject if an active auction already exists for this listing.
    if let Some(existing) = env
        .storage()
        .persistent()
        .get::<DataKey, AuctionState>(&DataKey::Auction(listing_id.clone()))
    {
        if existing.status == AuctionStatus::Active {
            panic!("auction already active for this listing");
        }
    }

    let now = env.ledger().timestamp();

    let state = AuctionState {
        seller: seller.clone(),
        usdc_asset: usdc_asset.clone(),
        start_price,
        end_price,
        start_time: now,
        duration_secs,
        status: AuctionStatus::Active,
        buyer: None,
        final_price: None,
    };

    set_auction(env, &listing_id, &state);

    emit_auction_created(env, listing_id, seller, usdc_asset, start_price, end_price, duration_secs, now);
}

/// Read the current price of an active Dutch auction without modifying state.
///
/// Panics if the auction does not exist.
pub fn get_current_price(env: &Env, listing_id: String) -> i128 {
    let state = get_auction(env, &listing_id);
    compute_current_price(
        state.start_price,
        state.end_price,
        state.start_time,
        state.duration_secs,
        env.ledger().timestamp(),
    )
}

/// Attempt to purchase the listing at the current Dutch-auction price.
///
/// * Panics with `"AuctionExpired"` if the auction duration has elapsed.
/// * Panics if the auction is not in `Active` state.
/// * Captures the **exact** price at the moment of the call and transfers
///   `price` USDC from `buyer` → this contract (escrow).
/// * Marks the auction `Sold` and records the buyer + final price.
///
/// The locked funds sit in the contract's balance and can be released / disputed
/// via the standard escrow `release` / `raise_dispute` flow.
pub fn buy_now(env: &Env, listing_id: String, buyer: Address) {
    buyer.require_auth();

    let mut state = get_auction(env, &listing_id);

    if state.status != AuctionStatus::Active {
        panic!("auction is not active");
    }

    let now = env.ledger().timestamp();

    // Expiry check — automatically return listing to available state.
    if now >= state.start_time + state.duration_secs {
        // Mark expired so callers get a clear error.
        state.status = AuctionStatus::Expired;
        set_auction(env, &listing_id, &state);
        emit_auction_expired(env, listing_id, state.start_time + state.duration_secs);
        panic!("AuctionExpired");
    }

    // Capture the current price at this exact moment.
    let price = compute_current_price(
        state.start_price,
        state.end_price,
        state.start_time,
        state.duration_secs,
        now,
    );

    // Transfer USDC from buyer → contract (acts as escrow lock).
    let token_client = token::Client::new(env, &state.usdc_asset);
    token_client.transfer(&buyer, &env.current_contract_address(), &price);

    // Finalise auction state.
    state.status = AuctionStatus::Sold;
    state.buyer = Some(buyer.clone());
    state.final_price = Some(price);
    set_auction(env, &listing_id, &state);

    emit_auction_bought(env, listing_id, buyer, price, now);
}

/// Expire an auction whose duration has elapsed without a buyer.
///
/// Callable by anyone (permissionless), reverts the listing to an available
/// state by marking the auction `Expired`.
pub fn cancel_expired(env: &Env, listing_id: String) {
    let mut state = get_auction(env, &listing_id);

    if state.status != AuctionStatus::Active {
        panic!("auction is not active");
    }

    let deadline = state.start_time + state.duration_secs;
    if env.ledger().timestamp() < deadline {
        panic!("auction has not expired yet");
    }

    state.status = AuctionStatus::Expired;
    set_auction(env, &listing_id, &state);

    emit_auction_expired(env, listing_id, deadline);
}

/// Read the full auction state for a listing. Panics if no auction exists.
pub fn get_auction_state(env: &Env, listing_id: String) -> AuctionState {
    get_auction(env, &listing_id)
}
