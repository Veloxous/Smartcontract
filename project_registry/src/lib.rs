#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

mod types;

pub use types::{DataKey, ProjectData};

#[contract]
pub struct ProjectRegistry;

#[contractimpl]
impl ProjectRegistry {}
