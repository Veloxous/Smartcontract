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
- **VeloxousEscrow:** Handles the locking of USDC. Funds are only released to the seller/technician when the buyer confirms receipt of the physical item or successful repair.
- **DisputeResolution:** Fallback mechanisms for handling contested swaps. Admin multi-sig intervention layer.

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
