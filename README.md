# Veloxous — Soroban Smart Contracts

> The Trust Layer of the Veloxous Circular Economy.

This repository contains the Rust-based Soroban smart contracts deployed on the Stellar network. It provides the core escrow logic that allows Veloxous to eliminate scams in peer-to-peer hardware trading.

---

## 🔒 Escrow State Machine

```text
[ Escrow State Machine ]

                            [ AWAITING FUNDS ]
                                     |
                                     | (Buyer Deposits USDC)
                                     v
                            [ FUNDS LOCKED ]
                               /            \
           (Buyer Confirms)   /              \   (Buyer Reports Issue)
                             /                \
                            v                  v
    [ ITEM RECEIVED ] ------------> [ DISPUTED ]
    (Funds Released                 /          \
     to Seller)                    /            \
                                  v              v
               [ FUNDS RETURNED ]           [ FUNDS RELEASED ]
               (Admin Sides with            (Admin Sides with
                Buyer)                       Seller)
```

---

## 🏗 Core Contracts

### VeloxousEscrow (⚡ **New Implementation**)
Complete state machine implementation with 7 strict lifecycle states: `AwaitingFunds → Funded → Shipped → Delivered → Completed | Disputed → Refunded`.

**Features:**
- Strict linear state machine with zero-ambiguity state checks
- Timeout-based auto-refund (7 days) and auto-release (14 days)
- Dispute flow with admin resolution
- Protocol fee routing (1.5% BPS) with fixed-point arithmetic
- Check-Effects-Interactions pattern for security
- 23 unit tests achieving >95% coverage

See [`contracts/veloxous_escrow/IMPLEMENTATION.md`](contracts/veloxous_escrow/IMPLEMENTATION.md) for full details.

### MarketplaceEscrow
Advanced escrow with multisig admin governance (M-of-N threshold voting), dispute resolution with proposal voting, fee pool accumulation, and optional reputation contract integration.

### Treasury
Fee routing engine with configurable BPS-based fees (max 5%) and treasury wallet splits. Supports fixed-point arithmetic for precision fee distribution.

### Vault
Optional yield vault for `VeloxousEscrow`. Holds idle collateral between `fund_escrow` and release, forwarding it into an external yield-bearing protocol when configured. Ships a circuit breaker: any failure talking to that protocol bounces the deposit straight back to the escrow contract, which ends up holding its own collateral directly — a deposit never fails just because the yield protocol is unavailable. Earned yield is routed to a treasury on withdrawal; principal always returns to the escrow untouched.

### Admin
Standalone M-of-N multisig admin contract with threshold voting for admin rotation.

## 🛠 Tech Stack
- **Language:** Rust
- **Framework:** Soroban SDK
- **Network:** Stellar (Testnet/Mainnet)

## 💻 Development

Make sure you have the Rust toolchain and the Soroban CLI installed.

```bash
# Add the wasm target
rustup target add wasm32v1-none

# Build the contracts
cargo build --target wasm32v1-none --release

# Run tests
cargo test
```

## 🚀 Deployment (Testnet)
```bash
soroban contract deploy \
  --wasm target/wasm32v1-none/release/veloxous_escrow.wasm \
  --source admin \
  --network testnet
```
