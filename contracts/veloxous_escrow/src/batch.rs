use soroban_sdk::{token, Address, Env, Map, String, Vec};
use crate::events;
use crate::types::*;
use crate::treasury_client::TreasuryClient;
use crate::vault_client::VaultClient;

/// Helper function to convert u64 transaction ID to soroban_sdk::String.
pub fn u64_to_string(env: &Env, mut val: u64) -> String {
    if val == 0 {
        return String::from_str(env, "0");
    }
    let mut buf = [0u8; 20];
    let mut pos = 20;
    while val > 0 {
        pos -= 1;
        buf[pos] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    let s = core::str::from_utf8(&buf[pos..]).unwrap();
    String::from_str(env, s)
}

/// Helper check for admin authorization
fn is_admin(env: &Env, address: &Address) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .map(|admin| admin == *address)
        .unwrap_or(false)
}

/// Execute batch release for a vector of String transaction IDs.
///
/// Callable by Admin multisig.
/// Validates that each escrow exists and is in `Delivered` state.
/// If an escrow fails validation (e.g. Disputed or wrong state), it is skipped and an event is emitted.
/// Token transfers are aggregated by seller and treasury to optimize gas (single token client call per recipient).
pub fn batch_release(
    env: &Env,
    admin: &Address,
    transaction_ids: Vec<String>,
) -> Result<(u32, u32), Error> {
    admin.require_auth();

    if !is_admin(env, admin) {
        return Err(Error::Unauthorized);
    }

    let accepted_asset: Address = env
        .storage()
        .instance()
        .get(&DataKey::AcceptedAsset)
        .ok_or(Error::NotInitialized)?;

    let vault_contract: Option<Address> = env
        .storage()
        .instance()
        .get(&DataKey::VaultContract);

    let treasury_contract: Option<Address> = env
        .storage()
        .instance()
        .get(&DataKey::TreasuryContract);

    let now = env.ledger().timestamp();
    let mut processed_count: u32 = 0;
    let mut skipped_count: u32 = 0;
    let mut total_released: i128 = 0;
    let mut total_fee: i128 = 0;

    let mut seller_payouts: Map<Address, i128> = Map::new(env);
    let contract_addr = env.current_contract_address();

    for tx_id in transaction_ids.iter() {
        let key = DataKey::Escrow(tx_id.clone());

        let mut record: EscrowRecord = match env.storage().persistent().get(&key) {
            Some(r) => r,
            None => {
                events::emit_batch_release_skipped(env, tx_id, Error::NotFound as u32, now);
                skipped_count += 1;
                continue;
            }
        };

        if record.current_state != EscrowStatus::Delivered {
            let error_code = if record.current_state == EscrowStatus::Disputed {
                Error::DisputeActive as u32
            } else {
                Error::InvalidStateTransition as u32
            };
            events::emit_batch_release_skipped(env, tx_id, error_code, now);
            skipped_count += 1;
            continue;
        }

        let fee = record
            .amount
            .checked_mul(PROTOCOL_FEE_BPS)
            .ok_or(Error::Overflow)?
            / BPS_DENOMINATOR;
        let seller_amount = record.amount - fee;

        // 1. Update state
        let old = record.current_state.clone();
        record.current_state = EscrowStatus::Completed;
        record.updated_at = now;
        env.storage().persistent().set(&key, &record);

        // 2. Vault withdrawal if configured
        if let Some(ref vault_addr) = vault_contract {
            VaultClient::new(env, vault_addr)
                .try_withdraw(&contract_addr, &tx_id)
                .map_err(|_| Error::VaultCallFailed)?
                .map_err(|_| Error::VaultCallFailed)?;
        }

        // 3. Emit individual events
        events::emit_status_changed(env, tx_id.clone(), old, EscrowStatus::Completed, now);
        events::emit_funds_released(env, tx_id, record.seller.clone(), seller_amount, fee, now);

        // 4. Accumulate seller payout
        let current_payout = seller_payouts.get(record.seller.clone()).unwrap_or(0);
        seller_payouts.set(record.seller.clone(), current_payout + seller_amount);

        total_released += seller_amount;
        total_fee += fee;
        processed_count += 1;
    }

    // Single-invocation token transfers (Gas Optimization)
    let token_client = token::Client::new(env, &accepted_asset);

    for (seller, amount) in seller_payouts.iter() {
        if amount > 0 {
            token_client.transfer(&contract_addr, &seller, &amount);
        }
    }

    if total_fee > 0 {
        if let Some(treasury_addr) = treasury_contract {
            token_client.transfer(&contract_addr, &treasury_addr, &total_fee);
            TreasuryClient::new(env, &treasury_addr).route_fee(&accepted_asset, &total_fee);
        }
    }

    events::emit_batch_release_completed(
        env,
        processed_count,
        skipped_count,
        total_released,
        total_fee,
        now,
    );

    Ok((processed_count, skipped_count))
}

/// Execute batch release for a vector of u64 transaction IDs.
pub fn batch_release_u64(
    env: &Env,
    admin: &Address,
    transaction_ids: Vec<u64>,
) -> Result<(u32, u32), Error> {
    let mut str_ids = Vec::new(env);
    for id in transaction_ids.iter() {
        str_ids.push_back(u64_to_string(env, id));
    }
    batch_release(env, admin, str_ids)
}
