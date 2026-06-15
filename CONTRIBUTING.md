# Contributing to Heliobond contracts

These are the Soroban smart contracts behind Heliobond — the `investment_vault` and `project_registry`, written in Rust. Thanks for helping out.

## Pick something to work on

Browse [open issues](https://github.com/heliobond/contracts/issues). Issues tagged **good first issue** are scoped for newcomers; **help wanted** are ready for anyone. Each issue has scope, acceptance criteria, and file pointers. Comment to claim it before you start.

## Setup

You need Rust and the [Stellar CLI](https://developers.stellar.org/docs/tools/cli).

```bash
rustup target add wasm32v1-none      # wasm target for builds
cargo test                            # run the suite
make build                            # stellar contract build
```

## Workflow

1. Fork and branch from `main` (`feat/…`, `fix/…`, `test/…`).
2. Make your change. Keep it scoped to one issue.
3. Run the quality gate locally before pushing:
   ```bash
   cargo fmt --all          # format (CI checks this)
   cargo test --all         # all tests must pass
   ```
4. Open a PR with `Closes #<issue>`. CI runs `cargo fmt --check` and `cargo test` — both must be green.

## Quality bar

- **Formatted** — `cargo fmt` clean; CI enforces it.
- **Tested** — new behaviour needs tests. We use the Soroban test SDK; see the existing `test.rs` modules.
- **Money paths are sacred** — anything touching share math, deposits, or withdrawals needs tests covering rounding direction and edge cases (first deposit, empty vault).
- **Events** — every state change should emit an event so the indexer can see it.

## Reporting issues

Bugs and ideas: [open an issue](https://github.com/heliobond/contracts/issues/new). Security problems: see [SECURITY.md](./SECURITY.md) — report privately, not in a public issue.

By contributing you agree your work is licensed under [Apache-2.0](./LICENSE), and you agree to the [Code of Conduct](./CODE_OF_CONDUCT.md).
