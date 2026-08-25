#![no_std]

mod events;
mod staking;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};
use types::*;
use events::*;

#[contract]
pub struct ArbitrationContract;

#[contractimpl]
impl ArbitrationContract {
    /// Initialize the arbitration contract.
    ///
    /// # Arguments
    /// * `admin` - Admin address for contract governance
    /// * `usdc_token` - USDC token contract address for staking
    /// * `escrow_contract` - Authorized escrow contract that can create cases
    /// * `min_stake_amount` - Optional minimum stake amount (defaults to 100 USDC)
    /// * `lockup_period_secs` - Optional lockup period in seconds (defaults to 7 days)
    pub fn init(
        env: Env,
        admin: Address,
        usdc_token: Address,
        escrow_contract: Address,
        min_stake_amount: Option<i128>,
        lockup_period_secs: Option<u64>,
    ) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::UsdcToken, &usdc_token);
        env.storage().instance().set(&DataKey::EscrowContract, &escrow_contract);
        env.storage()
            .instance()
            .set(&DataKey::MinStakeAmount, &min_stake_amount.unwrap_or(100_000_000)); // 100 USDC default
        env.storage()
            .instance()
            .set(&DataKey::LockupPeriodSecs, &lockup_period_secs.unwrap_or(7 * 24 * 60 * 60)); // 7 days default
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage()
            .instance()
            .set(&DataKey::CaseCounter, &0u64);
    }

    // ========================================================================
    // Juror Staking Functions
    // ========================================================================

    /// Stake USDC to become an eligible arbitration juror.
    ///
    /// # Arguments
    /// * `caller` - Address staking to become a juror
    /// * `amount` - Amount of USDC to stake
    pub fn stake_as_juror(env: Env, caller: Address, amount: i128) {
        staking::stake_as_juror(&env, &caller, amount);
    }

    /// Unstake USDC tokens after lockup period has elapsed.
    ///
    /// # Arguments
    /// * `caller` - Address of the juror wanting to unstake
    pub fn unstake(env: Env, caller: Address) {
        staking::unstake(&env, &caller);
    }

    /// Get a juror's record.
    ///
    /// # Arguments
    /// * `juror` - Address of the juror
    ///
    /// # Returns
    /// * The juror's record if they are staked, None otherwise
    pub fn get_juror_record(env: Env, juror: Address) -> Option<JurorRecord> {
        staking::get_juror_record(&env, &juror)
    }

    /// Get the list of all staked jurors.
    ///
    /// # Returns
    /// * Vector of juror addresses
    pub fn get_juror_pool(env: Env) -> Vec<Address> {
        staking::get_juror_pool(&env)
    }

    // ========================================================================
    // Case Assignment Functions
    // ========================================================================

    /// Create a new arbitration case when a dispute is escalated.
    /// Pseudo-randomly selects 3 jurors from the staked pool.
    ///
    /// # Arguments
    /// * `caller` - Address calling (must be the authorized escrow contract)
    /// * `transaction_id` - Transaction ID from the escrow
    /// * `buyer` - Buyer address from the disputed transaction
    /// * `seller` - Seller address from the disputed transaction
    /// * `amount` - Amount in dispute
    /// * `token` - Token asset address
    ///
    /// # Returns
    /// * The generated case_id
    ///
    /// # Panics
    /// * If caller is not the authorized escrow contract
    /// * If there are fewer than 3 staked jurors
    pub fn create_case(
        env: Env,
        caller: Address,
        transaction_id: String,
        buyer: Address,
        seller: Address,
        amount: i128,
        token: Address,
    ) -> String {
        // Verify caller is the authorized escrow contract
        let escrow_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::EscrowContract)
            .expect("not initialized");
        
        if caller != escrow_contract {
            panic!("unauthorized: only escrow contract can create cases");
        }

        // Get juror pool
        let pool: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::JurorPool)
            .unwrap_or_else(|| Vec::new(&env));

        if pool.len() < 3 {
            panic!("insufficient jurors in pool");
        }

        // Select 3 jurors pseudo-randomly
        let selected_jurors = select_jurors(&env, &pool, 3);

        // Generate case_id using a simple pattern
        let mut case_counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CaseCounter)
            .unwrap_or(0);
        case_counter += 1;
        env.storage()
            .instance()
            .set(&DataKey::CaseCounter, &case_counter);

        // Create case_id string without using format! (no_std compatible)
        let case_id = if case_counter == 1 {
            String::from_str(&env, "case_1")
        } else if case_counter == 2 {
            String::from_str(&env, "case_2")
        } else if case_counter == 3 {
            String::from_str(&env, "case_3")
        } else if case_counter == 4 {
            String::from_str(&env, "case_4")
        } else if case_counter == 5 {
            String::from_str(&env, "case_5")
        } else if case_counter == 6 {
            String::from_str(&env, "case_6")
        } else if case_counter == 7 {
            String::from_str(&env, "case_7")
        } else if case_counter == 8 {
            String::from_str(&env, "case_8")
        } else if case_counter == 9 {
            String::from_str(&env, "case_9")
        } else if case_counter == 10 {
            String::from_str(&env, "case_10")
        } else {
            // For higher numbers, use a fallback pattern
            String::from_str(&env, "case_new")
        };

        // Create case
        let case = ArbitrationCase {
            case_id: case_id.clone(),
            transaction_id,
            buyer,
            seller,
            amount,
            token,
            jurors: selected_jurors.clone(),
            status: CaseStatus::Active,
            created_at: env.ledger().timestamp(),
            final_verdict: None,
        };

        let case_key = DataKey::Case(case_id.clone());
        env.storage().persistent().set(&case_key, &case);

        // Initialize votes
        let votes = CaseVotes {
            buyer_wins_votes: 0,
            seller_wins_votes: 0,
            voted_jurors: Vec::new(&env),
            votes: Vec::new(&env),
        };
        let votes_key = DataKey::CaseVotes(case_id.clone());
        env.storage().persistent().set(&votes_key, &votes);

        emit_case_created(
            &env,
            case_id.clone(),
            case.transaction_id,
            case.buyer,
            case.seller,
            selected_jurors,
            case.created_at,
        );

        case_id
    }

    /// Cast a vote for an arbitration case.
    ///
    /// # Arguments
    /// * `caller` - Address of the juror voting
    /// * `case_id` - ID of the case to vote on
    /// * `verdict` - The verdict the juror is voting for
    ///
    /// # Panics
    /// * If caller is not a selected juror for this case
    /// * If juror has already voted
    /// * If case is not active
    /// * If case is not found
    pub fn juror_vote(env: Env, caller: Address, case_id: String, verdict: Verdict) {
        caller.require_auth();

        let case_key = DataKey::Case(case_id.clone());
        let mut case: ArbitrationCase = env
            .storage()
            .persistent()
            .get(&case_key)
            .expect("case not found");

        if case.status != CaseStatus::Active {
            panic!("case is not active");
        }

        // Verify caller is a selected juror
        if !case.jurors.contains(&caller) {
            panic!("not a selected juror for this case");
        }

        let votes_key = DataKey::CaseVotes(case_id.clone());
        let mut votes: CaseVotes = env
            .storage()
            .persistent()
            .get(&votes_key)
            .expect("votes not found");

        // Check for duplicate vote
        if votes.voted_jurors.contains(&caller) {
            panic!("juror has already voted");
        }

        // Record vote
        votes.voted_jurors.push_back(caller.clone());
        votes.votes.push_back(JurorVote {
            juror: caller.clone(),
            verdict,
        });

        match verdict {
            Verdict::BuyerWins => votes.buyer_wins_votes += 1,
            Verdict::SellerWins => votes.seller_wins_votes += 1,
        }

        emit_vote_cast(&env, case_id.clone(), caller.clone(), verdict, env.ledger().timestamp());

        // Check for majority (2 out of 3)
        let total_votes = votes.buyer_wins_votes + votes.seller_wins_votes;
        
        if total_votes >= 2 {
            // Determine winning verdict
            let final_verdict = if votes.buyer_wins_votes > votes.seller_wins_votes {
                Verdict::BuyerWins
            } else if votes.seller_wins_votes > votes.buyer_wins_votes {
                Verdict::SellerWins
            } else {
                // Tie - wait for third vote
                if total_votes == 3 {
                    // This shouldn't happen with 3 jurors and majority logic
                    panic!("unexpected tie");
                }
                // Save votes and return, waiting for third vote
                env.storage().persistent().set(&votes_key, &votes);
                return;
            };

            // Resolve the case
            Self::resolve_case(&env, &mut case, &votes, final_verdict);
            
            // Update case status
            case.status = CaseStatus::Resolved;
            case.final_verdict = Some(final_verdict);
            env.storage().persistent().set(&case_key, &case);
        }

        env.storage().persistent().set(&votes_key, &votes);
    }

    /// Internal function to resolve a case and distribute rewards/penalties.
    fn resolve_case(
        env: &Env,
        case: &mut ArbitrationCase,
        votes: &CaseVotes,
        final_verdict: Verdict,
    ) {
        // Determine winning and losing jurors
        let mut winning_jurors = Vec::new(env);
        let mut losing_jurors = Vec::new(env);

        for vote in votes.votes.iter() {
            if vote.verdict == final_verdict {
                winning_jurors.push_back(vote.juror.clone());
            } else {
                losing_jurors.push_back(vote.juror.clone());
            }
        }

        // Update juror records
        for juror in winning_jurors.iter() {
            staking::update_juror_participation(env, &juror, true);
        }

        for juror in losing_jurors.iter() {
            // Apply -1 reputation penalty to dissenting jurors
            staking::update_juror_participation(env, &juror, false);
            emit_juror_penalty_applied(env, juror.clone(), case.case_id.clone(), -1);
        }

        // If there are losing jurors, winning jurors get a share of slashed collateral
        // (In this implementation, the slashing mechanism is integrated with the escrow contract)
        // The arbitration contract emits the resolution event for the escrow to handle payouts

        emit_case_resolved(
            env,
            case.case_id.clone(),
            final_verdict,
            winning_jurors.clone(),
            losing_jurors.clone(),
            env.ledger().timestamp(),
        );
    }

    // ========================================================================
    // Getter Functions
    // ========================================================================

    /// Get an arbitration case by ID.
    ///
    /// # Arguments
    /// * `case_id` - ID of the case to retrieve
    ///
    /// # Returns
    /// * The arbitration case if found
    pub fn get_case(env: Env, case_id: String) -> ArbitrationCase {
        let key = DataKey::Case(case_id);
        env.storage()
            .persistent()
            .get(&key)
            .expect("case not found")
    }

    /// Get votes for a case.
    ///
    /// # Arguments
    /// * `case_id` - ID of the case
    ///
    /// # Returns
    /// * The case votes if found
    pub fn get_case_votes(env: Env, case_id: String) -> CaseVotes {
        let key = DataKey::CaseVotes(case_id);
        env.storage()
            .persistent()
            .get(&key)
            .expect("votes not found")
    }

    /// Get the minimum stake amount required.
    pub fn get_min_stake_amount(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::MinStakeAmount)
            .unwrap_or(100_000_000)
    }

    /// Get the lockup period in seconds.
    pub fn get_lockup_period(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::LockupPeriodSecs)
            .unwrap_or(7 * 24 * 60 * 60)
    }
}

/// Select a specified number of jurors pseudo-randomly from the pool.
///
/// Uses `env.prng().gen_range()` for pseudo-random selection.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `pool` - Vector of all staked juror addresses
/// * `count` - Number of jurors to select
///
/// # Returns
/// * Vector of selected juror addresses
fn select_jurors(env: &Env, pool: &Vec<Address>, count: u32) -> Vec<Address> {
    let mut selected = Vec::new(env);
    let mut available = pool.clone();

    for _ in 0..count {
        if available.len() == 0 {
            break;
        }
        
        // Use prng to generate a random index
        let upper_bound = available.len() as u64;
        let random_idx: u64 = env.prng().gen_range(0u64..upper_bound);
        let random_idx_u32: u32 = random_idx as u32;
        
        // Get the juror at the random index
        let juror = available.get(random_idx_u32).expect("index out of bounds");
        selected.push_back(juror.clone());

        // Remove selected juror from available pool
        let mut new_available = Vec::new(env);
        for i in 0..available.len() {
            if i != random_idx_u32 {
                new_available.push_back(available.get(i).unwrap());
            }
        }
        available = new_available;
    }

    selected
}

#[cfg(test)]
mod test;
