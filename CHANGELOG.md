# Changelog

## 0.1.0 - 2026-11-01

First GitHub source release of the Raims Rust SDK, for server applications
using Rust 1.90 or later and Tokio.

- Asynchronous resource services and typed parameters and response models for
  the public API pinned with this release.
- Cursor pagination with forward and backward navigation, preserving filters
  and request options after failed fetches.
- Explicit omission and null in inputs, integer cents, Unix timestamps, and
  exact JSON numbers in unknown response fields.
- Structured errors, request metadata, optional read retries, and scrubbed
  diagnostics. Writes are sent once; pending results do not start polling.
- Webhook signature verification using the original body bytes, including
  timestamp validation and secret rotation.

Install from GitHub as described in the [installation guide](docs/installation.md).
This release does not publish the crate on crates.io.
