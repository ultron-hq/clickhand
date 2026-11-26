# Contributing to Ultron

Thanks for helping. This guide covers how the port works and what a good pull request looks like.

## How the rewrite works

Ultron started as a Python app. The Rust version is built **unit by unit**, and each unit has to
behave like the original before it is merged:

1. Pick an `open` unit in [rust/MAP.md](rust/MAP.md) (or an issue), and comment that you are taking it.
2. Read the decisions that cover it in [rust/DECISIONS.md](rust/DECISIONS.md) and the names in
   [rust/GLOSSARY.md](rust/GLOSSARY.md).
3. Port it. Logic units are checked against recordings of the original with
   [rust-anything](https://github.com/erkamyaman/rust-anything) (`record`, `diff`, `gate`); UI units
   are checked with screenshots in the pull request.
4. Mark the unit `done` in `rust/MAP.md` with its results.

Maintainers hold the Python original and make the recordings; if your unit needs new cases, say
which inputs and we will record them.

## Development

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Every change must build and pass tests on Windows, macOS and Linux; CI checks all three.

## Rules of the road

- **Same behavior first, improvements second.** A port that changes behavior needs an ADR in
  `rust/DECISIONS.md` saying why.
- **Data formats are frozen** (ADR-005): the SQLite schema and config files must stay readable by
  both versions.
- **Safety rules are not optional**: destructive desktop actions only on a request from the user,
  the kill switch always works, typed text is never logged.
- **No secrets, personal paths or recordings with personal data** in commits.
- Platform-specific code goes behind a trait with one module per OS (`linux.rs`, `windows.rs`, `macos.rs`).
- Small pull requests, one unit each, with a description of what you checked.

## Commit messages

Short imperative subject (`Port sessions store`), a body explaining why when it isn't obvious.

## License

By contributing you agree that your work is dual licensed under MIT OR Apache-2.0, as the
project is.
