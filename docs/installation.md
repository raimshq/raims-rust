# Installation and updates

The SDK is distributed through
[GitHub](https://github.com/raimshq/raims-rust/releases), with source code and a
`.crate` archive for each release. It is not yet available on crates.io.
Requires Rust 1.90 or later. Applications need Tokio with a runtime; enable
its `macros` feature to use `#[tokio::main]`.

From your application's directory:

```sh
cargo add raims --git https://github.com/raimshq/raims-rust
cargo add tokio --features macros,rt-multi-thread
```

Cargo adds the Git dependency to your application's manifest and records its
resolved revision in `Cargo.lock`. The SDK uses the asynchronous reqwest
transport with TLS certificate verification enabled.

## Release archives and local source

Download the source or `.crate` archive from the
[current release](https://github.com/raimshq/raims-rust/releases/latest) and
extract it outside your application. A `.crate` file is a gzip-compressed tar
archive. From your application, add the extracted directory:

```sh
cargo add raims --path /path/to/Raims-Rust
```

Use the same command for a local source checkout. Keep that directory at the
configured path. Consumers need only Rust and the declared Cargo dependencies,
without Python or the engineering Toolkit.

## Build a local archive

To build an installable source archive from the SDK checkout:

```sh
cargo package --locked --allow-dirty --target-dir /path/to/artifacts
```

Choose an output directory outside the SDK checkout. In the Raims engineering
workspace use its `.tmp/raims-rust-sdk` directory. Cargo writes
`package/raims-0.1.0.crate` inside the selected target directory and verifies
that the packaged source compiles. The archive is a gzip-compressed tar file.
Extract it and add the extracted package with `cargo add raims --path ...`.

## Updates

For a Git dependency, review the release notes, then repeat the add command
to refresh the dependency and its version requirement:

```sh
cargo add raims --git https://github.com/raimshq/raims-rust
```

For a path dependency, replace the checkout or extracted package with the
reviewed source version. From your application's directory, run the add
command again with the path to that source:

```sh
cargo add raims --path /path/to/Raims-Rust
```

Cargo refreshes the dependency path and version requirement in `Cargo.toml`
and updates `Cargo.lock`. This also handles a source version outside the
previously declared compatible range. `cargo update` alone only selects
versions allowed by the existing requirement; it does not widen that range.

Review the source changes and run your integration checks before deployment.
