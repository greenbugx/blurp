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
cargo run
cargo fmt --all -- --check
cargo check
cargo test
```

## License

This project is licensed under the MIT license [LICENSE](./LICENSE)