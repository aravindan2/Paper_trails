//! WebAssembly adapter for Paper Trails: a thin, stable JavaScript API over
//! the shared [`game_core::GameEngine`].
//!
//! # Design rules
//!
//! - **No second implementation.** Every player command is executed by the
//!   same [`GameEngine::execute`] the native CLI uses, with the same Case 001
//!   definition (`GameEngine::case_001`). This crate only converts values
//!   across the JS/WASM boundary.
//! - **No UI concerns.** Terminal rendering, DOM access, history, and
//!   persistence stay out of this crate (and out of `game-core`); the host
//!   page decides how to display returned strings.
//! - **Simple data only.** The exported surface is strings and arrays of
//!   strings — no serde, no JS object models, no exposed Rust internals.
//!
//! # JavaScript API
//!
//! ```js
//! import { PaperTrails } from "./pkg/paper_trails_wasm.js";
//!
//! const game = new PaperTrails();   // fresh Case 001 session
//! game.execute("ls");               // string: complete command output
//! game.discoveredEvidence();        // string[]: ["EV-002", ...] in order
//! game.completedObjectives();       // string[]: ["OBJ-001", ...]
//! game.execute("evidence");         // full evidence journal (a command)
//! game.execute("timeline");         // chronological timeline (a command)
//! game.cwd();                       // "/" (current virtual directory)
//! game.reset();                     // back to a clean session
//! ```
//!
//! # Error behavior
//!
//! - `execute` **never throws for game-level problems**: unknown commands,
//!   bad paths, syntax errors, and empty input return exactly the text the
//!   native CLI prints (`""` when there is nothing to print). Progress is
//!   unchanged on error, like the CLI.
//! - Passing a non-string where a string is expected throws a JS `TypeError`
//!   from the generated glue (standard wasm-bindgen behavior).
//! - The engine does not panic on player input (no `unwrap` on input paths);
//!   a hypothetical internal panic would surface as a WebAssembly
//!   `RuntimeError` trap rather than partial output.
//!
//! # Building
//!
//! See the repository README ("WebAssembly adapter") for target/tool
//! installation, build commands, and the smoke-test procedure.

use game_core::GameEngine;
use game_core::case::{CASE_ID, CASE_TITLE};

/// One game session: the shared engine plus the Case 001 image.
///
/// This is the adapter's entire logic — pure Rust, testable on the host.
/// The exported [`PaperTrails`] wrapper delegates here 1:1, so native tests
/// exercise the same code the browser runs.
pub struct Session {
    engine: GameEngine,
}

impl Default for Session {
    /// Equivalent to [`Session::new`]: a fresh Case 001 session.
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// Start a fresh Case 001 session: cwd `/`, no evidence, no objectives,
    /// empty report.
    pub fn new() -> Self {
        Self {
            engine: GameEngine::case_001(),
        }
    }

    /// Execute one command and return its complete output text — exactly
    /// what the native CLI prints for the same input, including appended
    /// discovery notices. Empty input yields `""`.
    pub fn execute(&mut self, command: &str) -> String {
        self.engine.execute(command).text
    }

    /// Reset to a clean session, equivalent to `Session::new()`.
    pub fn reset(&mut self) {
        self.engine = GameEngine::case_001();
    }

    /// Current directory inside the virtual filesystem.
    pub fn cwd(&self) -> &str {
        self.engine.cwd()
    }

    /// Stable case identifier shown by the UI header (e.g. `CASE-001`).
    pub fn case_id(&self) -> &'static str {
        CASE_ID
    }

    /// Case title shown by the UI header.
    pub fn case_title(&self) -> &'static str {
        CASE_TITLE
    }

    /// Discovered evidence IDs in discovery order, deduplicated. The full
    /// journal text (titles, observations, sources, cross-references) is
    /// available through `execute("evidence")`.
    pub fn discovered_evidence(&self) -> Vec<String> {
        self.engine.state().discovered_evidence.clone()
    }

    /// Completed objective IDs in completion order. Objective checkboxes are
    /// also part of `execute("evidence")`.
    pub fn completed_objectives(&self) -> Vec<String> {
        self.engine.state().completed_objectives.clone()
    }
}

// ---------------------------------------------------------------------------
// Exported bindings.
//
// `#[wasm_bindgen]` compiles on host targets as well (the macro only emits
// JS glue for wasm32), so `PaperTrails` is available to native tests too:
// tests and the smoke script therefore exercise the *same* exported struct,
// not just the underlying `Session`.
// ---------------------------------------------------------------------------

mod bindings {
    use super::Session;
    use wasm_bindgen::prelude::*;

    /// JavaScript handle to one game session.
    ///
    /// Methods mirror [`Session`] one-to-one; see the crate docs for the API
    /// contract and error behavior.
    #[wasm_bindgen]
    pub struct PaperTrails {
        inner: Session,
    }

    #[wasm_bindgen]
    impl PaperTrails {
        /// Create a fresh Case 001 session.
        #[wasm_bindgen(constructor)]
        pub fn new() -> PaperTrails {
            PaperTrails {
                inner: Session::new(),
            }
        }

        /// Execute one command; returns the complete output text
        /// (the same string the native CLI prints; `""` when empty).
        /// Never throws for game-level errors.
        pub fn execute(&mut self, command: &str) -> String {
            self.inner.execute(command)
        }

        /// Reset the session to a clean Case 001 state.
        pub fn reset(&mut self) {
            self.inner.reset();
        }

        /// Current directory inside the virtual filesystem.
        pub fn cwd(&self) -> String {
            self.inner.cwd().to_string()
        }

        /// Stable case identifier, e.g. `"CASE-001"`.
        #[wasm_bindgen(js_name = caseId)]
        pub fn case_id(&self) -> String {
            self.inner.case_id().to_string()
        }

        /// Case title, e.g. `"The Last Commit"`.
        #[wasm_bindgen(js_name = caseTitle)]
        pub fn case_title(&self) -> String {
            self.inner.case_title().to_string()
        }

        /// Discovered evidence IDs in discovery order (`string[]` in JS).
        /// Full journal text: `execute("evidence")`.
        #[wasm_bindgen(js_name = discoveredEvidence)]
        pub fn discovered_evidence(&self) -> Vec<String> {
            self.inner.discovered_evidence()
        }

        /// Completed objective IDs in completion order (`string[]` in JS).
        #[wasm_bindgen(js_name = completedObjectives)]
        pub fn completed_objectives(&self) -> Vec<String> {
            self.inner.completed_objectives()
        }
    }

    impl Default for PaperTrails {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub use bindings::PaperTrails;
