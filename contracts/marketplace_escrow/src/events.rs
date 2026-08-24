use soroban_sdk::{contractevent, Address, BytesN, Env, String};

/// Event payload emitted when a dispute is raised by buyer or seller.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeRaised {
    #[topic]
    pub transaction_id: String,
    pub buyer: Address,
    pub seller: Address,
    pub reason_hash: BytesN<32>,
    pub timestamp: u64,
}

/// Event payload emitted when a resolution proposal is submitted by an admin.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionProposed {
    #[topic]
    pub transaction_id: String,
    pub proposer: Address,
    pub buyer_refund_amount: i128,
    pub seller_payout_amount: i128,
}

/// Event payload emitted when an admin votes on a resolution proposal.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionVoteCast {
    #[topic]
    pub transaction_id: String,
    pub admin: Address,
    pub buyer_refund_amount: i128,
    pub seller_payout_amount: i128,
    pub total_votes: u32,
}

/// Event payload emitted when a resolution reaches threshold and is executed.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionExecuted {
    #[topic]
    pub transaction_id: String,
    pub buyer_refund_amount: i128,
    pub seller_payout_amount: i128,
    pub timestamp: u64,
}

/// Event payload emitted when an admin rotation is proposed.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChangeProposed {
    #[topic]
    pub old_admin: Address,
    #[topic]
    pub new_admin: Address,
    pub proposer: Address,
}

/// Event payload emitted when an admin rotation is executed.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChanged {
    #[topic]
    pub old_admin: Address,
    #[topic]
    pub new_admin: Address,
    pub timestamp: u64,
}

/// Event payload emitted when accumulated fees are collected / swept.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeCollected {
    #[topic]
    pub source_contract: Address,
    #[topic]
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

pub fn emit_dispute_raised(
    env: &Env,
    transaction_id: String,
    buyer: Address,
    seller: Address,
    reason_hash: BytesN<32>,
    timestamp: u64,
) {
    DisputeRaised {
        transaction_id,
        buyer,
        seller,
        reason_hash,
        timestamp,
    }
    .publish(env);
}

pub fn emit_resolution_proposed(
    env: &Env,
    transaction_id: String,
    proposer: Address,
    buyer_refund_amount: i128,
    seller_payout_amount: i128,
) {
    ResolutionProposed {
        transaction_id,
        proposer,
        buyer_refund_amount,
        seller_payout_amount,
    }
    .publish(env);
}

pub fn emit_resolution_vote_cast(
    env: &Env,
    transaction_id: String,
    admin: Address,
    buyer_refund_amount: i128,
    seller_payout_amount: i128,
    total_votes: u32,
) {
    ResolutionVoteCast {
        transaction_id,
        admin,
        buyer_refund_amount,
        seller_payout_amount,
        total_votes,
    }
    .publish(env);
}

pub fn emit_resolution_executed(
    env: &Env,
    transaction_id: String,
    buyer_refund_amount: i128,
    seller_payout_amount: i128,
    timestamp: u64,
) {
    ResolutionExecuted {
        transaction_id,
        buyer_refund_amount,
        seller_payout_amount,
        timestamp,
    }
    .publish(env);
}

pub fn emit_admin_change_proposed(
    env: &Env,
    old_admin: Address,
    new_admin: Address,
    proposer: Address,
) {
    AdminChangeProposed {
        old_admin,
        new_admin,
        proposer,
    }
    .publish(env);
}

pub fn emit_admin_changed(env: &Env, old_admin: Address, new_admin: Address, timestamp: u64) {
    AdminChanged {
        old_admin,
        new_admin,
        timestamp,
    }
    .publish(env);
}

pub fn emit_fee_collected(
    env: &Env,
    source_contract: Address,
    asset: Address,
    amount: i128,
    timestamp: u64,
) {
    FeeCollected {
        source_contract,
        asset,
        amount,
        timestamp,
    }
    .publish(env);
}

// ---------------------------------------------------------------------------
// Dutch Auction Events
// ---------------------------------------------------------------------------

/// Emitted when a new Dutch auction is created for a listing.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionCreated {
    #[topic]
    pub listing_id: String,
    pub seller: Address,
    pub usdc_asset: Address,
    pub start_price: i128,
    pub end_price: i128,
    pub duration_secs: u64,
    pub start_time: u64,
}

/// Emitted when a buyer successfully purchases via buy_now.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionBought {
    #[topic]
    pub listing_id: String,
    pub buyer: Address,
    pub final_price: i128,
    pub timestamp: u64,
}

/// Emitted when an auction expires without a buyer (cancel_expired or buy_now after deadline).
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionExpired {
    #[topic]
    pub listing_id: String,
    pub expired_at: u64,
}

pub fn emit_auction_created(
    env: &Env,
    listing_id: String,
    seller: Address,
    usdc_asset: Address,
    start_price: i128,
    end_price: i128,
    duration_secs: u64,
    start_time: u64,
) {
    AuctionCreated {
        listing_id,
        seller,
        usdc_asset,
        start_price,
        end_price,
        duration_secs,
        start_time,
    }
    .publish(env);
}

pub fn emit_auction_bought(
    env: &Env,
    listing_id: String,
    buyer: Address,
    final_price: i128,
    timestamp: u64,
) {
    AuctionBought {
        listing_id,
        buyer,
        final_price,
        timestamp,
    }
    .publish(env);
}

pub fn emit_auction_expired(env: &Env, listing_id: String, expired_at: u64) {
    AuctionExpired {
        listing_id,
        expired_at,
    }
    .publish(env);
}

