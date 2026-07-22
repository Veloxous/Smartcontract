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
