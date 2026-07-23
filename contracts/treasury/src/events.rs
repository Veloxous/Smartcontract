use soroban_sdk::{contractevent, Address, Env, String};

/// Event payload emitted when a fee is collected and routed.
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

/// Event payload emitted when fee parameters (BPS or Splits) are updated.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeParametersUpdated {
    #[topic]
    pub parameter_name: String,
    pub old_value: u32,
    pub new_value: u32,
    pub timestamp: u64,
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

pub fn emit_fee_parameters_updated(
    env: &Env,
    parameter_name: String,
    old_value: u32,
    new_value: u32,
    timestamp: u64,
) {
    FeeParametersUpdated {
        parameter_name,
        old_value,
        new_value,
        timestamp,
    }
    .publish(env);
}
