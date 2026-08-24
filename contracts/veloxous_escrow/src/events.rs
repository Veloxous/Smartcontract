use soroban_sdk::{contractevent, Address, Env, String};

use crate::types::EscrowStatus;

/// Emitted when a buyer successfully funds an escrow.
/// Topics: ["Veloxous", "Escrow", "Funded", transaction_id]
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Funded {
    #[topic]
    pub transaction_id: String,
    pub buyer: Address,
    pub amount: i128,
    pub asset: Address,
}

/// Emitted on every escrow state transition.
/// Topics: ["Veloxous", "Escrow", "StatusChange", transaction_id]
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusChanged {
    #[topic]
    pub transaction_id: String,
    pub old_state: EscrowStatus,
    pub new_state: EscrowStatus,
    pub timestamp: u64,
}

/// Emitted when funds are released to the seller on completion.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundsReleased {
    #[topic]
    pub transaction_id: String,
    pub seller: Address,
    pub seller_amount: i128,
    pub fee_amount: i128,
    pub timestamp: u64,
}

/// Emitted when funds are refunded to the buyer.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundsRefunded {
    #[topic]
    pub transaction_id: String,
    pub buyer: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Emitted when an escrow in a batch release is skipped due to invalid state or dispute.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchReleaseSkipped {
    #[topic]
    pub transaction_id: String,
    pub error_code: u32,
    pub timestamp: u64,
}

/// Emitted when a batch release operation finishes execution.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchReleaseCompleted {
    pub processed_count: u32,
    pub skipped_count: u32,
    pub total_released: i128,
    pub total_fee: i128,
    pub timestamp: u64,
}

pub fn emit_funded(env: &Env, transaction_id: String, buyer: Address, amount: i128, asset: Address) {
    Funded { transaction_id, buyer, amount, asset }.publish(env);
}

pub fn emit_status_changed(
    env: &Env,
    transaction_id: String,
    old_state: EscrowStatus,
    new_state: EscrowStatus,
    timestamp: u64,
) {
    StatusChanged { transaction_id, old_state, new_state, timestamp }.publish(env);
}

pub fn emit_funds_released(
    env: &Env,
    transaction_id: String,
    seller: Address,
    seller_amount: i128,
    fee_amount: i128,
    timestamp: u64,
) {
    FundsReleased { transaction_id, seller, seller_amount, fee_amount, timestamp }.publish(env);
}

pub fn emit_funds_refunded(
    env: &Env,
    transaction_id: String,
    buyer: Address,
    amount: i128,
    timestamp: u64,
) {
    FundsRefunded { transaction_id, buyer, amount, timestamp }.publish(env);
}

pub fn emit_batch_release_skipped(
    env: &Env,
    transaction_id: String,
    error_code: u32,
    timestamp: u64,
) {
    BatchReleaseSkipped { transaction_id, error_code, timestamp }.publish(env);
}
