# Paper Trails — Rust + WASM CLI Detective Game

A single-player detective mystery played from a terminal. The player
investigates a fictional incident by examining a simulated computer system:
reading logs, searching files, inspecting bytes, and reconstructing a
timeline.

> The computer is the crime scene. Computer science is how you solve the crime.

**Current status: Milestone 2 complete** — native CLI with a virtual
filesystem, core commands, and a forensic toolkit (`stat`, `strings`,
`hexdump`, `sha256sum`, `decode`). Case content is still placeholder
scaffolding; mystery authoring, evidence tracking, WASM, and the web UI come
in later milestones.

## Repository layout

```text
paper_trails/
├── Cargo.toml              # workspace root
├── crates/
│   └── game-core/          # shared engine (WASM-ready)
│       ├── Cargo.toml      # one dependency: sha2 (real SHA-256)
│       ├── src/
│       │   ├── lib.rs      # module map + re-exports
│       │   ├── vfs.rs      # virtual filesystem + path normalization
│       │   ├── command.rs  # tokenizer + typed command parsing
│       │   ├── engine.rs   # command execution + player state
│       │   ├── forensics.rs# strings/hexdump/SHA-256/decode (pure functions)
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
browsers, so the same crate backs both the native CLI and the future WASM
build.

## Build, test, and run

Requires a stable Rust toolchain (tested with cargo 1.96, edition 2024).

```bash
# Build everything
cargo build --workspace

# Run all unit and integration tests
cargo test --workspace

# Play the game
cargo run -p paper-trails-cli

# Verify the engine still compiles for the browser target
cargo check -p game-core --target wasm32-unknown-unknown
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
| `stat PATH` | show path, type, size, permissions, and modification time |
| `grep [-n] PATTERN PATH` | search **one** file; `-n` prints line numbers |
| `strings [-n LENGTH] PATH` | printable ASCII runs (default minimum: 4) |
| `hexdump PATH` | hex + ASCII dump, 16 bytes per row, `hexdump -C` style |
| `sha256sum PATH` | real SHA-256 digest of the file's bytes |
| `decode hex\|base64 VALUE` | decode hex or standard base64 text |

### Examples

```text
stat /opt/atlas/engine.bin
hexdump /opt/atlas/engine.bin
strings -n 3 /opt/atlas/engine.bin
sha256sum /opt/atlas/engine.bin
grep -n "Accepted password" /var/log/auth.log
decode hex 68656c6c6f                → hello
decode base64 aGVsbG8=               → hello
decode hex fffe                      → (bytes) ff fe
```

### Documented subset (intentional differences from Unix tools)

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
  dump (use `strings` or `hexdump` instead).
- **`stat` uses its own labeled format** (`Path`/`Type`/`Size`/`Permissions`/
  `Modified`) rather than GNU `stat -c` format strings; flags are rejected.
  Directories show `Size: -` and `Modified: -` because the virtual filesystem
  does not track directory size or timestamps (permissions are derived).
- **`strings` is ASCII-only:** sequences are bytes `0x20..=0x7E` (space
  through `~`), the classic Unix definition. Only `-n LENGTH` is supported
  (no `-a`, `-t`, encoding flags); output has no filename headers. Runs shorter
  than the minimum are dropped; the default minimum is 4.
- **`hexdump` has a fixed layout:** `hexdump -C`-style rows (8-hex-digit
  offset, two groups of 8 hex bytes, ASCII gutter with `.` for non-printables)
  and no options or format strings. Like `hexdump -C`, partial final rows are
  space-padded; unlike `hexdump -C`, the trailing end-offset line is omitted.
  An empty file produces no output.
- **`sha256sum` handles one file** and prints `<digest>  <path as typed>`
  (two spaces, like the Unix tool). No `--check` mode.
