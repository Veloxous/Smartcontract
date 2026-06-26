use soroban_sdk::{contractevent, Address, Env};

/// Emitted when an investor deposits USDC and receives vault shares.
#[contractevent]
pub struct Deposit {
    #[topic]
    pub from: Address,
    pub usdc_amount: i128,
    pub shares_minted: i128,
}

/// Emitted when an investor burns shares and withdraws USDC.
#[contractevent]
pub struct Withdraw {
    #[topic]
    pub from: Address,
    pub shares_burned: i128,
    pub usdc_returned: i128,
}

/// Emitted when the vault funds a registered project.
#[contractevent]
pub struct ProjectFunded {
    #[topic]
    pub project_id: u32,
    pub amount: i128,
    pub recipient: Address,
}

/// Emitted when yield USDC is received from a project repayment (#125).
#[contractevent]
pub struct YieldReceived {
    #[topic]
    pub from: Address,
    pub amount: i128,
}

/// Emitted when a shareholder claims accumulated yield (#125).
#[contractevent]
pub struct YieldClaimed {
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// Emitted when an insurance payout is made for a defaulted project (#135).
#[contractevent]
pub struct InsuranceClaimed {
    #[topic]
    pub project_id: u32,
    pub recipient: Address,
    pub amount: i128,
}

pub fn deposit(env: &Env, from: &Address, usdc_amount: i128, shares_minted: i128) {
    Deposit {
        from: from.clone(),
        usdc_amount,
        shares_minted,
    }
    .publish(env);
}

pub fn withdraw(env: &Env, from: &Address, shares_burned: i128, usdc_returned: i128) {
    Withdraw {
        from: from.clone(),
        shares_burned,
        usdc_returned,
    }
    .publish(env);
}

pub fn project_funded(env: &Env, project_id: u32, amount: i128, recipient: &Address) {
    ProjectFunded {
        project_id,
        amount,
        recipient: recipient.clone(),
    }
    .publish(env);
}

pub fn yield_received(env: &Env, from: &Address, amount: i128) {
    YieldReceived { from: from.clone(), amount }.publish(env);
}

pub fn yield_claimed(env: &Env, to: &Address, amount: i128) {
    YieldClaimed { to: to.clone(), amount }.publish(env);
}

pub fn insurance_claimed(env: &Env, project_id: u32, recipient: &Address, amount: i128) {
    InsuranceClaimed {
        project_id,
        recipient: recipient.clone(),
        amount,
    }
    .publish(env);
}
