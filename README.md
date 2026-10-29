# Paper Trails — Rust + WASM CLI Detective Game

A single-player detective mystery played from a terminal. The player
investigates a fictional incident by examining a simulated computer system:
reading logs, searching files, inspecting bytes, and reconstructing a
timeline.

> The computer is the crime scene. Computer science is how you solve the crime.

**Current status: Milestone 5 complete** — native CLI with a virtual
filesystem, core commands, a forensic toolkit (`stat`, `strings`, `hexdump`,
`sha256sum`, `decode`), the authored Case 001 investigation (evidence
registry, objectives, chronological timeline, transparent report evaluator),
a `wasm-bindgen` adapter that exposes the same engine to JavaScript, and a
playable browser investigation interface built on that adapter. Deployment
is the remaining milestone.

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
│   ├── cli/                # thin native adapter (stdin → engine → stdout)
│   │   └── src/main.rs
│   └── wasm/               # thin WASM adapter (JS → engine → string)
│       ├── src/lib.rs              # Session + exported `PaperTrails` bindings
│       ├── build-web.sh            # build the browser package (wasm + web glue)
│       ├── examples/transcript.rs  # regenerate the golden parity fixture
│       ├── tests/
│       │   ├── session.rs      # adapter behavior vs. direct engine use
│       │   ├── transcript.rs   # native side of the shared golden transcript
│       │   ├── separation.rs   # shipped sources free of answer-key material
│       │   └── fixtures/       # commands.txt + expected.txt (shared w/ smoke)
│       ├── smoke/smoke.js      # Node smoke test against the built WASM
│       └── web/                # browser investigation interface
│           ├── index.html          # layout: header, terminal, sidebar
│           ├── style.css           # dark theme, responsive at ≤900px
│           ├── app.js              # DOM glue over the PaperTrails API
│           ├── term.js             # pure logic: history, line styling
│           ├── test.js             # Node unit tests for term.js
│           ├── serve.js            # zero-dep static server (correct WASM MIME)
│           ├── package.json        # marks web/*.js as ES modules for Node
│           └── pkg/                # generated browser package (gitignored)
└── README.md
```

The engine lives in `crates/game-core` and knows nothing about terminals or
browsers, so the same crate backs both the native CLI and the WASM build.
Private case-authoring documents live in a gitignored `authoring/`
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

# Verify the WASM adapter crate still compiles for the browser target
cargo check -p paper-trails-wasm --target wasm32-unknown-unknown

# Build + serve the browser interface (see "Browser interface")
apps/wasm/build-web.sh && node apps/wasm/web/serve.js
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

## WebAssembly adapter

`apps/wasm` (package `paper-trails-wasm`) compiles the game engine to
WebAssembly and exposes a small, stable JavaScript API. It contains **no
game logic**: every command runs through the same `GameEngine::execute` and
Case 001 definitions the native CLI uses, and no terminal, DOM, or
JavaScript behavior lives here or in `game-core`. Terminal UI, command
history, and local storage are Milestone 5 concerns and are intentionally
absent.

### JavaScript API

```js
import { PaperTrails } from "./pkg/paper_trails_wasm.js";

const game = new PaperTrails();   // fresh Case 001 session (cwd "/", no state)
game.execute("ls");               // string: complete output, exactly as the CLI prints it
game.discoveredEvidence();        // string[]: ["EV-002", "EV-001", ...] discovery order
game.completedObjectives();       // string[]: ["OBJ-001", ...]
game.execute("evidence");         // full evidence journal (a command)
game.execute("timeline");         // chronological timeline (a command)
game.cwd();                       // string: current virtual directory
game.caseId();                    // "CASE-001"
game.caseTitle();                 // "The Last Commit"
game.reset();                     // back to a clean session
```

Error behavior:

- `execute` **never throws for game-level problems** — unknown commands, bad
  paths, syntax errors, and empty input return the same text the native CLI
  prints (`""` when there is nothing to print).
- Passing a non-string where a string is expected throws a JS `TypeError`
  from the generated glue (standard wasm-bindgen behavior).
- The engine does not panic on player input; a hypothetical internal panic
  would surface as a WebAssembly `RuntimeError` rather than partial output.

### Tooling

```bash
# 1. The WASM target (already installed if the repo's wasm checks pass)
rustup target add wasm32-unknown-unknown

# 2. The glue generator — the version MUST match the wasm-bindgen crate
#    version pinned in Cargo.lock (checked by wasm-bindgen at load time):
grep -A1 'name = "wasm-bindgen"' Cargo.lock
cargo install wasm-bindgen-cli --version <that-version>   # repo verified: 0.2.129