- **`decode TYPE VALUE` syntax:** `hex` and `base64` only (case-insensitive),
  exactly one value operand.
  - Hex: even number of digits, `0-9a-fA-F` only — no `0x` prefix, no
    whitespace or separators.
  - Base64: standard alphabet (`A-Za-z0-9+/`, no URL-safe `-`/`_`), length a
    multiple of 4, `=` padding required and allowed only at the end (max two),
    trailing bits must be zero (canonical encoding). Whitespace is **not**
    stripped.
  - Valid UTF-8 results print as text; anything else uses the documented form
    `(bytes) 7f 45 ...` (lowercase hex, space-separated).
- **Quotes group arguments** (`"` and `'`); escape sequences inside quotes are
  not processed. An unterminated quote is a syntax error.
- **Directories are derived from file paths.** Empty directories cannot be
  represented yet.
- **`ls` output is sorted** (files in a `BTreeMap`), keeping tests and the
  future browser build deterministic.

## Key design decisions

1. **One dependency, justified: `sha2`.** The `sha256sum` command must compute
   the actual digest, and hand-rolling SHA-256 invites silent bugs in
   player-facing output. `sha2` (RustCrypto) is pure Rust — no C/asm, no JS
   glue — and default features are disabled (`std`/`oid`/`alloc` not needed
   for hashing byte slices), keeping the graph to `sha2 → digest →
   hybrid-array → crypto-common → block-buffer → cfg-if`. WASM compatibility
   is *verified in this repo*, not assumed:
   `cargo check -p game-core --target wasm32-unknown-unknown` passes.
   Digests are pinned in tests against FIPS 180-4 vectors plus values
   cross-checked with system `sha256sum` and Python `hashlib`.
2. **Typed command parsing, not string matching.** `Command::parse` returns
   `Result<Command, ParseError>`; the engine only dispatches. Error messages
   are terminal-style (`cd: ../../..: path escapes the virtual root`).
3. **Traversal above `/` errors instead of clamping**, so probing is visible
   and testable.
4. **Bytes are the source of truth.** `VirtualFile.contents` is `Vec<u8>`;
   `strings`, `hexdump`, `sha256sum`, and `grep` all operate on raw bytes,
   while `cat` layers UTF-8 rendering on top.
5. **Pure forensics module.** All byte-level analysis (`forensics.rs`) is
   filesystem-free, I/O-free functions over `&[u8]`/`&str`, so the same code
   is trivially testable and portable to WASM.
6. **Deterministic everything.** Fixed epoch timestamps formatted by an
   in-crate UTC formatter, sorted directory listings, fixed-width hex dump
   rows — no chrono, no local-timezone surprises.
7. **`CommandOutput` is the state-change channel.** Evidence discoveries and
   objective updates attach to `execute()` in Milestone 3 without changing the
   adapter contract.
8. **Placeholder case content is labeled.** `crates/game-core/src/case.rs`
   marks every file as scaffolding; no story facts are committed yet. Final
   case data becomes data-driven and the answer key stays out of public assets.

## Testing

```bash
cargo test --workspace
```

Coverage includes path normalization (`/a/../b`, relative paths, `.`/root
behavior), traversal rejection, missing files vs. directories, binary files,
quoted arguments, malformed commands, `grep` line numbers, exact `stat`
output, `strings` boundary conditions (empty files, all-binary inputs, run
lengths), `hexdump` formatting verified byte-for-byte against system
`hexdump -C` output, SHA-256 against FIPS and cross-tool vectors, hex/base64
decode success and every rejection path, exact session outputs, and
case-image structure. Case-solution tests (later milestones) stay separate
from public case assets.

## What remains (next milestones)

- **Milestone 3 — Case 001:** author the real artifacts and timeline, evidence
  registry, objectives, `evidence`/`timeline`/`submit`, solution tests kept
  separate from public assets.
- **Milestone 4 — WASM:** thin `wasm-bindgen` adapter, `wasm-pack` build,
  native/browser parity checks.
- **Milestone 5 — Web terminal UI:** keyboard-first terminal, history,
  scrolling, evidence journal, `localStorage` save/restore.
- **Milestone 6 — Deployment:** static hosting, MIME types, no answer key in
  public assets.
