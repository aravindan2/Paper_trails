# Paper Trails — Rust + WASM CLI Detective Game

A single-player detective mystery played from a terminal. The player
investigates a fictional incident by examining a simulated computer system:
reading logs, searching files, inspecting bytes, and reconstructing a
timeline.

> The computer is the crime scene. Computer science is how you solve the crime.

**Current status: Milestone 1 complete** — native CLI prototype with a virtual
filesystem and the first command set. Case content is placeholder scaffolding;
forensic tools, case authoring, WASM, and the web UI come in later milestones
(see the project brief).

## Repository layout

```text
paper_trails/
├── Cargo.toml              # workspace root
├── crates/
│   └── game-core/          # shared engine (no dependencies, WASM-ready)
│       ├── src/
│       │   ├── lib.rs      # module map + re-exports
│       │   ├── vfs.rs      # virtual filesystem + path normalization
│       │   ├── command.rs  # tokenizer + typed command parsing
│       │   ├── engine.rs   # command execution + player state
│       │   └── case.rs     # Case 001 scaffolding assets (placeholder)
│       └── tests/
│           └── session.rs  # end-to-end player session tests
├── apps/
│   └── cli/                # thin native adapter (stdin → engine → stdout)
│       └── src/main.rs
├── tests/                  # (workspace-level tests added in later milestones)
└── README.md
```

The engine lives in `crates/game-core` and knows nothing about terminals or
browsers, so the same crate will back both the native CLI and the future
WASM build.

## Build, test, and run

Requires a stable Rust toolchain (tested with cargo 1.96, edition 2024).

```bash
# Build everything
cargo build --workspace

# Run all unit and integration tests
cargo test --workspace

# Play the game
cargo run -p paper-trails-cli
```

Type `help` at the prompt, then navigate and search. `Ctrl-D` exits.

## Supported commands

| Command | Behavior |
| --- | --- |
| `help` | list supported commands and notes |
| `pwd` | print the current virtual directory |
| `ls [PATH]` | list a directory (default: cwd); directories shown with trailing `/` |
| `cd [PATH]` | change directory; bare `cd` returns to `/` |
| `cat PATH...` | print text files; operands are concatenated in order, like real `cat` |
| `grep [-n] PATTERN PATH` | search **one** file; `-n` prints line numbers |

### Documented subset (intentional deviations from Unix)

- **One filesystem only.** Player input is parsed in-process; nothing is ever
  executed on the host system. Shell operators (`&&`, pipes, redirects,
  globbing) are unknown commands.
- **Traversal above `/` is an error**, not silently clamped:
  `cd: ../../..: path escapes the virtual root`.
- **`grep` takes a single file.** Directories error with
  `grep: /var: Is a directory`. Only `-n` is supported; unknown options are
  rejected with a usage message. Non-UTF-8 files report
  `grep: PATH: binary file matches` when the pattern occurs in the raw bytes.
- **`cat` requires UTF-8.** Binary files get a clear refusal instead of a byte
  dump (`strings`/`hexdump` arrive in Milestone 2).
- **Quotes group arguments** (`"` and `'`); escape sequences inside quotes are
  not processed. An unterminated quote is a syntax error.
- **Directories are derived from file paths.** Empty directories cannot be
  represented yet, and `stat` is not implemented until Milestone 2.
- **`ls` output is sorted** (files in a `BTreeMap`), keeping tests and the
  future browser build deterministic.

## Key design decisions

1. **Zero dependencies in `game-core`.** Milestone 1 needs only `std`; this
   also keeps the crate compatible with `wasm32-unknown-unknown` later. Each
   future dependency must justify its WASM compatibility.
2. **Typed command parsing.** `Command::parse` returns a `Result<Command,
   ParseError>`; the engine only dispatches. There is no stringly `if
   command == ...` chain mixing parsing, filesystem behavior, and story logic.
3. **Bytes are the source of truth.** `VirtualFile.contents` is `Vec<u8>` from
   day one, so binary artifacts are first-class (`cat`/`grep` layer on top).
4. **Deterministic time.** Metadata uses fixed Unix timestamps formatted to
   UTC by a small in-crate formatter — no chrono, no local-timezone surprises
   in tests.
5. **`CommandOutput` is the state-change channel.** Evidence discoveries and
   objective updates will attach to `execute()` in Milestone 3 without changing
   the adapter contract.
6. **Placeholder case content is labeled.** `crates/game-core/src/case.rs`
   marks every file as scaffolding; no story facts are committed yet. Final
   case data becomes data-driven and the answer key stays out of public assets.

## Testing

```bash
cargo test --workspace
```

Coverage includes path normalization (`/a/../b`, relative paths, `.`/root
behavior), traversal rejection, missing files vs. directories, binary files,
quoted arguments, malformed commands, `grep` line numbers, exact session
outputs, and case-image structure.

## What remains (next milestones)

- **Milestone 2 — forensic toolkit:** `stat`, `strings`, `hexdump`,
  `sha256sum`, `decode`, richer binary fixtures, malformed-input hardening.
- **Milestone 3 — Case 001:** author the real artifacts and timeline, evidence
  registry, objectives, `evidence`/`timeline`/`submit`, solution tests kept
  separate from public assets.
- **Milestone 4 — WASM:** thin `wasm-bindgen` adapter, `wasm-pack` build,
  native/browser parity checks.
- **Milestone 5 — Web terminal UI:** keyboard-first terminal, history,
  scrolling, evidence journal, `localStorage` save/restore.
- **Milestone 6 — Deployment:** static hosting, MIME types, no answer key in
  public assets.
