# Raims Rust SDK

Read README.md, CONTRIBUTING.md, and docs/maintenance.md before changing this SDK.

- Write repository descriptions and Markdown documentation in English.
- Keep internal TODO lists, backlogs, and roadmaps outside this public repository
  and its packages. Use private tracking without publishing its contents,
  location, or links.
- The crate is `raims`, with Rust 1.90 or later and an asynchronous `RaimsClient`.
  Preserve typed services and the shared client suitable for concurrent tasks.
- Consume only the published public contract pinned in `contracts/source.json`.
  Preserve the original bytes and SHA-256. Do not import host implementation code
  or add unpublished endpoints from a local API checkout.
- Run `python scripts/generate.py`. Files named `*_generated.rs`, runtime
  contract metadata, and docs/reference.md are generated. Do not edit them
  manually. The service map must cover every pinned operation exactly once.
- Keep English reference wording in docs/reference.en.json. Missing wording
  must fail generation. Never translate or rewrite the pinned contract itself.
- Check the independent public API baseline with
  `python scripts/check-api.py --check`. Only an explicit `--write` updates it;
  review all changes before accepting a new baseline.
- Preserve integer cents, Unix seconds, unknown response fields and enum values,
  omitted inputs, and explicit null. Do not print credentials or customer data
  through Debug, Display, errors, or diagnostics.
- The API key determines company, environment, and permissions. Do not add an
  environment selector, OAuth, arbitrary headers, or a public arbitrary-request
  method. Keep TLS verification enabled and redirects disabled.
- Never generate idempotency keys, retry writes, poll after 202, upload files,
  or download external URLs. Optional retries apply only to GET.
- Tests, simulators, support scripts, tools, builds, and reports belong only in
  `C:\server\raims\.tmp\raims-rust-sdk`. Never version or package them. Use the
  configured external target directory or pass an explicit external target.
- Run the canonical `python scripts/check.py` gate before completing changes.
  It reconstructs tests through Developer-Toolkit and checks the contract,
  transport, pagination, webhooks, compatibility, security, and packaged consumer
  without calling the financial API. Stop temporary processes before finishing.
- Leave changes uncommitted and unstaged. Remote creation, push, tags, releases,
  registry publication, and public guide publication require authorization.
  Keep CI as a template until runner access and activation are authorized.
- Describe Raims features directly. Preserve required licenses and attribution
  and apply the workspace humanizer skill to prose.
