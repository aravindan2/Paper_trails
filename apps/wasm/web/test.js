// Unit tests for the browser interface's pure logic (web/term.js).
//
// Run:  node web/test.js        (from apps/wasm/ — or: node apps/wasm/web/test.js)
// No dependencies: plain Node, no DOM. DOM glue and the WASM integration
// are covered by the real-browser validation described in the README.

import assert from "node:assert/strict";
import {
  createHistoryState,
  pushCommand,
  moveUp,
  moveDown,
  clearHistory,
  classifyLine,
  extractVerdict,
  parseEvidenceStatus,
  HISTORY_LIMIT,
} from "./term.js";

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    passed += 1;
    console.log(`ok   ${name}`);
  } catch (err) {
    failed += 1;
    console.error(`FAIL ${name}`);
    console.error(`     ${err.message}`);
  }
}

// --- history ---------------------------------------------------------------

test("fresh history starts empty at the draft position", () => {
  const state = createHistoryState();
  assert.deepEqual(state, { entries: [], index: 0, draft: "" });
});

test("pushCommand records non-empty commands in order", () => {
  let state = createHistoryState();
  state = pushCommand(state, "pwd");
  state = pushCommand(state, "ls");
  assert.deepEqual(state.entries, ["pwd", "ls"]);
  assert.equal(state.index, 2);
});

test("pushCommand ignores empty and whitespace-only input", () => {
  let state = createHistoryState();
  state = pushCommand(state, "");
  state = pushCommand(state, "   ");
  assert.deepEqual(state.entries, []);
});

test("pushCommand does not duplicate consecutive identical commands", () => {
  let state = createHistoryState();
  state = pushCommand(state, "evidence");
  state = pushCommand(state, "evidence");
  state = pushCommand(state, "ls");
  state = pushCommand(state, "evidence");
  assert.deepEqual(state.entries, ["evidence", "ls", "evidence"]);
});

test("history is capped at HISTORY_LIMIT entries (oldest dropped)", () => {
  let state = createHistoryState();
  for (let i = 0; i < HISTORY_LIMIT + 25; i += 1) {
    state = pushCommand(state, `cmd${i}`);
  }
  assert.equal(state.entries.length, HISTORY_LIMIT);
  assert.equal(state.entries[0], `cmd${25}`);
  assert.equal(state.entries.at(-1), `cmd${HISTORY_LIMIT + 24}`);
});

test("ArrowUp walks back through commands and stops at the oldest", () => {
  let state = createHistoryState();
  state = pushCommand(state, "pwd");
  state = pushCommand(state, "ls");
  state = pushCommand(state, "evidence");

  let result = moveUp(state, "");
  assert.equal(result.handled, true);
  assert.equal(result.value, "evidence");
  state = result.state;

  result = moveUp(state, "");
  assert.equal(result.value, "ls");
  state = result.state;

  result = moveUp(state, "");
  assert.equal(result.value, "pwd");
  state = result.state;

  result = moveUp(state, "");
  assert.equal(result.handled, false, "no older command exists");
  assert.equal(result.value, "");
});

test("ArrowUp on empty history is a no-op", () => {
  const state = createHistoryState();
  const result = moveUp(state, "typed draft");
  assert.equal(result.handled, false);
  assert.equal(result.value, "typed draft");
  assert.deepEqual(result.state, state);
});

test("ArrowDown past the newest entry restores the saved draft", () => {
  let state = createHistoryState();
  state = pushCommand(state, "pwd");
  state = pushCommand(state, "ls");

  let result = moveUp(state, "half-typed"); // saves draft, shows "ls"
  state = result.state;
  assert.equal(result.value, "ls");

  result = moveUp(state, "half-typed"); // shows "pwd"
  state = result.state;
  assert.equal(result.value, "pwd");

  result = moveDown(state, "pwd"); // back to "ls"
  state = result.state;
  assert.equal(result.value, "ls");

  result = moveDown(state, "ls"); // back to the draft
  assert.equal(result.value, "half-typed");
  state = result.state;

  result = moveDown(state, "half-typed"); // already at draft
  assert.equal(result.handled, false);
  assert.equal(result.value, "half-typed");
});

test("ArrowDown at the draft position is a no-op", () => {
  const state = createHistoryState(["a", "b"]);
  const result = moveDown(state, "draft");
  assert.equal(result.handled, false);
  assert.equal(result.value, "draft");
});

test("editing while browsing does not corrupt the saved draft", () => {
  let state = createHistoryState();
  state = pushCommand(state, "first");
  state = pushCommand(state, "second");

  let result = moveUp(state, "draft-text"); // → "second"
  state = result.state;
  result = moveUp(state, "edited while browsing"); // → "first" (draft keeps original)
  state = result.state;
  assert.equal(result.value, "first");

  result = moveDown(state, "first"); // → "second"
  state = result.state;
  result = moveDown(state, "second"); // → original draft
  assert.equal(result.value, "draft-text");
});

