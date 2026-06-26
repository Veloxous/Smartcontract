# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for the Heliobond smart contracts. Each ADR documents a significant architectural choice, the context that drove it, the options considered, and the trade-offs accepted.

## Index

| # | Title | Status |
|---|-------|--------|
| [001](001-soroban-platform.md) | Use Soroban / Stellar for smart contracts | Accepted |
| [002](002-storage-patterns.md) | Persistent vs instance storage partitioning | Accepted |
| [003](003-share-vault-model.md) | ERC-4626-inspired share vault for investments | Accepted |
| [004](004-security-model.md) | Owner-only admin pattern and whitelist access control | Accepted |

## Format

```
# ADR-NNN: Title

**Status:** Proposed | Accepted | Deprecated | Superseded by ADR-NNN

## Context
Why does this decision need to be made?

## Decision
What did we decide?

## Consequences
What are the trade-offs?
```
