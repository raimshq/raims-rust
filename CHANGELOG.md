# Changelog

## 0.1.2 - 2026-11-07

Response metadata now reads the canonical `Request-Id` header.
The `X-Request-Id` fallback has been removed.

## 0.1.1 - 2026-11-02

The official `raims` crate is available on crates.io. Install it with
`cargo add raims`, and update an existing registry dependency with
`cargo update -p raims`. The installation guide explains how to migrate a
Git or path dependency while preserving the application's lockfile.

The public Rust API, pinned contract, and runtime behavior are unchanged.

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
