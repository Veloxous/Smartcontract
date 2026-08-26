use soroban_sdk::{contractclient, Address, Env};

#[contractclient(name = "OracleClient")]
pub trait OracleTrait {
    /// Get the current price of an asset in USD (e.g. 7 decimals)
    fn get_price(env: Env, asset: Address) -> i128;
}
