use soroban_sdk::{token, Address, Env, Vec};
use crate::types::{DataKey, JurorRecord};
use crate::events;

/// Default lockup period: 7 days in seconds
const DEFAULT_LOCKUP_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;

/// Minimum stake amount required to become a juror (100 USDC with 6 decimals)
const DEFAULT_MIN_STAKE_AMOUNT: i128 = 100_000_000;

/// Stake USDC to become an eligible arbitration juror.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `caller` - Address staking to become a juror
/// * `amount` - Amount of USDC to stake
///
/// # Panics
/// * If amount is less than minimum stake amount
/// * If contract is not initialized
pub fn stake_as_juror(env: &Env, caller: &Address, amount: i128) {
    caller.require_auth();

    let min_stake: i128 = env
        .storage()
        .instance()
        .get(&DataKey::MinStakeAmount)
        .unwrap_or(DEFAULT_MIN_STAKE_AMOUNT);

    if amount < min_stake {
        panic!("stake amount below minimum");
    }

    let usdc_token: Address = env
        .storage()
        .instance()
        .get(&DataKey::UsdcToken)
        .expect("not initialized");

    // Transfer USDC from caller to contract
    let contract_addr = env.current_contract_address();
    let token_client = token::Client::new(env, &usdc_token);
    token_client.transfer(caller, &contract_addr, &amount);

    // Update or create juror record
    let key = DataKey::Juror(caller.clone());
    let mut juror_record: JurorRecord = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| JurorRecord {
            address: caller.clone(),
            staked_amount: 0,
            cases_participated: 0,
            cases_won: 0,
            last_case_timestamp: 0,
        });

    juror_record.staked_amount += amount;

    // Add to juror pool if this is a new juror
    if juror_record.cases_participated == 0 && juror_record.staked_amount == amount {
        let mut pool: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::JurorPool)
            .unwrap_or_else(|| Vec::new(env));
        
        if !pool.contains(caller) {
            pool.push_back(caller.clone());
            env.storage().persistent().set(&DataKey::JurorPool, &pool);
        }
    }

    env.storage().persistent().set(&key, &juror_record);

    events::emit_juror_staked(env, caller.clone(), amount, env.ledger().timestamp());
}

/// Initiate unstaking of USDC tokens.
///
/// There is a 7-day lockup period after the last case participation
/// before the juror can actually withdraw their stake.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `caller` - Address of the juror wanting to unstake
///
/// # Panics
/// * If caller is not a staked juror
/// * If lockup period has not elapsed since last case participation
pub fn unstake(env: &Env, caller: &Address) {
    caller.require_auth();

    let key = DataKey::Juror(caller.clone());
    let juror_record: JurorRecord = env
        .storage()
        .persistent()
        .get(&key)
        .expect("not a juror");

    if juror_record.staked_amount <= 0 {
        panic!("no stake to withdraw");
    }

    let lockup_period: u64 = env
        .storage()
        .instance()
        .get(&DataKey::LockupPeriodSecs)
        .unwrap_or(DEFAULT_LOCKUP_PERIOD_SECS);

    let current_time = env.ledger().timestamp();
    let last_case = juror_record.last_case_timestamp;

    // Check lockup period
    if last_case > 0 && current_time < last_case + lockup_period {
        panic!("lockup period not elapsed");
    }

    let usdc_token: Address = env
        .storage()
        .instance()
        .get(&DataKey::UsdcToken)
        .expect("not initialized");

    // Transfer USDC back to juror
    let contract_addr = env.current_contract_address();
    let token_client = token::Client::new(env, &usdc_token);
    let amount = juror_record.staked_amount;
    token_client.transfer(&contract_addr, caller, &amount);

    // Remove from juror pool
    let pool: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::JurorPool)
        .unwrap_or_else(|| Vec::new(env));
    
    let mut new_pool = Vec::new(env);
    for addr in pool.iter() {
        if &addr != caller {
            new_pool.push_back(addr);
        }
    }
    env.storage().persistent().set(&DataKey::JurorPool, &new_pool);

    // Remove juror record
    env.storage().persistent().remove(&key);

    events::emit_unstake_completed(env, caller.clone(), amount, current_time);
}

/// Get a juror's record.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `juror` - Address of the juror
///
/// # Returns
/// * The juror's record if they are staked
pub fn get_juror_record(env: &Env, juror: &Address) -> Option<JurorRecord> {
    let key = DataKey::Juror(juror.clone());
    env.storage().persistent().get(&key)
}

/// Get the list of all staked jurors.
///
/// # Arguments
/// * `env` - Soroban environment
///
/// # Returns
/// * Vector of juror addresses
pub fn get_juror_pool(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::JurorPool)
        .unwrap_or_else(|| Vec::new(env))
}

/// Update juror's participation record after a case.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `juror` - Address of the juror
/// * `won` - Whether the juror voted with the majority
pub fn update_juror_participation(env: &Env, juror: &Address, won: bool) {
    let key = DataKey::Juror(juror.clone());
    let mut juror_record: JurorRecord = env
        .storage()
        .persistent()
        .get(&key)
        .expect("juror not found");

    juror_record.cases_participated += 1;
    if won {
        juror_record.cases_won += 1;
    }
    juror_record.last_case_timestamp = env.ledger().timestamp();

    env.storage().persistent().set(&key, &juror_record);
}
