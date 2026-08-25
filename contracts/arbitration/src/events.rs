use soroban_sdk::{contractevent, Address, Env, String};
use crate::types::Verdict;

/// Event emitted when a user stakes USDC to become a juror.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JurorStaked {
    #[topic]
    pub juror: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event emitted when a juror initiates unstaking.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnstakeInitiated {
    #[topic]
    pub juror: Address,
    pub amount: i128,
    pub unlock_timestamp: u64,
}

/// Event emitted when a juror completes unstaking after lockup period.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnstakeCompleted {
    #[topic]
    pub juror: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event emitted when a new arbitration case is created.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseCreated {
    #[topic]
    pub case_id: String,
    pub transaction_id: String,
    pub buyer: Address,
    pub seller: Address,
    pub jurors: Vec<Address>,
    pub timestamp: u64,
}

/// Event emitted when a juror casts their vote.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteCast {
    #[topic]
    pub case_id: String,
    #[topic]
    pub juror: Address,
    pub verdict: Verdict,
    pub timestamp: u64,
}

/// Event emitted when a case is resolved with majority verdict.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseResolved {
    #[topic]
    pub case_id: String,
    pub final_verdict: Verdict,
    pub winning_jurors: Vec<Address>,
    pub losing_jurors: Vec<Address>,
    pub timestamp: u64,
}

/// Event emitted when a juror is slashed for dissenting.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JurorPenaltyApplied {
    #[topic]
    pub juror: Address,
    #[topic]
    pub case_id: String,
    pub penalty: i32,
}

use soroban_sdk::Vec;

pub fn emit_juror_staked(env: &Env, juror: Address, amount: i128, timestamp: u64) {
    JurorStaked {
        juror,
        amount,
        timestamp,
    }
    .publish(env);
}

pub fn emit_unstake_initiated(env: &Env, juror: Address, amount: i128, unlock_timestamp: u64) {
    UnstakeInitiated {
        juror,
        amount,
        unlock_timestamp,
    }
    .publish(env);
}

pub fn emit_unstake_completed(env: &Env, juror: Address, amount: i128, timestamp: u64) {
    UnstakeCompleted {
        juror,
        amount,
        timestamp,
    }
    .publish(env);
}

pub fn emit_case_created(
    env: &Env,
    case_id: String,
    transaction_id: String,
    buyer: Address,
    seller: Address,
    jurors: Vec<Address>,
    timestamp: u64,
) {
    CaseCreated {
        case_id,
        transaction_id,
        buyer,
        seller,
        jurors,
        timestamp,
    }
    .publish(env);
}

pub fn emit_vote_cast(
    env: &Env,
    case_id: String,
    juror: Address,
    verdict: Verdict,
    timestamp: u64,
) {
    VoteCast {
        case_id,
        juror,
        verdict,
        timestamp,
    }
    .publish(env);
}

pub fn emit_case_resolved(
    env: &Env,
    case_id: String,
    final_verdict: Verdict,
    winning_jurors: Vec<Address>,
    losing_jurors: Vec<Address>,
    timestamp: u64,
) {
    CaseResolved {
        case_id,
        final_verdict,
        winning_jurors,
        losing_jurors,
        timestamp,
    }
    .publish(env);
}

pub fn emit_juror_penalty_applied(env: &Env, juror: Address, case_id: String, penalty: i32) {
    JurorPenaltyApplied {
        juror,
        case_id,
        penalty,
    }
    .publish(env);
}
