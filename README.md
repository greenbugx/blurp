# blurp

blurp is a local-first, keyboard-driven terminal workspace that turns one raw
brain dump into separate, editable message drafts. You type a stream of
unrelated thoughts; a local open-weight model splits them into individual
messages, drafts each one, and flags whatever it cannot write responsibly as
missing context. You review, edit, copy, snooze, or complete each draft.
Everything stays on your machine.

## How it works

1. Press `n` and dump several unrelated intentions in one go.
2. blurp sends the dump to your local Ollama model with a strict system prompt
   that forbids inventing recipients, dates, amounts, or promises.
3. The reply is parsed and validated in Rust: required fields are strict, and
   anything untrusted (tone, status) is normalized, so a model can never mark
   a draft completed or sneak in malformed data.
4. Each draft lands in the workspace with its own status, tone, and a stable
   ID, and is saved to SQLite before it is shown as saved.

## Features 

- Full-screen TUI with a three-pane workspace, adaptive layouts, and
  keyboard-first navigation
- One brain dump produces multiple separate drafts
- Statuses: Inbox (review), Drafts, Snoozed, and Completed, with section
  filtering and persistence
- Edit a draft body and save it (persisted, flagged as hand-edited)
- Copy a draft to the system clipboard
- Per-draft tone change with local regeneration: Casual, Warm, Professional,
  Direct, Firm (hand-edited drafts ask before being overwritten)
- Mark complete, snooze and wake, delete with confirmation; the app never
  sends a message for you
- Search over draft titles, recipients, and bodies, entirely local
- SQLite persistence across restarts in the per-user data directory
- Demo mode (`--demo`) with clearly labeled in-memory sample drafts that are
  never written to storage
- Recoverable errors for Ollama down, missing model, timeouts, and malformed
  model replies, with the original dump always preserved

## Planned ideas

- Surfacing each draft's stored missing-details list in the UI (it is parsed
  and persisted today)
- Choosing a tone before the first generation (tone selection currently
  applies to per-draft regeneration)
- A settings screen for the model and storage location
- Optional `$EDITOR` integration, mouse support, and undo for deletion

## Requirements

- A current stable Rust toolchain (the project uses edition 2024)
- [Ollama](https://ollama.com) installed and running locally
- One pulled instruction model. blurp is tested with `gemma4:e4b` and `gemma4:e2b` while other instruction-tuned models should work.

## Install and run

```bash
git clone https://github.com/greenbugx/blurp.git
cd blurp
cargo run --release
```

The first launch creates an empty workspace. `cargo run -- --demo` starts
with in-memory sample drafts instead, for trying the UI without a model.

## Ollama setup

```bash
ollama pull gemma4:e4b
ollama serve
```

If Ollama runs as a systemd service or desktop app, it is already listening
on `http://localhost:11434` and you can skip `ollama serve`.

### Configuration (environment variables)

| Variable | Default | Meaning |
| --- | --- | --- |
| `BLURP_MODEL` | `gemma4:e4b` | Any model available in your local Ollama |
| `BLURP_OLLAMA_URL` | `http://localhost:11434` | Local Ollama endpoint |
| `BLURP_TIMEOUT_SECONDS` | `300` | Request timeout for one generation |

blurp talks only to the configured local endpoint and never falls back to a
cloud API.

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `n` | New brain dump |
| `Tab` / `Shift+Tab` | Switch pane |
| `j` / `k` or arrows | Move selection |
| `Home` / `End` | First / last item |
| `Enter` | Open / select; submit from the review step |
| `Ctrl+S` | Review a dump (composer), save edits (editor) |
| `x` | Discard a dump from the review step (with confirmation) |
| `e` | Edit the selected draft |
| `c` | Copy the selected draft to the clipboard |
| `t` | Change tone and regenerate the draft |
| `m` | Mark complete / move back to Drafts |
| `z` | Snooze / wake a draft |
| `d` | Delete a draft (with confirmation) |
| `/` | Search titles, recipients, and bodies |
| `Esc` | Back, cancel drafting, or clear the search |
| `?` | Help |
| `q` | Quit (with confirmation) |

## Demo workflow

A two-minute walkthrough lives in [DEMO.md](./DEMO.md). In short:

1. Launch blurp, press `n`, and dump: *Tell Anya I can't make it tonight, ask
   Dev whether the invoice was paid, and thank my professor and ask if a
   Monday submission is okay.*
2. Review the text, submit, and watch three separate drafts appear in the
   Inbox: a casual note to Anya, a direct invoice question to Dev, and a
   professional request to the professor, each with its own status.
3. Edit one draft, copy another, mark one complete, then quit and relaunch:
   every draft and status is still there.

## Where your data lives

- Linux: `~/.local/share/blurp/blurp.sqlite3`
- macOS: `~/Library/Application Support/blurp/blurp.sqlite3`
- Windows: `%APPDATA%\blurp\data\blurp.sqlite3`

Deleting the file resets the workspace; blurp recreates it on the next
launch. Nothing is synced anywhere.

## Privacy boundaries

- Drafts, dumps, and search never leave your machine. The only network
  destination is the Ollama endpoint you configure, on your own localhost by
  default.
- No analytics, telemetry, crash reporting, or auto-updates.
- No API keys exist because no hosted service is involved.
- The app never sends a message anywhere; completing a draft only records
  that you sent it yourself.

## Known limitations

- Small local models can be slow: a three-draft generation may take from a
  few seconds to a couple of minutes depending on your hardware. The
  timeout is configurable and the UI stays responsive while generating.
- Model output is strictly validated; a malformed reply shows a recoverable
  error and keeps your dump so you can retry.
- The stored missing-details list per draft is not yet displayed in the UI.
- Clipboard support needs a desktop session (Wayland or X11 provider).
- Single user, single machine, no sync. That is the point.
- Tested on Linux (Wayland) with `gemma4:e4b` and `gemma4:e2b`.

## Development

```bash
cargo fmt --all -- --check
cargo check
cargo test
```

The test suite covers model-contract parsing and validation, status
transitions, and SQLite behavior using in-memory and temporary databases, and
passes without Ollama or network access.

## License

This project is licensed under the MIT license [LICENSE](./LICENSE)
