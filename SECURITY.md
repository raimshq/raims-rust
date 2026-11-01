# Security

Report vulnerabilities privately to `security@raims.com`. Include the affected
version, observed behavior, and a minimal reproduction with fictitious data.
Keep credentials, personal data, webhook secrets, and temporary URLs out of
public discussions.

Use this SDK in server applications. Store API keys and webhook secrets in your
secret store and grant only the permissions your integration needs. Keep Rust
and the resolved dependencies updated. Your application remains responsible for
authorization, checking event environments, and persisting idempotency keys.

Webhook verification authenticates a delivery. Validate its meaning, deduplicate
the event, and persist the work before acknowledging the HTTP request.
