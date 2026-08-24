use soroban_sdk::{contractevent, Address, Env, String};

/// Emitted when an escrow's funds are deposited into the vault.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposited {
    #[topic]
    pub transaction_id: String,
    pub principal: i128,
    pub in_yield_protocol: bool,
}

/// Emitted when the circuit breaker falls back to holding funds directly
/// instead of routing them into the external yield protocol.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerTripped {
    #[topic]
    pub transaction_id: String,
    pub yield_protocol: Address,
}

/// Emitted when an escrow's principal (and any earned yield) is withdrawn.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Withdrawn {
    #[topic]
    pub transaction_id: String,
    pub principal: i128,
    pub yield_earned: i128,
}

/// Emitted when earned yield is routed to the protocol treasury.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldRouted {
    #[topic]
    pub transaction_id: String,
    pub treasury: Address,
    pub amount: i128,
}

pub fn emit_deposited(env: &Env, transaction_id: String, principal: i128, in_yield_protocol: bool) {
    Deposited { transaction_id, principal, in_yield_protocol }.publish(env);
}

pub fn emit_circuit_breaker_tripped(env: &Env, transaction_id: String, yield_protocol: Address) {
    CircuitBreakerTripped { transaction_id, yield_protocol }.publish(env);
}

pub fn emit_withdrawn(env: &Env, transaction_id: String, principal: i128, yield_earned: i128) {
    Withdrawn { transaction_id, principal, yield_earned }.publish(env);
}

pub fn emit_yield_routed(env: &Env, transaction_id: String, treasury: Address, amount: i128) {
    YieldRouted { transaction_id, treasury, amount }.publish(env);
}
