# Pagination

List methods return one `ApiResponse<T>`. Its typed data contains `list` and
`page`. Reading the returned list never sends another request.

Use a service's `_pages` helper to navigate lazily:

```rust
use raims::{params::ListCustomersParams, Optional, RaimsClient};

let client = RaimsClient::new(std::env::var("RAIMS_API_KEY")?)?;
let filters = ListCustomersParams {
    page_size: Optional::Value(20),
    ..Default::default()
};
let mut pages = client.customers().list_pages(&filters)?;
while let Some(response) = pages.next_page().await? {
    for customer in response.data.list {
        process_customer(customer);
    }
}
```

`process_customer` is your application handler. Creating the pager does not
send HTTP. `next_page().await` fetches the first page, then follows
`page.next_after` while `page.has_next` is true. `previous_page().await` uses
`page.previous_before` after a page has been received. Each call returns
`None` when navigation in that direction is finished.

Filters and per-request options remain attached to the pager. For explicit
options, use `_pages_with_options`. A failed or cancelled fetch leaves the
cursor unchanged so the application can retry that same page. Repeated cursors
are rejected to stop endless traversal.

For manual navigation, copy the API cursor unchanged, keep the same filters,
and send `after` or `before`, never both. Restart without a cursor if the API
reports it as invalid. Do not parse cursors or construct them from resource IDs.
