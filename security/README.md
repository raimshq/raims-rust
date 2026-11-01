# Security checks

The canonical validation runs the local Semgrep rules and audits the resolved
runtime dependencies for published advisories. Missing tools, failed scans,
and unavailable advisory services fail the gate.

The rules cover disabled TLS verification, unsafe source, shell execution in
tooling, and accidentally embedded credentials. Runtime regressions also check
redirect handling, write replay, and scrubbed diagnostics. These checks do not
replace a review of changes to authentication or transport behavior.
