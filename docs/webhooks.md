# Receiving webhooks

Preserve the original HTTP body bytes and every occurrence of the signing
headers. Verification rejects missing or repeated headers, including names
that differ only in letter case. Pass the original values to the SDK before
parsing or changing the body.

```rust
use std::time::SystemTime;
use raims::{construct_webhook_event, DEFAULT_WEBHOOK_TOLERANCE};

let event = construct_webhook_event(
    raw_body,
    &signing_headers,
    &webhook_secret,
    DEFAULT_WEBHOOK_TOLERANCE,
    SystemTime::now(),
)?;
```

`raw_body` is a `&[u8]`, `signing_headers` is a list of `(&str, &str)` pairs,
and `webhook_secret` is the `whsec_` value held in your secret store. Include
`webhook-id`, `webhook-timestamp`, and `webhook-signature`. Preserve all header
occurrences when adapting your HTTP framework; collapsing them into a map
before verification can hide duplicate values.

`verify_webhook_signature` checks the signature without decoding JSON.
`construct_webhook_event` verifies first, then returns a JSON object. Both use
the exact body bytes and constant-time HMAC-SHA256 comparison. Multiple
`v1` signatures in a single signature header support secret rotation.

The default timestamp tolerance is five minutes in either direction. A zero
tolerance requires the same Unix second and does not disable verification.
Keep your server clock synchronized. Invalid secrets, signatures, timestamps,
headers, and event bodies produce a signature-verification error with scrubbed
formatting.

After verification, check the event's company and environment for your
integration, deduplicate its event ID durably, and persist the work before
acknowledging the delivery. A verified delivery can still be retried and does
not authorize a financial operation on its own. Keep signing secrets,
signatures, and payloads out of routine logs.
