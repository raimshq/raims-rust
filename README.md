# Raims Rust

Official Rust SDK for the Raims public API. Use typed resource services,
cursor pagination, structured errors, and webhook signature verification in
server applications. Requires Rust 1.90 or later and a Tokio runtime.

## Install from crates.io

Add the official [`raims` crate](https://crates.io/crates/raims) from your
application's directory:

```sh
cargo add raims
cargo add tokio --features macros,rt-multi-thread
```

Cargo selects the current compatible stable release and records the resolved
versions in `Cargo.lock`. Keep the lockfile under version control and run
`cargo update -p raims` to update within the manifest's requirement. See the
installation guide when an update requires changing that requirement. Test
your integration before deploying an update.

Source code and release assets remain available on
[GitHub](https://github.com/raimshq/raims-rust/releases). See
[installation and updates](docs/installation.md) for migration from a Git or
path dependency, release archives, and local source checkouts.

## First request

```rust
use raims::RaimsClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RaimsClient::new(std::env::var("RAIMS_API_KEY")?)?;
    let balance = client.balances().retrieve().await?;
    println!("HTTP status: {}", balance.last_response.status_code);
    Ok(())
}
```

Reuse the client across requests and tasks. Clones share the connection pool.
The key determines the company, environment, and permissions. The default
base URL is `https://api.raims.com`. Keep keys and personal data on your server.

## Create a charge

With a customer ID and order data from your application:

```rust
use raims::{params::CreateChargeParams, Optional, RequestOptions};

let params = CreateChargeParams {
    customer_id: Optional::Value(customer_id),
    amount_cents: Optional::Value(15000),
    due_date: Optional::Value(due_date_unix),
    title: Optional::Value("Order 1234".to_owned()),
    ..Default::default()
};
let options = RequestOptions {
    idempotency_key: Some(persisted_order_key),
    ..Default::default()
};
let response = client.charges().create_with_options(&params, &options).await?;
let pending = response.last_response.status_code == 202;
```

`customer_id` is a `String` returned by the API, `due_date_unix` is an `i64`
in Unix seconds UTC, and `persisted_order_key` is a `String` identifying the
business intent. Persist the key and parameters before sending the request.
`15000` represents BRL 150.00. A `202` response is pending; inspect the resource
before treating the operation as complete or attempting the intent again.

## Services

| Service | Resources |
| --- | --- |
| `accounts()` | Account context, number, fees, and checkout settings |
| `balances()` | Balance and forecast |
| `customers()` | Customer registration, lookup, and removal |
| `charges()` | Charges, payment details, documents, and refunds for a charge |
| `payment_links()` | Payment links and requests to send them by email |
| `refunds()` | Refunds and receipts |
| `disputes()` | Disputes |
| `statements()` | Statements |
| `withdrawals()` | Withdrawals, limits, summary, and automatic settings |
| `withdrawal_destinations()` | Withdrawal destinations |
| `webhooks()` | Subscriptions, pause, health, tests, and secret rotation |
| `webhook_event_types()` | Event types |
| `simulations()` | Charge and refund simulations with a Sandbox key |

Each service exposes the operations its resource supports. Methods accept
typed parameters and return `ApiResponse<T>` with typed data, HTTP metadata,
and the original JSON. Use a method's `_with_options` variant to pass
request options such as an idempotency key.

Amounts remain integer cents and resource dates remain Unix seconds UTC.
`Optional::Unset`, `Optional::Null`, and `Optional::Value(...)` preserve the
difference between leaving an input absent, clearing it, and setting a value.

## Guides

- [Configuration and partial updates](docs/usage.md)
- [Pagination](docs/pagination.md)
- [Errors, idempotency, and retries](docs/errors-and-retries.md)
- [Receiving webhooks](docs/webhooks.md)
- [Endpoint reference](docs/reference.md)
- [Contributing](CONTRIBUTING.md) and [security](SECURITY.md)
- [Public API documentation](https://docs.raims.com/en/api)

Distributed under the [MIT license](LICENSE).
