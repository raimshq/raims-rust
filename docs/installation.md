# Installation and updates

The official [`raims` crate](https://crates.io/crates/raims) is distributed
through crates.io. Requires Rust 1.90 or later. Applications need Tokio with a
runtime; enable its `macros` feature to use `#[tokio::main]`.

## Install from crates.io

From your application's directory:

```sh
cargo add raims
cargo add tokio --features macros,rt-multi-thread
```

Cargo adds the current compatible stable release to `Cargo.toml` and records
the resolved versions in `Cargo.lock`. Keep the lockfile under version control.
The SDK uses the asynchronous reqwest transport with TLS certificate
verification enabled. Consumers need Rust and the declared Cargo dependencies;
Python and the engineering Toolkit are not runtime requirements.

## Updates

Review the release notes and update the SDK within the requirement currently
declared in your application:

```sh
cargo update -p raims
```

Cargo updates `Cargo.lock` conservatively. It updates transitive dependencies
when the SDK update requires it, while retaining other locked versions where
possible. This command does not widen the requirement in `Cargo.toml`.

For a release outside that requirement, review the current stable release on
crates.io and adjust the requirement in `Cargo.toml` before updating. Running
`cargo add raims` alone can retain an existing requirement.

Review the manifest and lockfile changes, compile, and run your application's
integration checks before deployment. Adding a dependency does not apply
future SDK updates automatically.

## Migrate from Git or a local path

Remove the existing `raims` dependency entry from the manifest that declares
it. Preserve any dependency options your application uses so you can reapply
them. For an optional dependency, also preserve and temporarily remove its
references in `[features]`, such as `dep:raims`, to keep the manifest valid.
After adding the crate, restore those declarations along with its `optional`
and feature settings. Keep `Cargo.lock`; Cargo will update its source and
resolved versions. Then run from your application's directory:

```sh
cargo add raims
```

Review the resulting entry in `Cargo.toml`. It must use the registry version
requirement selected by Cargo and must no longer contain `git`, `path`,
`branch`, `tag`, or `rev`. When the dependency is inherited from a workspace,
make the source change in that workspace's `[workspace.dependencies]` entry.
The Rust crate name and `use raims::...` imports remain the same.

Compile and run your application's checks after the migration. A Git or path
dependency can otherwise keep selecting its existing source when you run
`cargo add`; specifying a registry alone does not remove that source.

## Install from GitHub

With Git installed, you can use the repository's default branch instead:

```sh
cargo add raims --git https://github.com/raimshq/raims-rust
```

Cargo records the selected revision in `Cargo.lock`. To refresh a Git
dependency, run `cargo update -p raims` and test the selected source before
deployment.

## Release archives and local source

Download the source or `.crate` archive from the
[current release](https://github.com/raimshq/raims-rust/releases/latest) and
extract it outside your application. A `.crate` file is a gzip-compressed tar
archive. From your application, add the extracted directory:

```sh
cargo add raims --path /path/to/Raims-Rust
```

Use the same command for a local source checkout. Keep that directory at the
configured path.

## Build a local archive

To build an installable source archive from the SDK checkout:

```sh
cargo package --locked --allow-dirty --target-dir /path/to/artifacts
```

Choose an output directory outside the SDK checkout. Cargo writes a versioned
`.crate` file inside its `package` output directory and verifies that the
packaged source compiles. The archive is a gzip-compressed tar file.
Extract it and add the extracted package with `cargo add raims --path ...`.

For a path dependency, replace the checkout or extracted package with the
reviewed source version. Refresh the dependency entry from your application:

```sh
cargo add raims --path /path/to/Raims-Rust
```

Review the manifest and lockfile changes and run your integration checks.