test("clearHistory returns a fresh empty state", () => {
  let state = createHistoryState(["a", "b"]);
  state = clearHistory();
  assert.deepEqual(state, { entries: [], index: 0, draft: "" });
});

test("createHistoryState filters junk and caps restored entries", () => {
  const junk = [1, null, "", "ok", undefined, "also-ok"];
  const state = createHistoryState(junk);
  assert.deepEqual(state.entries, ["ok", "also-ok"]);
  assert.equal(state.index, 2);
});

// --- output classification (styling only) ----------------------------------

test("classifyLine recognizes engine discovery notices", () => {
  assert.equal(classifyLine("evidence recorded: EV-001 — Badge exit after hours"), "notice");
  assert.equal(classifyLine("objective complete: OBJ-001 — Title"), "notice");
});

test("classifyLine recognizes the engine verdict line", () => {
  assert.equal(classifyLine("  verdict: ACCEPTED — case reconstruction complete"), "verdict");
  assert.equal(classifyLine("  verdict: INCOMPLETE — revise your report"), "verdict");
});

test("classifyLine recognizes engine command errors", () => {
  assert.equal(classifyLine("cat: /nope: No such file or directory"), "error");
  assert.equal(classifyLine("unknown command: unknowncmd"), "error");
  assert.equal(classifyLine("cd: ../..: path escapes the virtual root"), "error");
  assert.equal(classifyLine("grep: /missing: no such file"), "error");
});

test("classifyLine leaves ordinary output alone (colon-rich lines included)", () => {
  assert.equal(classifyLine("auth.log:7:Oct 27 23:47:12 sshd: Accepted password"), "normal");
  assert.equal(classifyLine("recorded who: someone"), "normal");
  assert.equal(classifyLine("  evidence: 7 of 8 recorded (6 required)"), "normal");
  assert.equal(classifyLine("CASE-001 evidence: 3 of 8"), "normal");
  assert.equal(classifyLine(""), "normal");
});

// --- verdict extraction (reads the engine's line; decides nothing) ----------

test("extractVerdict finds and parses an INCOMPLETE verdict", () => {
  const output = [
    "CASE-001 report evaluation",
    "",
    "  evidence: 7 of 8 recorded (6 required — key observations missing)",
    "  who: does not match the evidence you have recorded",
    "",
    "  verdict: INCOMPLETE — revise your report and gather more evidence",
  ].join("\n");
  const verdict = extractVerdict(output);
  assert.equal(verdict.accepted, false);
  assert.equal(verdict.text, "INCOMPLETE — revise your report and gather more evidence");
});

test("extractVerdict recognizes the ACCEPTED verdict", () => {
  const output = "  verdict: ACCEPTED — case reconstruction complete";
  const verdict = extractVerdict(output);
  assert.equal(verdict.accepted, true);
  assert.equal(verdict.text, "ACCEPTED — case reconstruction complete");
});

test("extractVerdict returns null when no verdict was evaluated", () => {
  assert.equal(extractVerdict("CASE-001 evidence: 0 of 8"), null);
  assert.equal(extractVerdict(""), null);
});

// --- evidence status parsing ------------------------------------------------

test("parseEvidenceStatus reads the engine header and objective marks", () => {
  const output = [
    "CASE-001 evidence: 7 of 8",
    "",
    "EV-002  Some title",
    "        observation text",
    "        source: /var/log/auth.log",
    "",
    "Objectives:",
    "  [x] First objective",
    "  [x] Second objective",
    "  [ ] Third objective",
    "  [ ] Fourth objective",
  ].join("\n");
  assert.deepEqual(parseEvidenceStatus(output), {
    recorded: 7,
    total: 8,
    objectivesDone: 2,
    objectivesTotal: 4,
  });
});

test("parseEvidenceStatus handles the empty journal state", () => {
  const output = [
    "CASE-001 evidence: 0 of 8",
    "",
    "No evidence recorded yet. Examine the case artifacts to record observations.",
    "",
    "Objectives:",
    "  [ ] First objective",
    "  [ ] Second objective",
    "  [ ] Third objective",
    "  [ ] Fourth objective",
  ].join("\n");
  assert.deepEqual(parseEvidenceStatus(output), {
    recorded: 0,
    total: 8,
    objectivesDone: 0,
    objectivesTotal: 4,
  });
});

test("parseEvidenceStatus rejects non-journal output", () => {
  assert.equal(parseEvidenceStatus("timeline: no case is loaded"), null);
  assert.equal(parseEvidenceStatus(""), null);
});

// --- summary ----------------------------------------------------------------

console.log(`\n${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
