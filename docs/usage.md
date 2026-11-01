# Configuration and inputs

## Client lifetime

Create a `RaimsClient` with an API key from your secret store. Reuse or clone
it across tasks. Clones share the HTTP connection pool; dropping the final
client releases it when outstanding requests finish. Run requests inside a
Tokio runtime.

The default request timeout is 80 seconds, including response reading, retries,
and backoff. Configure a positive timeout up to 300 seconds. Read retries
default to zero and accept values from zero to five.

```rust
use std::time::Duration;
use raims::RaimsClient;

let client = RaimsClient::builder(std::env::var("RAIMS_API_KEY")?)
    .timeout(Duration::from_secs(40))
    .max_read_retries(2)
    .build()?;
```

The key determines the company, environment, and permissions. Use a Sandbox
key for simulations. There is no environment selector or OAuth flow in this
SDK. Requests authenticate with Bearer and never send `X-Raims-Environment`.

The production base URL is `https://api.raims.com`. A custom base URL must use
HTTPS; HTTP is accepted only for local loopback validation. The SDK keeps
certificate verification enabled and does not follow redirects. It does not
upload files to presigned URLs or download receipts from external locations.
Use a separate client without Raims credentials for those operations.

## Typed inputs

Parameter types live in `raims::params`; response types live in `raims::models`.
Field names follow Rust snake_case and serialize to the public API names.
Use `Default::default()` for unspecified input fields.

`Optional<T>` represents three different instructions:

| Value | Serialized input |
| --- | --- |
| `Optional::Unset` | The field is absent |
| `Optional::Null` | The field is explicitly `null` |
| `Optional::Value(value)` | The field contains `value` |

Sending null requires a nullable field in the contract. For updates, omission
leaves the field unchanged while null can clear a field where supported.
Empty objects and arrays remain empty rather than becoming absent.

Use integer cents for monetary values. For example, `15000` represents
BRL 150.00. Resource timestamps use Unix seconds UTC. Inputs are checked
against the pinned contract before a request is sent; business rules and
authorization remain the responsibility of the API.

## Responses and metadata

`ApiResponse<T>` contains `data`, `last_response`, and `raw`. The typed data
supports normal Rust field access. `raw` preserves the original JSON,
including unknown fields and the difference between absent fields and null.
Future response fields and enum values remain readable. Displaying response
data is an explicit application decision; keep it out of routine logs.
Unknown fields preserve exact JSON numbers, including integers outside
64-bit ranges and values inside nested objects or arrays. Known financial
and timestamp fields keep their declared types.
Optional fields in the typed view use `Option<T>` for both absence and null.
Use `raw` when that distinction matters; serializing the typed view does not
reconstruct the original field presence.

`last_response` exposes the status code and Request-Id, along with available
idempotency and retry metadata. Inspect the HTTP status for pending results:
`202` accepts work without confirming its completion. The SDK returns it
directly and does not start polling or repeat the write.

## Per-request options

Use the `_with_options` variant of a resource method for a request timeout,
read-retry setting, explicit API key override, or persisted idempotency key.
An override key determines that request's scope. Keep it in the secret store
and preserve the corresponding options when navigating pages.

Diagnostics report only operation metadata, attempts, elapsed time, status,
and Request-Id. They omit IDs, credentials, query strings, and payloads.
