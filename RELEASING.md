# Releasing slint-kit crates

Seven libraries are prepared for their first crates.io release at `0.1.0`.
`slint-widgets-gallery` and `slint-widgets-tests` are development tools with
`publish = false`. Libraries can be released individually; their version numbers
do not need to remain synchronized.

## Release checks

Use the current stable Rust toolchain. This project does not yet promise a
minimum supported Rust version. Slint consumers currently target Slint 1.18;
the headless test harness pins its compiler and testing backend to 1.18.1.

1. Review the target crate's public API, README, feature flags, and changelog.
   Update its version and any dependent workspace version requirements. Keep
   path dependencies paired with registry versions in publishable manifests.
2. Run the checks below. CI runs formatting, Clippy, tests, strict documentation,
   examples, and package verification on macOS, Linux, and Windows. Wait for
   those checks on the release commit, and perform the native smoke checks in
   the root README when platform behavior changes.
3. Inspect the archives under `target/package`. Each includes its own README
   and MIT license. `emoji-shortcodes` must also contain both TSV datasets and
   their upstream notices; `slint-widgets` must contain every `ui/*.slint` file.
4. Commit the reviewed changes before publishing. Release archives should come
   from a clean checkout. Local packaging during development can use
   `--allow-dirty`; release commands should not.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p md-segments --no-default-features
cargo test -p desktop-clipboard --no-default-features
cargo test -p desktop-notify --no-default-features
cargo doc --workspace --all-features --no-deps --config 'build.rustdocflags=["-D", "warnings"]'
cargo doc -p md-segments --no-default-features --no-deps --config 'build.rustdocflags=["-D", "warnings"]'
cargo doc -p desktop-clipboard --no-default-features --no-deps --config 'build.rustdocflags=["-D", "warnings"]'
cargo doc -p desktop-notify --no-default-features --no-deps --config 'build.rustdocflags=["-D", "warnings"]'
cargo run -p palette-rank --example rank
cargo run -p slint-model-sync --example reconcile
cargo deny check licenses sources bans
cargo package --workspace --exclude slint-widgets-gallery --exclude slint-widgets-tests --all-features
```

Package the libraries together for the initial preflight: Cargo can then verify
`md-segments` against the packaged `emoji-shortcodes` before either is published.
This command builds the extracted archives without uploading them. To inspect
one crate's file list, run `cargo package -p slint-widgets --list` (substitute the
crate you are reviewing).

## Publication order

Crate names are allocated on first publication. Before the first upload, check
availability again and authenticate with `cargo login` using an account that
can publish. Do not store registry tokens in this repository.

Publish `emoji-shortcodes` first. Wait for it to be available in the registry
before publishing `md-segments`. The other five libraries have no dependencies
on other slint-kit crates and can be published independently.

Run these commands individually after checking each result:

```sh
cargo publish -p emoji-shortcodes
cargo publish -p md-segments
cargo publish -p palette-rank
cargo publish -p desktop-clipboard
cargo publish -p desktop-notify
cargo publish -p slint-model-sync
cargo publish -p slint-widgets
```

Use `cargo publish -p <crate> --dry-run` for a final check before each upload.
For `md-segments`, that standalone dry run requires `emoji-shortcodes` to have
been published already. Publishing a version is permanent: its contents cannot
be overwritten. See Cargo's [publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html).

## After publication

Confirm each released version's crates.io README and docs.rs build. Record the
release date in `CHANGELOG.md` and create an annotated tag for each published
crate, for example `emoji-shortcodes-v0.1.0`, pointing at the release commit.
Push the tags. After the first releases are available, replace the root README's
git dependency example with the corresponding crates.io dependency.

The per-crate READMEs already show the intended `0.1.0` registry dependency for
readers of the published packages. No upload or tag creation is part of release
preparation itself.
