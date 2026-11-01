# Contributing

Read AGENTS.md and the [maintenance guide](docs/maintenance.md). Use Rust 1.90
or later. Offline generation requires Python 3.8 or later; the complete
engineering gate requires Python 3.12 or later.

From the SDK root:

```sh
python scripts/generate.py --check
cargo fetch --locked
python scripts/check-api.py --check
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
cargo doc --locked --no-deps
```

The workspace Cargo configuration places builds outside this checkout. Keep
any additional build output, tests, corpora, simulators, dependencies used for
validation, and reports in `C:\server\raims\.tmp\raims-rust-sdk`.

Using Python 3.12 or later, run the complete engineering gate with:

```sh
python scripts/check.py --prepare-tools
```

The launcher calls the canonical adapter in the sibling Developer-Toolkit
checkout. It reconstructs a fresh suite under `.tmp` and validates the pinned
contract, generated files, compatibility, transport, pagination, webhooks,
security, and a packaged consumer. It never calls the real financial API.
A standalone clone can run generation, compatibility, formatting, and Cargo
checks; it reports the missing Toolkit rather than skipping the complete gate.

Review generated changes and use the appropriate SDK version for any change
to the public Rust surface. The compatibility checker does not rewrite its
baseline unless explicitly run with `--write`. Leave changes uncommitted and
unstaged. Passing checks does not authorize publication or runner activation.

The compatibility check also compiles an external consumer to verify public
`Send`/`Sync` guarantees and request futures. Its generated source and Cargo
output stay under `.tmp`. Once dependencies have been fetched, that check runs
without network access.
