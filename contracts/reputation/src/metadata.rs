use soroban_sdk::{contracttype, Address, Env, String};
use crate::{DataKey, Error};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SbtMetadataState {
    Uninitialized = 0,
    Active = 1,
    Updating = 2,
    Suspended = 3,
    Revoked = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbtMetadata {
    pub user: Address,
    pub uri: String,
    pub version: u64,
    pub state: SbtMetadataState,
    pub last_updated: u64,
}

// 30 days assuming ~5s per ledger
const DAY_IN_LEDGERS: u32 = 17280;
const BUMP_LEDGERS: u32 = 30 * DAY_IN_LEDGERS;
const BUMP_THRESHOLD: u32 = 15 * DAY_IN_LEDGERS;

fn bump_metadata(env: &Env, user: &Address) {
    env.storage().persistent().extend_ttl(
        &DataKey::Metadata(user.clone()),
        BUMP_THRESHOLD,
        BUMP_LEDGERS,
    );
}

/// Strict State Machine Transition Rules:
/// - Uninitialized -> Active | Revoked
/// - Active -> Updating | Suspended | Revoked
/// - Updating -> Active | Suspended | Revoked
/// - Suspended -> Active | Revoked
/// - Revoked -> (Terminal state, no further transitions allowed)
pub fn can_transition(current: SbtMetadataState, next: SbtMetadataState) -> bool {
    match (current, next) {
        (SbtMetadataState::Uninitialized, SbtMetadataState::Active) => true,
        (SbtMetadataState::Uninitialized, SbtMetadataState::Revoked) => true,
        (SbtMetadataState::Active, SbtMetadataState::Updating) => true,
        (SbtMetadataState::Active, SbtMetadataState::Suspended) => true,
        (SbtMetadataState::Active, SbtMetadataState::Revoked) => true,
        (SbtMetadataState::Updating, SbtMetadataState::Active) => true,
        (SbtMetadataState::Updating, SbtMetadataState::Suspended) => true,
        (SbtMetadataState::Updating, SbtMetadataState::Revoked) => true,
        (SbtMetadataState::Suspended, SbtMetadataState::Active) => true,
        (SbtMetadataState::Suspended, SbtMetadataState::Revoked) => true,
        _ => false,
    }
}

/// Initialize metadata for a user's SBT token.
pub fn init_metadata(env: &Env, user: &Address, uri: String) -> Result<SbtMetadata, Error> {
    let key = DataKey::Metadata(user.clone());
    if env.storage().persistent().has(&key) {
        return Err(Error::MetadataAlreadyInitialized);
    }

    let now = env.ledger().timestamp();
    let metadata = SbtMetadata {
        user: user.clone(),
        uri,
        version: 1,
        state: SbtMetadataState::Active,
        last_updated: now,
    };

    env.storage().persistent().set(&key, &metadata);
    bump_metadata(env, user);
    Ok(metadata)
}

/// Retrieve full metadata record for a user's SBT token.
pub fn get_metadata(env: &Env, user: &Address) -> Result<SbtMetadata, Error> {
    let key = DataKey::Metadata(user.clone());
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(Error::MetadataNotFound)
}

/// Retrieve dynamic token URI for a user's SBT.
pub fn get_token_uri(env: &Env, user: &Address) -> Result<String, Error> {
    let metadata = get_metadata(env, user)?;
    if metadata.state == SbtMetadataState::Revoked {
        return Err(Error::MetadataStateInvalid);
    }
    Ok(metadata.uri)
}

/// Update metadata URI with optimistic version control to mitigate high-concurrency race conditions.
pub fn update_metadata(
    env: &Env,
    user: &Address,
    new_uri: String,
    expected_version: u64,
) -> Result<SbtMetadata, Error> {
    let key = DataKey::Metadata(user.clone());
    let mut metadata = get_metadata(env, user)?;

    // 1. Check version to prevent concurrency race conditions
    if metadata.version != expected_version {
        return Err(Error::VersionMismatch);
    }

    // 2. Validate current state permits updates
    if metadata.state != SbtMetadataState::Active && metadata.state != SbtMetadataState::Updating {
        return Err(Error::MetadataStateInvalid);
    }

    // 3. Perform transition: Active -> Updating -> Active
    if metadata.state == SbtMetadataState::Active {
        if !can_transition(metadata.state, SbtMetadataState::Updating) {
            return Err(Error::MetadataStateInvalid);
        }
        metadata.state = SbtMetadataState::Updating;
    }

    let now = env.ledger().timestamp();
    metadata.uri = new_uri;
    metadata.version = metadata.version.checked_add(1).ok_or(Error::MetadataStateInvalid)?;
    metadata.state = SbtMetadataState::Active;
    metadata.last_updated = now;

    env.storage().persistent().set(&key, &metadata);
    bump_metadata(env, user);
    Ok(metadata)
}

/// Explicitly transition metadata state machine state (e.g. Active -> Suspended, Suspended -> Active, Revoked).
pub fn set_metadata_state(
    env: &Env,
    user: &Address,
    new_state: SbtMetadataState,
) -> Result<SbtMetadata, Error> {
    let key = DataKey::Metadata(user.clone());
    let mut metadata = get_metadata(env, user)?;

    if !can_transition(metadata.state, new_state) {
        return Err(Error::MetadataStateInvalid);
    }

    metadata.state = new_state;
    metadata.last_updated = env.ledger().timestamp();

    env.storage().persistent().set(&key, &metadata);
    bump_metadata(env, user);
    Ok(metadata)
}
