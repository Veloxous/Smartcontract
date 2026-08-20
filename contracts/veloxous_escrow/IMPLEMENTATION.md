# Veloxous Escrow — Implementation Summary

## Overview

Complete rewrite of the `veloxous_escrow` contract implementing a strict linear state machine for P2P hardware trading escrow with full dispute resolution, timeouts, and protocol fee routing.

## State Machine

```
AwaitingFunds → Funded → Shipped → Delivered → Completed
                  ↓         ↓          ↓
                  └─────→ Disputed ──→ Refunded / Completed
```

### States (u32 discriminants for zero-ambiguity)

- `AwaitingFunds (0)` — Initial state before buyer deposits
- `Funded (1)` — Buyer has deposited USDC into escrow
- `Shipped (2)` — Seller has marked the item as shipped
- `Delivered (3)` — Buyer has confirmed physical receipt
- `Completed (4)` — Funds released to seller (minus protocol fee)
- `Disputed (5)` — Dispute raised by buyer or seller (blocks all normal transitions)
- `Refunded (6)` — Full refund returned to buyer

## Core Functions

### Initialization

- **`init(admin, accepted_asset, treasury_contract?, vault_contract?)`**  
  One-time setup. Sets admin, accepted USDC asset address, optional treasury for fee routing, and an optional yield [`vault`](../vault) contract that puts idle collateral to work between `fund_escrow` and release.

### Deposit & Lock

