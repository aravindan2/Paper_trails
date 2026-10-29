// Paper Trails — browser investigation interface.
//
// Thin DOM glue over the `PaperTrails` WASM API. All game behavior (command
// parsing, virtual filesystem, evidence discovery, timeline ordering, report
// grading) lives in Rust; this file only:
//   - initializes the WASM module and surfaces failures visibly,
//   - forwards input lines to `game.execute()` and renders the exact text
//     the engine returned,
//   - renders engine commands (`evidence`, `timeline`, `help`) on demand,
//   - highlights engine output lines (color only — no decisions),
//   - manages UI-only state: scrollback, arrow-key history, localStorage
//     command history (a UI preference — never game state), and reset.

import init, { PaperTrails } from "./pkg/paper_trails_wasm.js";
import {
  createHistoryState,
  pushCommand,
  moveUp,
  moveDown,
  clearHistory,
  classifyLine,
  extractVerdict,
  parseEvidenceStatus,
} from "./term.js";

const HISTORY_STORAGE_KEY = "paper-trails.command-history";
const MAX_SCROLLBACK_ENTRIES = 300;
const SCROLL_STICK_PX = 24;

const el = {
  caseLabel: document.getElementById("case-label"),
  resetBtn: document.getElementById("reset-btn"),
  fatalBanner: document.getElementById("fatal-banner"),
  verdictBanner: document.getElementById("verdict-banner"),
  terminal: document.getElementById("terminal"),
  jumpLatest: document.getElementById("jump-latest"),
  form: document.getElementById("cmd-form"),
  promptCwd: document.getElementById("prompt-cwd"),
  input: document.getElementById("cmd-input"),
  statusLine: document.getElementById("status-line"),
  evidenceBtn: document.getElementById("evidence-btn"),
  timelineBtn: document.getElementById("timeline-btn"),
  helpBtn: document.getElementById("help-btn"),
  inspectorCaption: document.getElementById("inspector-caption"),
  inspector: document.getElementById("inspector"),
};

/** @type {InstanceType<typeof PaperTrails> | null} */
let game = null;
/** Which engine listing the inspector is mirroring, if any. */
let inspectorMode = null;
/** Stick to the bottom of the scrollback unless the user scrolled up. */
let stickToBottom = true;
let history = loadPersistedHistory();

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function appendLineClass(parent, text, className) {
  const span = document.createElement("span");
  span.className = className;
  span.textContent = text;
  parent.append(span);
}

/** Render engine output verbatim, coloring lines the engine produced. */
function renderOutput(container, output) {
  const lines = output.split("\n");
  lines.forEach((line, index) => {
    if (index > 0) container.append("\n");
    const kind = classifyLine(line);
    if (kind === "normal") {
      container.append(line);
    } else {
      appendLineClass(container, line, `line-${kind}`);
    }
  });
}

function trimScrollback() {
  while (el.terminal.childElementCount > MAX_SCROLLBACK_ENTRIES) {
    el.terminal.firstElementChild.remove();
  }
}

function scrollToBottomIfStuck() {
  if (stickToBottom) {
    el.terminal.scrollTop = el.terminal.scrollHeight;
    el.jumpLatest.hidden = true;
  } else {
    el.jumpLatest.hidden = false;
  }
}

/** One echoed command plus its (possibly empty) engine output. */
function appendEntry(command, output) {
  const entry = document.createElement("div");
  entry.className = "entry";

  const cmdline = document.createElement("div");
  cmdline.className = "cmdline";
  const glyph = document.createElement("span");
  glyph.className = "prompt-glyph";
  glyph.textContent = "› ";
  cmdline.append(glyph, command);
  entry.append(cmdline);

  if (output !== "") {
    const pre = document.createElement("pre");
    pre.className = "output";
    renderOutput(pre, output);
    entry.append(pre);
  }

  el.terminal.append(entry);
  trimScrollback();
  scrollToBottomIfStuck();
}

/** UI-only note (not engine output). */
function appendSystemLine(text) {
  const line = document.createElement("div");
  line.className = "system-line";
  line.textContent = text;
  el.terminal.append(line);
  trimScrollback();
  scrollToBottomIfStuck();
}

// ---------------------------------------------------------------------------
// Status, inspector, verdict
// ---------------------------------------------------------------------------

function refreshStatus() {
  if (!game) return;
  const status = parseEvidenceStatus(game.execute("evidence"));
  if (!status) {
    el.statusLine.textContent = "Status unavailable.";
    return;
  }
  el.statusLine.textContent =
    `Evidence ${status.recorded}/${status.total} · ` +
    `Objectives ${status.objectivesDone}/${status.objectivesTotal}`;
}

function refreshInspector() {
  if (!game || !inspectorMode) return;
  showInspector(inspectorMode, /* silent */ true);
}

function showInspector(mode, silent = false) {
  if (!game) return;
  // `evidence` and `timeline` are side-effect-free engine commands.
  const output = mode === "timeline" ? game.execute("timeline") : game.execute("evidence");
  inspectorMode = mode;
  el.inspector.replaceChildren();
  if (output === "") {
    el.inspector.textContent = "(no output)";
  } else {
    renderOutput(el.inspector, output);
  }
  el.inspectorCaption.textContent =
    mode === "timeline"
      ? "Chronological timeline, as returned by the engine."
      : "Evidence journal, as returned by the engine.";
  if (!silent) el.inspector.focus({ preventScroll: true });
}

