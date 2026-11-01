# Errors, idempotency, and retries

## Structured errors

Requests return `Result<ApiResponse<T>, Error>`. Use the error category and
HTTP metadata to distinguish local validation, authentication, permission,
missing resources, conflicts, rate limits, API failures, connection failures,
timeouts, and invalid responses. An API rejection may include a code and field
details for your application to inspect. Treat the server's text and details
as sensitive data before showing them to a person or storing them.

Default error formatting and diagnostics omit credentials, resource IDs,
query strings, and payloads. A Request-Id helps support investigate a failed
operation without receiving your API key or the full response.
If the total timeout expires after headers arrive, the error retains the last
received HTTP metadata. Diagnostics report the timed-out attempt once. A
timeout before headers has no response metadata, and an expired retry wait
does not create an additional attempt.

## Idempotency

Persist a key and its request parameters with the business intent before
sending an idempotent write. Supply that key through `RequestOptions`:

```rust
use raims::RequestOptions;

let options = RequestOptions {
    idempotency_key: Some(persisted_order_key),
    ..Default::default()
};
```

Pass `&options` to the method's `_with_options` variant. The SDK accepts a key
only for operations that support it. It never creates one automatically.
After a lost response, reconcile the operation and reuse the same intent and
parameters according to the API's idempotency rules. A new key represents a
new intent and can create another financial operation.

## Retry policy

Automatic retries are disabled by default. Configure `max_read_retries` to
retry eligible GET failures. The SDK honors `Retry-After` in seconds or HTTP
date form and keeps retry waits within the request timeout. Writes are sent
once, including when they carry an idempotency key. The underlying transport
also has automatic retries disabled.
When the server requests a delay longer than 30 seconds, the SDK returns the
error for your application to schedule a later attempt.

A `202` response is returned as a pending result. Check the resource before
treating it as complete. The SDK does not poll pending work automatically.
Dropping a request future cancels the local wait; it does not reverse work
that the API may already have accepted.
