# Contract and maintenance

## Published contract

`contracts/openapi.json` preserves the published public OpenAPI bytes.
`contracts/source.json` records the immutable revision, SHA-256, API version,
operation count, and source URL. The canonical API contract remains in the
Public API; this SDK consumes its published mirror.

The generator runs offline, checks the hash, and maps every operation exactly
once through `contracts/service-map.json`. It generates parameter types,
response models, services, constants, runtime metadata, and the endpoint
reference. Keep English reference wording in `docs/reference.en.json`.
Missing translations and unsupported schema forms fail generation.
Named or top-level input objects with additional properties and non-scalar
query parameters require explicit representation and serialization support.
Until that support exists, generation rejects them. Inline map fields retain
their existing representation.

```sh
python scripts/generate.py
python scripts/generate.py --check
python scripts/check-api.py --check
```

Choose an approved published revision before updating the snapshot. Review
authentication, environments, fields, nullability, statuses, pagination,
idempotency, and schema composition. Regenerate and review all changed files.
Do not add an endpoint solely because it exists in a local API checkout.

`contracts/rust-api.json` independently records the public Rust declarations,
type signatures, serialization attributes, and trait implementations. It also
records compiler checks for `Send`/`Sync` on public types and `Send` on request
and pagination futures. The checker compiles an independent consumer under
`.tmp`, so a private implementation change cannot silently remove those
concurrency guarantees. It uses the local Cargo cache without network access;
run `cargo fetch --locked` first when setting up a new checkout. The generator
does not update the baseline. Review a compatibility change before explicitly
running `python scripts/check-api.py --write` and choosing the SDK version.

## Validation and packaging

Use Python 3.12 or later for the complete engineering gate. Offline generation
and the public API baseline checker support Python 3.8 or later.

```sh
python scripts/check.py --prepare-tools
```

The launcher calls the canonical Rust adapter in the sibling Developer
Toolkit. The adapter materializes its suite in a new `.tmp` directory and
checks generated files, compatibility, Cargo formatting, lint, compilation,
documentation, contract coverage, transport, pagination, webhooks, dependency
advisories, SAST, and an independent consumer of the packaged crate. Validation
uses loopback and fictitious data without calling the real financial API.
Timeout regressions cover headers received before a delayed body, waits before
headers, and retry backoff. They check correlation metadata and one diagnostic
notification for each started attempt.
The suite also checks published request and success examples through both
JSON text and `serde_json::Value`. Numeric regressions cover unknown fields
at the root and in nested models, integers around 64-bit and 128-bit limits,
precise decimals, and each revocation response variant. Known integer types,
required fields, nullability, and duplicate-field checks remain enforced.

All tests, simulators, supporting scripts, validation dependencies, build
artifacts, and reports belong only in `C:\server\raims\.tmp\raims-rust-sdk`.
The Cargo configuration sends local build output there. The package allowlist
contains runtime source, metadata, licenses, and user guides. Inspect the
actual archive and validate it outside the checkout before distribution.

Freeze SDK and Toolkit sources during a gate. Changing either invalidates
affected evidence. Run the workspace preflight Commit stage before a delivery
commit and Deploy before an authorized publication. Stop temporary processes
and preserve the user's existing processes.

## Distribution and support

The initial distribution provides source code, tags, release notes, and local
crate archives on GitHub. crates.io distribution remains a separate step.
Future pushes, tags, releases, crates.io publication, and public guide
publication require authorization.
CI remains a template until runner access and activation are approved.
Keep the sources uncommitted and unstaged until a commit is requested.

For the confirmed distribution, prepare BR/EN installation and usage
guides in the existing developer documentation catalog and update the support
knowledge map. Explain purpose, requirements, steps, access, permissions,
key-scoped environments, simulations, availability, limits, cursor navigation,
omission/null, idempotency, pending responses, errors, and webhooks. Regenerate
the catalog and validate guidance before marking the knowledge item complete.
This client library adds no support context state or financial query tool.

Keep the knowledge and changelog items in private tracking outside this
repository. Publish the changelog using the real availability date. A local
checkout must not be presented as a registry installation.

Adding this consumer does not change public API behavior or its OpenAPI.
Existing CLI commands and pinned clients therefore need no regeneration.
