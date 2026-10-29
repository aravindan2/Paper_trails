//! The browser package must never carry development-only answer-key material.
//!
//! What ships to the browser comes from exactly three source trees:
//! `crates/game-core/src` (engine + authored case), `apps/wasm/src`
//! (adapter), and `apps/wasm/web/` (the page: HTML/CSS/JS plus the
//! `wasm-bindgen` glue in `web/pkg/` when present). This test scans all of
//! them for:
//!
//! - the private design doc's distinctive prose (mirroring
//!   `game-core/tests/answer_key.rs`, which covers the engine + native CLI);
//! - file-inclusion macros (`include_str!`/`include_bytes!`) in Rust
//!   sources that could accidentally embed `authoring/case-001-solution.md`
//!   later (a prose *mention* of the directory is fine; embedding it is not);
//! - references to the gitignored `authoring/` directory in the page code
//!   the browser actually loads (`index.html`, `app.js`, `term.js`,
//!   `style.css`).
//!
//! It also checks the shared smoke fixtures stay mechanics-only: they are
//! repo test data, but they feed the JS smoke test, so the accepted report
//! (solution keywords) must not appear there either.

use std::path::{Path, PathBuf};

/// Prose from the private case-design doc. See `answer_key.rs` for why
/// grading keywords and the decode trigger are deliberately allowed in the
/// engine: the narrative solution is what must never ship.
const MARKERS: [&str; 3] = [
    "Ground truth (what actually happened)",
    "private case design",
    "Dana Reyes used Elias's account",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_with_extension(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_with_extension(&path, extensions, out);
        } else if path
            .extension()
            .is_some_and(|ext| extensions.iter().any(|wanted| ext == *wanted))
        {
            out.push(path);
        }
    }
}

fn collect_rs_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    collect_with_extension(dir, &["rs"], out);
}

fn collect_web_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    collect_with_extension(dir, &["js", "html", "css", "json"], out);
}

#[test]
fn browser_package_sources_carry_no_answer_key() {
    let root = repo_root();

    let mut shipped = Vec::new();
    collect_rs_sources(&root.join("crates/game-core/src"), &mut shipped);
    collect_rs_sources(&root.join("apps/wasm/src"), &mut shipped);
    assert!(!shipped.is_empty(), "no package sources found");

    for file in &shipped {
        let source = std::fs::read_to_string(file).expect("readable source");
        for marker in MARKERS {
            assert!(
                !source.contains(marker),
                "answer-key prose in {}",
                file.display()
            );
        }
        assert!(
            !source.contains("include_str!") && !source.contains("include_bytes!"),
            "file inclusion in package source: {}",
            file.display()
        );
        // Prose may *mention* the private directory (e.g. case.rs explains
        // where the design doc lives); without an inclusion macro or the
        // doc's actual prose, nothing from it can reach the package.
    }
}

#[test]
fn web_page_sources_carry_no_answer_key() {
    let web = repo_root().join("apps/wasm/web");
    let mut shipped = Vec::new();
    collect_web_sources(&web, &mut shipped);
    // index.html / app.js / term.js / style.css / package.json must exist;
    // generated glue (web/pkg) and dev tools (serve.js, test.js) are
    // additionally present only after a build / in the repo.
    let required = [
        "index.html",
        "app.js",
        "term.js",
        "style.css",
        "package.json",
    ];
    for name in required {
        assert!(
            shipped.iter().any(|path| path.ends_with(name)),
            "missing web source: {name}"
        );
    }
    assert!(!shipped.is_empty());

    for file in &shipped {
        let source = std::fs::read_to_string(file).expect("readable source");
        for marker in MARKERS {
            assert!(
                !source.contains(marker),
                "answer-key prose in {}",
                file.display()
            );
        }
    }

    // The page code the browser loads must not reference the private
    // authoring directory at all. (serve.js/test.js are dev tools that the
    // page never loads, so they are exempt from this stricter rule.)
    for name in ["index.html", "app.js", "term.js", "style.css"] {
        let path = web.join(name);
        let source = std::fs::read_to_string(&path).expect("readable page source");
        assert!(
            !source.contains("authoring"),
            "page source references the authoring directory: {name}"
        );
    }
}

#[test]
fn smoke_fixtures_are_mechanics_only() {
    let fixtures = repo_root().join("apps/wasm/tests/fixtures");
    // Not .rs files — read the fixtures explicitly.
    let commands = std::fs::read_to_string(fixtures.join("commands.txt")).expect("commands.txt");
    let expected = std::fs::read_to_string(fixtures.join("expected.txt")).expect("expected.txt");
    assert!(!commands.trim().is_empty() && !expected.trim().is_empty());

    for (name, text) in [("commands.txt", &commands), ("expected.txt", &expected)] {
        assert!(
            !text.contains("verdict: ACCEPTED"),
            "solution submission in fixture {name}"
        );
        // The accepted report's keyword set must not be assembled here; the
        // neutral `submit` in the sequence only ever reaches INCOMPLETE.
        assert!(
            !text.contains("submit who dreyes"),
            "solution keywords in fixture {name}"
        );
    }
}
