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
//! - [`case`] — Case 001 scaffolding assets

pub mod case;
pub mod command;
pub mod engine;
pub mod vfs;

pub use command::{Command, ParseError, tokenize};
pub use engine::{CommandOutput, GameEngine, GameState};
pub use vfs::{DirEntry, EntryInfo, EntryKind, VfsError, VirtualFile, VirtualFs, format_timestamp};
