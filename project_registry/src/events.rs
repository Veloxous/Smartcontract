use soroban_sdk::{contractevent, Address, Env, String};
use crate::types::CertificationStatus;

/// Emitted when a whitelisted creator registers a new project.
#[contractevent]
pub struct ProjectCreated {
    #[topic]
    pub project_id: u32,
    pub owner: Address,
    pub uri: String,
}

/// Emitted when the oracle updates a project's credit-quality / green-impact scores.
#[contractevent]
pub struct ProjectUpdated {
    #[topic]
    pub project_id: u32,
    pub credit_quality: u32,
    pub green_impact: u32,
}

/// Emitted when an account's whitelist status is changed.
#[contractevent]
pub struct WhitelistSet {
    #[topic]
    pub account: Address,
    pub status: bool,
}

/// Emitted when a project's certification status is updated (#130).
#[contractevent]
pub struct ProjectCertified {
    #[topic]
    pub project_id: u32,
    pub status: CertificationStatus,
}

/// Emitted when a governance proposal is created (#134).
#[contractevent]
pub struct ProposalCreated {
    #[topic]
    pub proposal_id: u32,
    pub proposer: Address,
    pub voting_ends_at: u64,
}

/// Emitted when a vote is cast on a proposal (#134).
#[contractevent]
pub struct VoteCast {
    #[topic]
    pub proposal_id: u32,
    pub voter: Address,
    pub support: bool,
    pub weight: i128,
}

/// Emitted when a proposal is finalised (#134).
#[contractevent]
pub struct ProposalExecuted {
    #[topic]
    pub proposal_id: u32,
    pub passed: bool,
}

pub fn project_created(env: &Env, project_id: u32, owner: &Address, uri: &String) {
    ProjectCreated {
        project_id,
        owner: owner.clone(),
        uri: uri.clone(),
    }
    .publish(env);
}

pub fn project_updated(env: &Env, project_id: u32, credit_quality: u32, green_impact: u32) {
    ProjectUpdated {
        project_id,
        credit_quality,
        green_impact,
    }
    .publish(env);
}

pub fn whitelist_set(env: &Env, account: &Address, status: bool) {
    WhitelistSet {
        account: account.clone(),
        status,
    }
    .publish(env);
}

pub fn project_certified(env: &Env, project_id: u32, status: CertificationStatus) {
    ProjectCertified { project_id, status }.publish(env);
}

pub fn proposal_created(env: &Env, proposal_id: u32, proposer: &Address, voting_ends_at: u64) {
    ProposalCreated {
        proposal_id,
        proposer: proposer.clone(),
        voting_ends_at,
    }
    .publish(env);
}

pub fn vote_cast(env: &Env, proposal_id: u32, voter: &Address, support: bool, weight: i128) {
    VoteCast {
        proposal_id,
        voter: voter.clone(),
        support,
        weight,
    }
    .publish(env);
}

pub fn proposal_executed(env: &Env, proposal_id: u32, passed: bool) {
    ProposalExecuted { proposal_id, passed }.publish(env);
}