# 3. A JavaScript runtime for the smoke test (any recent Node LTS works;
#    the repo verified v24.22.2): https://nodejs.org
```

### Build and smoke test

```bash
# Build the WASM module
cargo build -p paper-trails-wasm --target wasm32-unknown-unknown --release

# Generate the JS bindings into apps/wasm/pkg/ (gitignored)
wasm-bindgen --target nodejs --out-dir pkg \
  target/wasm32-unknown-unknown/release/paper_trails_wasm.wasm

# Run the smoke test (from apps/wasm/)
node smoke/smoke.js
```

The smoke test executes the 26-command sequence in
`tests/fixtures/commands.txt` against the compiled WASM module and diffs
every output against `tests/fixtures/expected.txt`, then checks the
structured getters (`caseId`, `discoveredEvidence`, …), journal/timeline
headers, and `reset()`. Exit code 0 = pass, 1 = mismatch, 2 = not built.

**Parity chain.** The same golden file ties both targets together:

```text
GameEngine (native) == PaperTrails (native)   ← tests/session.rs
PaperTrails (native) == expected.txt          ← tests/transcript.rs
PaperTrails (wasm)   == expected.txt          ← smoke/smoke.js (Node, executed)
```

To regenerate the golden after an intentional output change:

```bash
cargo run -q -p paper-trails-wasm --example transcript \
  > apps/wasm/tests/fixtures/expected.txt
```

### Known limitations

- Two glue targets coexist: `--target nodejs` (apps/wasm/pkg, used by the
  Node smoke test) and `--target web` (apps/wasm/web/pkg, used by the
  browser interface — see "Browser interface" below). `wasm-pack` and
  bundler-oriented builds are not used; no bundler is configured.
- `console_error_panic_hook` is not installed, so a hypothetical panic
  would produce an opaque `RuntimeError` message (the engine does not panic
  on player input).
- Structured getters expose IDs only; full journal/timeline text comes from
  `execute("evidence")` / `execute("timeline")`.
- Report draft fields are not separately exposed; their state is visible
  through the `submit` command output.
- Answer-key material never enters the package: `apps/wasm` depends only on
  `game-core`, `tests/separation.rs` scans shipped sources for the private
  design doc's prose and bans file-inclusion macros, and `apps/wasm/pkg/`
  is gitignored build output.

## Browser interface (Milestone 5)

`apps/wasm/web/` is a static, dependency-free page that turns the WASM
adapter into a playable, desktop-first terminal investigation UI.

### Integration approach (and why)

- **Chosen: `wasm-bindgen --target web` + native ES modules — no bundler,
  no npm install, no framework.** The page imports the generated glue
  (`import init, { PaperTrails } from "./pkg/paper_trails_wasm.js"`),
  `init()` fetches the `.wasm` sitting next to the module, and `app.js`
  forwards every input line to `game.execute()` and renders the returned
  text verbatim.
- **Rejected:** the `--target nodejs` glue (CommonJS — cannot run in a
  browser; it stays in use for the Node smoke test), `wasm-pack` /
  `--target bundler` (would add npm plus a bundler config for no benefit
  here), and any JavaScript re-implementation of game behavior (banned by
  design: parsing, discovery, timeline ordering, and grading stay in Rust).
- The page's own JavaScript splits in two: `term.js` is pure presentation
  logic (history recall, coloring engine lines, counting what the engine
  printed — unit-testable in Node), and `app.js` is DOM glue over the
  `PaperTrails` API. Neither contains game rules.

### Build and run

```bash
# One-time tooling (the same tools the WASM adapter already needs):
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <version in Cargo.lock>   # repo: 0.2.129

# Build the browser package: WASM module + web glue → apps/wasm/web/pkg/
apps/wasm/build-web.sh

# Serve locally (zero-dependency Node server, correct application/wasm MIME)
node apps/wasm/web/serve.js        # → http://localhost:8080/   (Ctrl-C stops)

# UI-logic unit tests (plain Node, no DOM)
node apps/wasm/web/test.js
```

`build-web.sh` enforces that the `wasm-bindgen` CLI version matches
`Cargo.lock`. Any static server works for serving as long as `.wasm` is
sent as `application/wasm` and the page is loaded over HTTP — ES modules do
not load from `file://`; `serve.js` exists so no extra tooling is required.

### Artifact locations

