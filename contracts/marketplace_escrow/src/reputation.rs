use soroban_sdk::{contractclient, Address, Env, String};

/// Abstract trait definition for reputation contract integration.
/// Future reputation implementations can fulfill this interface.
#[contractclient(name = "ReputationClient")]
pub trait ReputationTrait {
    fn record_dispute_result(env: Env, winner: Address, loser: Address, transaction_id: String);
}

/// Helper function to notify the configured reputation contract.
pub fn notify_reputation(
    env: &Env,
    reputation_contract: &Address,
    winner: &Address,
    loser: &Address,
    transaction_id: &String,
) {
    let client = ReputationClient::new(env, reputation_contract);
    client.record_dispute_result(winner, loser, transaction_id);
}