function showVerdict(verdict) {
  el.verdictBanner.textContent = `Report verdict: ${verdict.text}`;
  el.verdictBanner.classList.toggle("accepted", verdict.accepted);
  el.verdictBanner.classList.toggle("incomplete", !verdict.accepted);
  el.verdictBanner.hidden = false;
}

function hideVerdict() {
  el.verdictBanner.hidden = true;
  el.verdictBanner.textContent = "";
  el.verdictBanner.classList.remove("accepted", "incomplete");
}

function updatePrompt() {
  if (game) el.promptCwd.textContent = `investigator:${game.cwd()}$`;
}

// ---------------------------------------------------------------------------
// Command execution
// ---------------------------------------------------------------------------

function runCommand(raw) {
  if (!game) return;
  const command = raw;
  if (command.trim() === "") return;

  let output;
  try {
    output = game.execute(command);
  } catch (err) {
    // A WASM trap: show it, keep the session, point at Reset.
    appendEntry(command, `internal engine error: ${err}`);
    appendSystemLine("The session may be inconsistent — use Reset investigation to start over.");
    return;
  }

  appendEntry(command, output);

  history = pushCommand(history, command);
  persistHistory(history);

  const verdict = extractVerdict(output);
  if (verdict) showVerdict(verdict);

  updatePrompt();
  refreshStatus();
  refreshInspector();
}

// ---------------------------------------------------------------------------
// History persistence (UI preference only — never game state)
// ---------------------------------------------------------------------------

function loadPersistedHistory() {
  try {
    const raw = window.localStorage.getItem(HISTORY_STORAGE_KEY);
    if (!raw) return createHistoryState();
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) return createHistoryState(parsed);
  } catch {
    // Storage unavailable (private mode, disabled cookies): in-memory only.
  }
  return createHistoryState();
}

function persistHistory(state) {
  try {
    window.localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(state.entries));
  } catch {
    // Non-fatal: history simply won't survive a reload.
  }
}

function forgetPersistedHistory() {
  try {
    window.localStorage.removeItem(HISTORY_STORAGE_KEY);
  } catch {
    // Ignore.
  }
}

// ---------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------

function setUiEnabled(enabled) {
  el.input.disabled = !enabled;
  el.resetBtn.disabled = !enabled;
  el.evidenceBtn.disabled = !enabled;
  el.timelineBtn.disabled = !enabled;
  el.helpBtn.disabled = !enabled;
}

function showFatal(message) {
  el.fatalBanner.textContent = message;
  el.fatalBanner.hidden = false;
  el.statusLine.textContent = "Engine failed to load.";
  setUiEnabled(false);
}

function resetInvestigation() {
  if (!game) return;
  const confirmed = window.confirm(
    "Start a fresh investigation?\n\nTerminal output and command history will be cleared.",
  );
  if (!confirmed) return;

  game.reset();
  history = clearHistory();
  forgetPersistedHistory();
  inspectorMode = null;

  el.terminal.replaceChildren();
  el.inspector.replaceChildren();
  el.inspectorCaption.textContent = "Evidence and Timeline open here.";
  hideVerdict();
  stickToBottom = true;

  appendSystemLine("Fresh session started — the previous investigation was cleared.");
  updatePrompt();
  refreshStatus();
  el.input.focus();
}

function wireEvents() {
  el.form.addEventListener("submit", (event) => {
    event.preventDefault();
    const value = el.input.value;
    el.input.value = "";
    runCommand(value);
    el.input.focus();
  });

  el.input.addEventListener("keydown", (event) => {
    if (event.key === "ArrowUp") {
      event.preventDefault();
      const result = moveUp(history, el.input.value);
      history = result.state;
      if (result.handled) el.input.value = result.value;
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      const result = moveDown(history, el.input.value);
      history = result.state;
      if (result.handled) el.input.value = result.value;
    }
  });

  el.terminal.addEventListener("scroll", () => {
    const distance =
      el.terminal.scrollHeight - el.terminal.scrollTop - el.terminal.clientHeight;
    stickToBottom = distance <= SCROLL_STICK_PX;
    if (stickToBottom) el.jumpLatest.hidden = true;
  });

  el.jumpLatest.addEventListener("click", () => {
    stickToBottom = true;
    el.terminal.scrollTop = el.terminal.scrollHeight;
    el.jumpLatest.hidden = true;
    el.input.focus();
  });

  // Clicking the terminal (without selecting text) returns focus to input.
  el.terminal.addEventListener("click", () => {
    if (window.getSelection()?.toString() === "") el.input.focus();
  });

  el.evidenceBtn.addEventListener("click", () => showInspector("evidence"));
  el.timelineBtn.addEventListener("click", () => showInspector("timeline"));
  el.helpBtn.addEventListener("click", () => runCommand("help"));
  el.resetBtn.addEventListener("click", resetInvestigation);
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

async function boot() {
  wireEvents();
  try {
    await init();
    game = new PaperTrails();
  } catch (err) {
    console.error("WASM initialization failed:", err);
    showFatal(
      "Failed to initialize the game engine. Serve this page over HTTP " +
        "(not file://) and make sure the WASM package is built — see the " +
        `README ("Browser interface"). Details: ${err}`,
    );
    return;
  }

  el.caseLabel.textContent = `${game.caseId()} — ${game.caseTitle()}`;
  setUiEnabled(true);
  updatePrompt();
  refreshStatus();
  appendSystemLine(
    "Session ready. Type help for commands; ↑/↓ recalls history. " +
      "Evidence and Timeline open in the inspector.",
  );
  stickToBottom = true;
  scrollToBottomIfStuck();
  el.input.focus();
}

boot();
