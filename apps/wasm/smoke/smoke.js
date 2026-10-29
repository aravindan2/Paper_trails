#!/usr/bin/env node
// JavaScript smoke test for the Paper Trails WASM adapter.
//
// Runs the shared fixture command sequence against the compiled WASM module
// and diffs the output against tests/fixtures/expected.txt — the same golden
// file the native test (tests/transcript.rs) pins. Matching on both sides
// means the native and WASM builds behave identically.
//
// Build + run (see README, "WebAssembly adapter"):
//   cargo build -p paper-trails-wasm --target wasm32-unknown-unknown --release
//   wasm-bindgen --target nodejs --out-dir pkg \
//     target/wasm32-unknown-unknown/release/paper_trails_wasm.wasm
//   node smoke/smoke.js
//
// Exit codes: 0 = all checks passed, 1 = mismatches, 2 = not built yet.
"use strict";

const fs = require("fs");
const path = require("path");

const pkgGlue = path.join(__dirname, "..", "pkg", "paper_trails_wasm.js");
if (!fs.existsSync(pkgGlue)) {
  console.error(`missing ${pkgGlue}`);
  console.error("build the bindings first:");
  console.error("  cargo build -p paper-trails-wasm --target wasm32-unknown-unknown --release");
  console.error("  wasm-bindgen --target nodejs --out-dir pkg target/wasm32-unknown-unknown/release/paper_trails_wasm.wasm");
  process.exit(2);
}
const { PaperTrails } = require(pkgGlue);

const fixturesDir = path.join(__dirname, "..", "tests", "fixtures");

// --- fixtures ---------------------------------------------------------------

function loadCommands() {
  const text = fs.readFileSync(path.join(fixturesDir, "commands.txt"), "utf8");
  return text
    .split("\n")
    .filter((line) => line !== "" && !line.startsWith("#"));
}

// Format: ">>> <command>" / output lines / "=== end ===" (see examples/transcript.rs).
function parseTranscript(text) {
  const lines = text.split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  const pairs = [];
  let i = 0;
  while (i < lines.length) {
    const header = lines[i++];
    if (!header.startsWith(">>> ")) {
      throw new Error(`expected a '>>> ' command line, found: ${JSON.stringify(header)}`);
    }
    const command = header.slice(4);
    const output = [];
    while (i < lines.length && lines[i] !== "=== end ===") output.push(lines[i++]);
    if (i >= lines.length) throw new Error(`transcript ended without '=== end ===' for ${JSON.stringify(command)}`);
    i += 1; // consume the marker
    pairs.push({ command, output: output.join("\n") });
  }
  return pairs;
}

// --- checks -----------------------------------------------------------------

let failures = 0;
function fail(label, detail) {
  failures += 1;
  console.error(`FAIL  ${label}`);
  if (detail !== undefined) console.error(`      ${detail}`);
}
function equal(label, actual, expected) {
  if (actual !== expected) {
    fail(label, `expected ${JSON.stringify(expected)}\n      actual   ${JSON.stringify(actual)}`);
  }
}
function arrayEqual(label, actual, expected) {
  const a = JSON.stringify(actual);
  const e = JSON.stringify(expected);
  if (a !== e) fail(label, `expected ${e}\n      actual   ${a}`);
}
function ok(label, condition, detail) {
  if (!condition) fail(label, detail);
}

// --- 1. command sequence vs golden transcript -------------------------------

const commands = loadCommands();
const golden = parseTranscript(fs.readFileSync(path.join(fixturesDir, "expected.txt"), "utf8"));

if (golden.length !== commands.length) {
  fail("fixture sizes", `expected.txt has ${golden.length} commands, commands.txt has ${commands.length}`);
}

const game = new PaperTrails();
golden.forEach(({ command, output: expected }, i) => {
  if (commands[i] !== command) {
    fail("fixture order", `line ${i}: expected.txt=${JSON.stringify(command)} commands.txt=${JSON.stringify(commands[i])}`);
    return;
  }
  const actual = game.execute(command);
  if (actual !== expected) {
    fail(`output for: ${command}`, `expected ${JSON.stringify(expected)}\n      actual   ${JSON.stringify(actual)}`);
  }
});
console.log(`ran ${golden.length} commands against the WASM build`);

// --- 2. exported API --------------------------------------------------------

equal("caseId()", game.caseId(), "CASE-001");
equal("caseTitle()", game.caseTitle(), "The Last Commit");
equal("cwd() after sequence", game.cwd(), "/");
arrayEqual(
  "discoveredEvidence()",
  game.discoveredEvidence(),
  ["EV-002", "EV-001", "EV-004", "EV-003", "EV-006", "EV-005", "EV-007"]
);
arrayEqual("completedObjectives()", game.completedObjectives(), ["OBJ-001", "OBJ-002", "OBJ-003"]);

// Journal and timeline are commands; verify their headers on the WASM build.
ok(
  "execute('evidence') header",
  game.execute("evidence").startsWith("CASE-001 evidence: 7 of 8")
);
ok(
  "execute('timeline') header",
  game.execute("timeline").startsWith("CASE-001 timeline: 6 dated events")
);

// --- 3. reset returns a clean session ---------------------------------------

game.reset();
equal("cwd() after reset", game.cwd(), "/");
arrayEqual("discoveredEvidence() after reset", game.discoveredEvidence(), []);
arrayEqual("completedObjectives() after reset", game.completedObjectives(), []);
equal("execute('pwd') after reset", game.execute("pwd"), "/");
ok(
  "execute('evidence') after reset",
  game.execute("evidence").startsWith("CASE-001 evidence: 0 of 8")
);

// A fresh handle behaves the same as reset (clean-state equivalence).
const fresh = new PaperTrails();
equal("fresh pwd", fresh.execute("pwd"), "/");
arrayEqual("fresh state", fresh.discoveredEvidence(), []);

// --- summary ----------------------------------------------------------------

if (failures > 0) {
  console.error(`\n${failures} check(s) failed`);
  process.exit(1);
}
console.log("smoke test passed: WASM output matches the native golden transcript");
