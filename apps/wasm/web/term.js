// Pure presentation logic for the Paper Trails browser interface.
//
// Everything here is deliberately DOM-free and engine-free so it can be
// unit-tested in plain Node (`node web/test.js`). The module never decides
// game outcomes: verdicts, evidence, and timelines are computed by the Rust
// engine; these functions only *classify and count text the engine returned*
// for display.

/// Maximum number of recalled commands kept in history (also the cap for
/// what is persisted to localStorage).
export const HISTORY_LIMIT = 200;

// ---------------------------------------------------------------------------
// Command history (Up/Down recall with draft preservation).
// ---------------------------------------------------------------------------

/**
 * Create a history state. `entries` are previously executed commands
 * (oldest first); the cursor starts at the end ("draft" position).
 */
export function createHistoryState(entries = []) {
  const list = entries.filter((entry) => typeof entry === "string" && entry !== "");
  return { entries: list.slice(-HISTORY_LIMIT), index: list.length, draft: "" };
}

/**
 * Record an executed command. Empty input is ignored; a command identical
 * to the most recent one is not duplicated. Returns a new state.
 */
export function pushCommand(state, command) {
  if (typeof command !== "string" || command.trim() === "") return state;
  const entries = state.entries.slice();
  if (entries[entries.length - 1] !== command) entries.push(command);
  const capped = entries.slice(-HISTORY_LIMIT);
  return { entries: capped, index: capped.length, draft: "" };
}

/**
 * Recall an older command (ArrowUp). `currentInput` is the text in the
 * input right now; when leaving the draft position it is saved so ArrowDown
 * can restore it. Returns `{ state, value, handled }`; when `handled` is
 * false the caller should keep the input unchanged.
 */
export function moveUp(state, currentInput) {
  if (state.index === 0) return { state, value: currentInput, handled: false };
  const next = { ...state };
  if (state.index === state.entries.length) next.draft = currentInput;
  next.index = state.index - 1;
  return { state: next, value: next.entries[next.index], handled: true };
}

/**
 * Move toward newer commands (ArrowDown). Moving past the newest entry
 * restores the saved draft. Returns `{ state, value, handled }`.
 */
export function moveDown(state, currentInput) {
  if (state.index >= state.entries.length) {
    return { state, value: currentInput, handled: false };
  }
  const next = { ...state };
  next.index += 1;
  const value = next.index === next.entries.length ? next.draft : next.entries[next.index];
  return { state: next, value, handled: true };
}

/** Empty history (used by Reset). */
export function clearHistory() {
  return createHistoryState();
}

// ---------------------------------------------------------------------------
// Output line classification (coloring only — never a decision).
// ---------------------------------------------------------------------------

const NOTICE_PREFIXES = ["evidence recorded:", "objective complete:"];
const ERROR_PREFIXES = [
  "cat:",
  "grep:",
  "ls:",
  "cd:",
  "stat:",
  "strings:",
  "hexdump:",
  "sha256sum:",
  "decode:",
  "submit:",
  "evidence:",
  "timeline:",
  "unknown command:",
];

/**
 * Classify one line of engine output for styling:
 *   "notice" — engine discovery/objective notice
 *   "verdict" — the engine's final-report verdict line
 *   "error" — the engine's command error text
 *   "normal" — anything else (displayed verbatim, unstyled)
 */
export function classifyLine(line) {
  if (NOTICE_PREFIXES.some((prefix) => line.startsWith(prefix))) return "notice";
  if (line.startsWith("  verdict: ")) return "verdict";
  if (ERROR_PREFIXES.some((prefix) => line.startsWith(prefix))) return "error";
  return "normal";
}

/**
 * Extract the engine's verdict line from a `submit` evaluation, if present.
 * The banner shows the engine's own words; `accepted` only picks the color.
 */
export function extractVerdict(output) {
  const line = output.split("\n").find((candidate) => candidate.startsWith("  verdict: "));
  if (!line) return null;
  const text = line.slice("  verdict:".length).trim();
  return { accepted: text.startsWith("ACCEPTED"), text };
}

/**
 * Parse the engine's `evidence` command output for the status line:
 * header `CASE-001 evidence: N of M` plus the `[x]`/`[ ]` objective marks.
 * Returns null when the output is not an evidence listing (e.g. no case).
 */
export function parseEvidenceStatus(output) {
  const header = /^\S+ evidence: (\d+) of (\d+)/.exec(output);
  if (!header) return null;
  const objectivesTotal = (output.match(/^  \[[ x]\] /gm) || []).length;
  const objectivesDone = (output.match(/^  \[[xX]\] /gm) || []).length;
  return {
    recorded: Number(header[1]),
    total: Number(header[2]),
    objectivesDone,
    objectivesTotal,
  };
}