| Artifact | Produced by | In the browser package? |
| --- | --- | --- |
| `target/wasm32-unknown-unknown/release/paper_trails_wasm.wasm` | `cargo build --release --target wasm32-unknown-unknown` | copied to `web/pkg/paper_trails_wasm_bg.wasm` |
| `apps/wasm/web/pkg/` (ESM glue + `.wasm` + `.d.ts`) | `wasm-bindgen --target web` (via `build-web.sh`) | yes |
| `apps/wasm/pkg/` (Node glue) | `wasm-bindgen --target nodejs` (Milestone 4) | no — Node smoke test only |
| `apps/wasm/web/{index.html,app.js,term.js,style.css}` | hand-written | yes |

`apps/wasm/web/pkg/` and `apps/wasm/pkg/` are gitignored build output.

### Interface features

- Paper Trails title, spoiler-free Case 001 introduction, and a live
  `investigator:<cwd>$` prompt mirroring the CLI.
- Enter executes a command; ↑/↓ recall command history while preserving
  the in-progress draft; ordinary text editing is untouched.
- Auto-scrolling scrollback that sticks to the bottom, pauses when you
  scroll up (with a "jump to latest" button), and caps at 300 entries.
- **Evidence** and **Timeline** buttons show the engine's own `evidence`
  and `timeline` output in the inspector; the status line shows
  `Evidence N/M · Objectives N/M` parsed from the engine's journal header —
  no totals are hardcoded in JavaScript.
- A visible verdict banner appears when `submit` returns the engine's
  verdict (green for `ACCEPTED`, amber for `INCOMPLETE`); the banner shows
  the engine's own words, and no grading logic exists in JavaScript.
- **Help** runs the engine's spoiler-free `help` in the terminal.
- **Reset** asks for confirmation, then builds a genuinely fresh engine
  session and clears the terminal, inspector, verdict, and history.
- Failures are visible: WASM initialization errors and engine traps render
  in banners; command errors display the engine's exact error text
  (colored for scanning).
- Accessibility: labelled controls, `role="log"` scrollback that can be
  focused and scrolled by keyboard, visible focus rings, AA-or-better
  contrast, polite live regions for status and verdict, and
  `prefers-reduced-motion` support.
- Responsive: desktop-first two-column layout that stacks into a single
  column below 900px.

### Persistence decision

The adapter does **not** expose enough information to restore a session
(report fields are not readable and there is no state-injection API), so
game-state save/resume is deliberately out of scope: every page load starts
a genuinely fresh engine session. Only non-authoritative UI state persists
in `localStorage` — the command history used for ↑/↓ recall, capped at 200
entries — and Reset clears it too.

### Browser-interface limitations

- No game-state persistence (by the decision above): reloading the page
  starts over; the persisted history is input recall only.
- Live-browser verification of the interface was performed manually by the
  project owner; automated in-repo coverage consists of the Node unit tests
  (`web/test.js`), the Node WASM smoke test, and the Rust suite — the DOM
  glue (`app.js`) itself is not unit-tested.
- `serve.js` is a development server, not a deployment story (Milestone 6).

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
- **`apps/wasm/tests/`** — adapter behavior: fresh-session contract, empty
  input, byte-identical output vs. a directly-driven engine for the shared
  command sequence, discovery notices + dedup, journal/timeline/state
  agreement, neutral report → `INCOMPLETE`, reset equivalence, host
  filesystem isolation (probe paths that exist on the host but not in the
  case image), and plain-text error handling; the **golden-transcript**
  parity test (`transcript.rs`) pins the native adapter to the fixture the
  JS smoke test replays; and `separation.rs` scans the engine, adapter, and
  browser-page sources for answer-key prose (banning file-inclusion macros
  in Rust and any `authoring/` reference in the page code) while keeping
  the smoke fixtures mechanics-only.

JavaScript-side tests run separately with plain Node (no dependencies, no
DOM):

```bash
node apps/wasm/web/test.js     # browser-interface pure logic (22 tests)
node apps/wasm/smoke/smoke.js  # WASM build vs. native golden transcript
                               # (after `wasm-bindgen --target nodejs`, see above)
```

`web/test.js` covers command-history recall (draft preservation, limits,
dedupe), engine-output line classification, verdict extraction, and
evidence-status parsing. The DOM glue and layout are covered by manual
live-browser verification (see "Browser interface limitations").

## What remains (next milestones)

- **Milestone 6 — Deployment:** static hosting, correct MIME types for the
  WASM module, direct URL access and refresh, clean-start/reset checks, and
  confirmation that no answer key is present in public assets.