- **`fund_escrow(buyer, seller, asset, amount, transaction_id)`**  
  Buyer deposits exact expected USDC. Validates asset matches strictly. Transfers via `token::Client`. Updates state to `Funded`.  
  **Check-Effects-Interactions pattern:** State written before external token transfer.  
  If a vault is configured, the deposited collateral is immediately forwarded into it (see [Yield Vault Integration](#yield-vault-integration)).

### Shipping & Delivery

- **`mark_shipped(seller, transaction_id)`**  
  Seller marks item shipped. Transition: `Funded → Shipped`.

- **`mark_delivered(buyer, transaction_id)`**  
  Buyer confirms receipt. Transition: `Shipped → Delivered`.

### Release & Treasury Routing

- **`release_funds(caller, transaction_id)`**  
  Callable by buyer (happy path from `Delivered`) or admin (dispute resolution from `Disputed`).  
  Fee calculation: `fee = (amount * 150) / 10_000` (1.5%, rounds down).  
  Transfers seller net amount, routes fee to treasury (if configured).  
  Transition: `Delivered | Disputed → Completed`.

### Timeouts

- **`auto_refund(transaction_id)`**  
  Callable by anyone if seller doesn't mark `Shipped` within 7 days of funding. Full refund to buyer.  
  Transition: `Funded → Refunded` (only after `SHIPPING_DEADLINE_SECS = 604800s`).

- **`auto_release(transaction_id)`**  
  Callable by anyone if buyer doesn't confirm delivery within 14 days of shipping (AFK buyer).  
  Transition: `Shipped → Completed` (only after `ACCEPTANCE_DEADLINE_SECS = 1209600s`).

### Dispute

- **`raise_dispute(caller, transaction_id, reason)`**  
  Buyer or seller can raise dispute from `Funded`, `Shipped`, or `Delivered`.  
  Halts all normal execution paths (mark_shipped, mark_delivered, auto_refund, auto_release).  
  Transition: `Funded | Shipped | Delivered → Disputed`.

- **`resolve_dispute(admin, transaction_id, release_to_seller)`**  
  Admin resolves dispute:  
  - `release_to_seller = true` → seller paid (minus fee)  
  - `release_to_seller = false` → buyer fully refunded

### Getters

- **`get_escrow(transaction_id) → EscrowRecord`**
- **`get_admin() → Address`**

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 1 | `InvalidStateTransition` | Illegal state change attempted |
| 2 | `AlreadyExists` | Duplicate `transaction_id` |
| 3 | `NotFound` | Escrow not found |
| 4 | `AmountMismatch` | (Reserved for future use) |
| 5 | `AssetMismatch` | Asset doesn't match accepted USDC |
| 6 | `Unauthorized` | Caller not authorized |
| 7 | `DisputeActive` | Operation blocked during dispute |
| 8 | `ShippingDeadlineNotElapsed` | Too early for `auto_refund` |
| 9 | `AcceptanceDeadlineNotElapsed` | Too early for `auto_release` |
| 10 | `AlreadyInitialized` | Init called twice |
| 11 | `NotInitialized` | Contract not initialized |
| 12 | `Overflow` | Integer overflow in calculation |
| 13 | `InvalidAmount` | Amount ≤ 0 |
| 14 | `VaultCallFailed` | The configured vault contract failed while depositing/withdrawing collateral |

## Event Emission

All events use structured topics for indexing:

- **`Funded`** — `[transaction_id, buyer, amount, asset]`
- **`StatusChanged`** — `[transaction_id, old_state, new_state, timestamp]`
- **`FundsReleased`** — `[transaction_id, seller, seller_amount, fee_amount]`
- **`FundsRefunded`** — `[transaction_id, buyer, amount, timestamp]`

## Yield Vault Integration

An optional [`vault`](../vault) contract can be wired in at `init` to put idle escrow
collateral to work instead of letting it sit unused between `fund_escrow` and release.

- **On `fund_escrow`**: once the buyer's USDC lands in this contract, it is forwarded
  to the vault, which in turn attempts to deposit it into a configured external
  yield-bearing protocol.
- **On any exit path** (`release_funds`, `auto_release`, `auto_refund`,
  `resolve_dispute`): the escrow pulls its principal back from the vault *before*
  running its existing seller/buyer transfer logic, which is otherwise unchanged.
  Any yield earned in the meantime is routed by the vault straight to its own
  treasury — it never passes through this contract.
- **Circuit breaker**: if the yield protocol is unreachable, paused, or otherwise
  fails, the vault never takes custody at all — it bounces the deposit straight
  back within the same call, so the USDC ends up held directly in *this* escrow
  contract, exactly as if no vault had been configured. `fund_escrow` never fails
  because the yield protocol is down. A failure calling the *vault contract
  itself* (e.g. misconfiguration) surfaces here as `Error::VaultCallFailed`.
- Omitting `vault_contract` at `init` fully preserves the original behavior:
  collateral just sits in this contract's own balance, as before.

See [`contracts/vault/src/lib.rs`](../vault/src/lib.rs) for the vault's own deposit/withdraw
logic and circuit breaker implementation.

## Testing

**25 unit tests covering >95% of state machine logic** (plus 8 more in the `vault` crate
covering the yield vault in isolation):

✅ Valid lifecycle: AwaitingFunds → Funded → Shipped → Delivered → Completed  
✅ Invalid transitions: Funded → Delivered (skips Shipped), Delivered → Shipped  
✅ Timeout logic: `auto_refund` after 7 days, `auto_release` after 14 days  
✅ Dispute flow: Raise dispute, block normal paths, admin resolution  
✅ Asset validation: Wrong asset, zero amount, duplicate transaction_id  
✅ Authorization: Buyer/seller role checks, admin-only dispute resolution  
✅ Fee calculation: Fixed-point arithmetic, rounding down  
✅ Yield vault: full Fund → Release lifecycle routes earned yield to treasury  
✅ Yield vault: circuit breaker falls back gracefully when the yield protocol fails, without losing funds  

All tests use `try_*` methods to validate error codes without panic string matching (Soroban SDK wraps errors in `HostError`).

## Security Patterns

- **Check-Effects-Interactions**: State updated before external token transfers
- **Strict asset validation**: Only accepted USDC token can be deposited
- **Dispute lock**: All normal operations blocked when `status == Disputed`
- **Fixed-point fee arithmetic**: Prevents precision loss, rounds down in favor of user
- **Authorization guards**: `require_auth()` on buyer, seller, admin at every entry point
- **Immutable `transaction_id`**: No reuse or overwrite possible

## Differences from `marketplace_escrow`

| Feature | `marketplace_escrow` | `veloxous_escrow` |
|---------|----------------------|-------------------|
| State machine | 5 states (Locked, Released, Refunded, Disputed, Resolved) | 7 states (linear lifecycle) |
| Dispute resolution | Multisig voting (M-of-N threshold) | Single admin resolution |
| Timeout mechanism | ❌ None | ✅ `auto_refund` & `auto_release` |
| Fee routing | Fee pool + sweep to treasury | Direct routing on release |
| Reputation | Optional reputation contract hook | ❌ None |
| Admin rotation | Multisig proposal voting | ❌ Fixed single admin |

## Deployment

```bash
stellar contract build
stellar contract deploy \
  --wasm target/wasm32v1-none/release/veloxous_escrow.wasm \
  --source admin \
  --network testnet
```

## Next Steps

- [ ] Integration testing with deployed treasury contract
- [ ] Multisig admin support (integrate with `admin` contract)
- [ ] Reputation contract integration
- [ ] Advanced fee structures (tiered, milestone-based)
- [ ] Gas optimization pass
