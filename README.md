# Paper Trails — Rust + WASM CLI Detective Game

A single-player detective mystery played from a terminal. The player
investigates a fictional incident by examining a simulated computer system:
reading logs, searching files, inspecting bytes, and reconstructing a
timeline.

> The computer is the crime scene. Computer science is how you solve the crime.

**Current status: Milestone 3 complete** — native CLI with a virtual
filesystem, core commands, a forensic toolkit (`stat`, `strings`, `hexdump`,
`sha256sum`, `decode`), and the authored Case 001 investigation: a complete
evidence registry, objectives, chronological timeline, and a transparent
report evaluator (`evidence` / `timeline` / `submit`). WASM bindings, the web
UI, and deployment come in later milestones.

## Repository layout

```text
paper_trails/
├── Cargo.toml              # workspace root
├── crates/
│   └── game-core/          # shared engine (WASM-ready)
│       ├── Cargo.toml      # one dependency: sha2 (real SHA-256)
│       ├── src/
│       │   ├── lib.rs          # module map + re-exports
│       │   ├── vfs.rs          # virtual filesystem + path normalization
│       │   ├── command.rs      # tokenizer + typed command parsing
│       │   ├── engine.rs       # command execution + player state
│       │   ├── forensics.rs    # strings/hexdump/SHA-256/decode (pure functions)
│       │   ├── investigation.rs# evidence/discovery/objective/report mechanics
│       │   └── case.rs         # Case 001 artifacts + evidence definitions
│       └── tests/
│           ├── session.rs      # end-to-end player session tests
│           ├── investigation.rs# discovery/journal/report mechanics
│           └── answer_key.rs   # solution + consistency + separation tests
├── apps/
│   └── cli/                # thin native adapter (stdin → engine → stdout)
│       └── src/main.rs
└── README.md
```

The engine lives in `crates/game-core` and knows nothing about terminals or
browsers, so the same crate backs both the native CLI and the future WASM
build. Private case-authoring documents live in a gitignored `authoring/`
directory on the development machine only; solution-dependent *test* code
(`tests/answer_key.rs`) may know the answer because tests are never part of a
browser bundle.

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
| `evidence` | list recorded observations and case objectives |
| `timeline` | show dated observations in chronological order, with sources |
| `submit who\|did\|why TEXT` | record one line of your final report |
| `submit` | evaluate the recorded report |

### Examples

```text
stat /opt/atlas/engine.bin
hexdump /opt/atlas/engine.bin
strings -n 30 /opt/atlas/engine.bin
sha256sum /opt/atlas/engine.bin
grep -n "Accepted password" /var/log/auth.log
decode hex 68656c6c6f                → hello
decode base64 aGVsbG8=               → hello
decode hex fffe                      → (bytes) ff fe
evidence
timeline
submit did <what you think was changed>
submit
```

## Investigation

Case 001 ships a complete authored mystery: roughly fifteen artifacts across
the virtual filesystem — system logs, a source repository with git history,
release documentation, an installed binary, and personal files — plus a
registry of eight observations, four objectives, and a report evaluator.

### How evidence is recorded

Evidence is recorded when a command actually **surfaces the relevant content
to you**: displaying the matching line with `cat` or `grep`, showing the
artifact's metadata with `stat`, extracting the run with `strings` or
`hexdump`, or decoding the registered value. Running a command against the
wrong file — or one that filters the content out (e.g. an over-high
`strings -n`) — records nothing. Each observation is recorded at most once;
repeating an inspection is silent. Successful discoveries append notices like
`evidence recorded: EV-00X — Title` and `objective complete: OBJ-00X — Title`
to the command's output, and `evidence` shows the journal with sources and
cross-references.

### How reports are evaluated

`submit who|did|why TEXT` records (or revises) a report field; bare `submit`
evaluates. The evaluator prints:

- an evidence line — `N of M recorded (K required — key observations
  covered|missing)`,
- one status per field — `not recorded — use: submit X TEXT`,
  `does not match the evidence you have recorded`, or `ok`,
- a verdict — `ACCEPTED` or `INCOMPLETE`.

Evaluation runs entirely in the client (report text never leaves the process;
evaluation must work offline in the browser). It is deliberately transparent
and generous: any reasonable wording of the supported explanation passes —
the evidence gate does the heavy lifting, and incomplete feedback says what
is missing without ever revealing the answer.

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
   objective completions append notice lines to the same output the adapter
   already prints — the CLI (and later the browser) needs no new plumbing.
8. **Discovery is content-based, not command-based.** Each finished command
   reports what it surfaced (an `Inspection`: displayed text/lines, extracted
   runs, shown metadata, decoded value); triggers match against that content
   on the resolved path. Merely running the right *command* never records
   anything the player did not see.
9. **Case definitions are data.** `case.rs` registers artifacts, evidence,
   objectives, and grading rules as statics; `investigation.rs` is
   case-agnostic, and `GameEngine::with_case` can load future cases. The
   unit-test sandbox (`GameEngine::new`) loads no case at all, which keeps
   engine mechanics tests independent of story content.
10. **Grading is transparent and evidence-backed.** `submit` uses documented
    keyword checks (case-insensitive, generously worded) *plus* an evidence
    sufficiency gate, and always reports per-field status. Evaluation runs in
    the client by design (offline browser play); nothing relies on
    client-side secrecy.
11. **The answer key stays out of shipped material.** The ground-truth
    narrative lives in the gitignored `authoring/` directory; solution
    knowledge is confined to development-only tests (`tests/answer_key.rs`);
    public case assets carry the mystery's *facts* but never its solution
    prose. A test enforces all of this — including a spoiler-free README.

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
case-image structure — plus the investigation layer:

- **`tests/investigation.rs`** — discovery conditions (irrelevant actions
  record nothing; content-based triggers do), duplicate discovery is silent,
  journal/timeline output formats, objective completion, and report
  recording/evaluation for missing, unsupported, empty, and malformed input.
- **`tests/answer_key.rs`** — solution-dependent, development-only: a full
  clean-state progression to all eight observations and four objectives,
  accepted/incomplete/unsupported report verdicts, artifact ↔ timeline
  consistency (timestamps, reflog epochs, digests pinned against ground
  truth), and the answer-key separation checks (no solution text in case
  assets, authoring docs gitignored and outside the shipped trees,
  spoiler-free README).

## What remains (next milestones)

- **Milestone 4 — WASM:** thin `wasm-bindgen` adapter, `wasm-pack` build,
  native/browser parity checks.
- **Milestone 5 — Web terminal UI:** keyboard-first terminal, history,
  scrolling, evidence journal, `localStorage` save/restore.
- **Milestone 6 — Deployment:** static hosting, MIME types, no answer key in
  public assets.
