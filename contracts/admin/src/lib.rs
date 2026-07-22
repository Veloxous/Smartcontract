#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admins,
    Threshold,
    Initialized,
    AdminChangeProposal(Address, Address), // (old_admin, new_admin) -> Vec<Address> (voters)
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChangeProposed {
    #[topic]
    pub old_admin: Address,
    #[topic]
    pub new_admin: Address,
    pub proposer: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChanged {
    #[topic]
    pub old_admin: Address,
    #[topic]
    pub new_admin: Address,
    pub timestamp: u64,
}

#[contract]
pub struct AdminContract;

#[contractimpl]
impl AdminContract {
    /// Initialize the contract with a vector of admin addresses and an M-of-N threshold.
    ///
    /// # Arguments
    /// * `admins` - A list of admin addresses. Must contain at least 1 admin and no duplicate entries.
    /// * `threshold` - The required number of signatures/votes for multisig approval. Must be <= admins.len() and > 0.
    pub fn init(env: Env, admins: Vec<Address>, threshold: u32) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("already initialized");
        }

        let n = admins.len();
        if n == 0 {
            panic!("admins list cannot be empty");
        }
        if threshold == 0 || threshold > n {
            panic!("invalid threshold: M must be <= N and > 0");
        }

        // Check for duplicates in initial admins
        for i in 0..n {
            for j in (i + 1)..n {
                if admins.get(i).unwrap() == admins.get(j).unwrap() {
                    panic!("duplicate admin detected");
                }
            }
        }

        env.storage().instance().set(&DataKey::Admins, &admins);
        env.storage()
            .instance()
            .set(&DataKey::Threshold, &threshold);
        env.storage().instance().set(&DataKey::Initialized, &true);
    }

    /// Check if a given address is a registered admin.
    pub fn is_admin(env: Env, address: Address) -> bool {
        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Admins)
            .unwrap_or_else(|| Vec::new(&env));
        admins.contains(address)
    }

    /// Read all configured admin addresses from instance storage.
    pub fn get_admins(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Admins)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Read the threshold value M from instance storage.
    pub fn get_threshold(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::Threshold)
            .unwrap_or(0)
    }

    /// Propose and vote on replacing `old_admin` with `new_admin`.
    ///
    /// The caller must be a current admin. Each admin may vote once.
    /// When threshold votes are reached, `old_admin` is replaced with `new_admin`.
    pub fn propose_admin_change(
        env: Env,
        proposer: Address,
        old_admin: Address,
        new_admin: Address,
    ) {
        proposer.require_auth();

        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Admins)
            .unwrap_or_else(|| panic!("not initialized"));
        let threshold: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Threshold)
            .unwrap_or_else(|| panic!("not initialized"));

        // Only admins may vote
        if !admins.contains(&proposer) {
            panic!("only admins may vote");
        }

        // Verify old_admin exists
        if !admins.contains(&old_admin) {
            panic!("old admin not found");
        }

        // Verify new_admin is not already an admin
        if admins.contains(&new_admin) {
            panic!("new admin already exists");
        }

        let key = DataKey::AdminChangeProposal(old_admin.clone(), new_admin.clone());
        let mut voters: Vec<Address> = env
            .storage()
            .temporary()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        // Prevent duplicate vote
        if voters.contains(&proposer) {
            panic!("duplicate vote");
        }

        voters.push_back(proposer.clone());
        env.storage().temporary().set(&key, &voters);

        // Emit proposal event
        AdminChangeProposed {
            old_admin: old_admin.clone(),
            new_admin: new_admin.clone(),
            proposer: proposer.clone(),
        }
        .publish(&env);

        // If threshold reached, execute rotation
        if voters.len() >= threshold {
            let mut updated_admins = Vec::new(&env);
            for a in admins.iter() {
                if a == old_admin {
                    updated_admins.push_back(new_admin.clone());
                } else {
                    updated_admins.push_back(a);
                }
            }

            // Ensure threshold validity M <= N
            if threshold > updated_admins.len() {
                panic!("threshold invalid for updated admins");
            }

            env.storage()
                .instance()
                .set(&DataKey::Admins, &updated_admins);
            env.storage().temporary().remove(&key);

            AdminChanged {
                old_admin,
                new_admin,
                timestamp: env.ledger().timestamp(),
            }
            .publish(&env);
        }
    }
}

#[cfg(test)]
mod test;
