//! `game-core` — the shared game engine for the Paper Trails CLI detective
//! game.
//!
//! The core is UI-agnostic: the native CLI and the future browser/WASM
//! frontend both call [`GameEngine::execute`] with a command string and render
//! the returned [`CommandOutput`].
//!
//! Module map:
//! - [`vfs`] — simulated filesystem and path normalization
//! - [`command`] — tokenizer and typed command parsing
//! - [`engine`] — command execution and player state
//! - [`forensics`] — byte-level analysis: strings, hexdump, SHA-256, decoding
//! - [`investigation`] — evidence registry mechanics, objectives, report grading
//! - [`case`] — Case 001 artifacts, evidence definitions, and rules

pub mod case;
pub mod command;
pub mod engine;
pub mod forensics;
pub mod investigation;
pub mod vfs;

pub use command::{Command, Encoding, ParseError, ReportField, tokenize};
pub use engine::{CommandOutput, GameEngine, GameState};
pub use forensics::DecodeError;
pub use investigation::{
    CaseDef, Evaluation, EvidenceDef, FieldStatus, ObjectiveDef, Report, Trigger,
};
pub use vfs::{DirEntry, EntryInfo, EntryKind, VfsError, VirtualFile, VirtualFs, format_timestamp};
