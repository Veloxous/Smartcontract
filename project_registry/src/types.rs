use soroban_sdk::{contracttype, Address, String};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectData {
    pub owner: Address,
    pub uri: String,
    pub credit_quality: u32,
    pub green_impact: u32,
}

#[contracttype]
pub enum DataKey {
    Whitelister,
    ProjectCounter,
    Project(u32),
    Whitelist(Address),
}
