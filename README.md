# blurp

A local-first, keyboard-driven terminal application that turns
rough thoughts into editable message drafts.

## Goals

- Interactive full-screen terminal UI
- Process multiple message intentions from one brain dump
- Edit, regenerate, copy, snooze, and complete drafts
- Local open-weight AI inference
- Local SQLite persistence
- No mandatory cloud backend or remote inference API

## Stack

- Rust
- Ratatui + Crossterm
- Tokio
- Ollama for local inference
- SQLite via rusqlite

## Development

```bash
cargo run            # empty workspace
cargo run -- --demo  # in-memory sample drafts for UI work
cargo fmt --all -- --check
cargo check
cargo test
```

`--demo` loads a small set of clearly labeled sample drafts. They live only
in memory, are never written to local storage, and reset when the app exits.
The normal launch path starts empty and is reserved for real persisted data.

## License

This project is licensed under the MIT license [LICENSE](./LICENSE)