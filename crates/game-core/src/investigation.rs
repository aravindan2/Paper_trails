//! Investigation engine: evidence registry, discovery, objectives, reporting.
//!
//! This module is **case-agnostic**: it defines what an evidence item looks
//! like, how a finished command proves that the player actually saw something
//! relevant ([`Inspection`] / [`Trigger`]), how objectives complete, and how a
//! final report is evaluated ([`evaluate`]). Case 001 supplies its own
//! definitions in [`crate::case`].
//!
//! Design rules enforced here:
//!
//! - Evidence is recorded from **content surfaced to the player** (displayed
//!   lines, extracted strings, shown metadata, decoded values) — never from
//!   the mere fact that a command ran. `ls`ing a directory or `stat`ing an
//!   unrelated file records nothing.
//! - Each item has a stable ID and is recorded **at most once**; repeated
//!   discoveries are silent.
//! - Evaluation gives field-by-field feedback (`missing` / `not supported` /
//!   `ok`) and an evidence sufficiency check without printing the answer.
//! - Nothing here touches the host filesystem or any terminal/browser API, so
//!   the module is directly portable to WASM.

use crate::command::ReportField;

/// What a completed command surfaced to the player, in enough detail for the
/// registry to decide whether an observation was actually seen. Paths are
/// **normalized absolute paths** so relative player input matches the same
/// registry entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inspection {
    /// Full text of a file was displayed (`cat`).
    Text { path: String, content: String },
    /// These lines of a file were displayed (`grep`); only matched lines are
    /// stored because those are what the player read.
    Lines { path: String, lines: Vec<String> },
    /// Printable ASCII runs were shown (`strings`, and the ASCII gutter of
    /// `hexdump`).
    Bytes { path: String, runs: Vec<String> },
    /// Metadata was shown (`stat`).
    Meta { path: String },
    /// A decoded value was shown (`decode`).
    Decoded { text: String },
}

/// How an evidence item is discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Displayed content from `path` contains `needle` (matching line,
    /// displayed file text, or an extracted printable run).
    Content {
        path: &'static str,
        needle: &'static str,
    },
    /// Metadata for `path` was displayed (`stat PATH`).
    Metadata { path: &'static str },
    /// `decode` produced exactly `plaintext`.
    Decode { plaintext: &'static str },
}

impl Trigger {
    /// Does this inspection prove the player saw the trigger content?
    pub fn matches(&self, inspection: &Inspection) -> bool {
        match (self, inspection) {
            (
                Trigger::Content { path, needle },
                Inspection::Text {
                    path: shown,
                    content,
                },
            ) => shown == path && content.contains(needle),
            (Trigger::Content { path, needle }, Inspection::Lines { path: shown, lines }) => {
                shown == path && lines.iter().any(|line| line.contains(needle))
            }
            (Trigger::Content { path, needle }, Inspection::Bytes { path: shown, runs }) => {
                shown == path && runs.iter().any(|run| run.contains(needle))
            }
            (Trigger::Metadata { path }, Inspection::Meta { path: shown }) => shown == path,
            (Trigger::Decode { plaintext }, Inspection::Decoded { text }) => text == plaintext,
            _ => false,
        }
    }
}

/// One registered evidence item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceDef {
    /// Stable ID, e.g. `EV-001`. Never reused or renumbered.
    pub id: &'static str,
    pub title: &'static str,
    /// Player-facing observation shown once the item is discovered.
    pub observation: &'static str,
    /// Source artifact (normalized absolute path) for provenance.
    pub source: &'static str,
    /// IDs of related evidence, shown as journal cross-references.
    pub related: &'static [&'static str],
    /// Optional timeline timestamp (Unix seconds, UTC).
    pub time: Option<i64>,
    pub trigger: Trigger,
}

/// A case objective: completed automatically when all required evidence is
/// recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveDef {
    pub id: &'static str,
    pub title: &'static str,
    pub requires: &'static [&'static str],
}

/// How a final report is graded. Keyword lists are intentionally generous
/// (case-insensitive substrings): success must reward evidence-backed
/// explanations, not exact phrasing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationRules {
    /// Minimum number of recorded evidence items for acceptance.
    pub min_evidence: usize,
    /// Evidence that must be among the recorded items regardless of count.
    pub required_evidence: &'static [&'static str],
    /// `who` is satisfied by naming any of these (person or account).
    pub who_tokens: &'static [&'static str],
    /// `did` must mention this anchor concept...
    pub did_anchor: &'static str,
    /// ...and at least one token describing what was done to it.
    pub did_context_tokens: &'static [&'static str],
    /// `why` is satisfied by any of these motive/context tokens.
    pub why_tokens: &'static [&'static str],
}

/// Everything the engine needs to run one case's investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseDef {
    pub id: &'static str,
    pub title: &'static str,
    pub evidence: &'static [EvidenceDef],
    pub objectives: &'static [ObjectiveDef],
    pub rules: &'static EvaluationRules,
}

/// The player's final report fields, recorded via `submit FIELD TEXT`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    who: Option<String>,
    did: Option<String>,
    why: Option<String>,
}

impl Report {
    pub fn get(&self, field: ReportField) -> Option<&str> {
        match field {
            ReportField::Who => self.who.as_deref(),
            ReportField::Did => self.did.as_deref(),
            ReportField::Why => self.why.as_deref(),
        }
    }

    pub fn set(&mut self, field: ReportField, text: String) {
        match field {
            ReportField::Who => self.who = Some(text),
            ReportField::Did => self.did = Some(text),
            ReportField::Why => self.why = Some(text),
        }
    }

    /// `true` when no field has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.who.is_none() && self.did.is_none() && self.why.is_none()
    }
}

/// Outcome for one report field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldStatus {
    /// The field was never recorded (or recorded empty).
    Missing,
    /// Recorded, but the recorded text is not supported by the evidence.
    Unsupported,
    /// Recorded and consistent with the evidence.
    Ok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldAssessment {
    pub field: ReportField,
    pub status: FieldStatus,
}

/// Result of grading a report. Deliberately says *what* is missing without
/// printing the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// Recorded evidence items.
    pub discovered: usize,
    /// Total registered evidence items.
    pub total: usize,
    pub min_evidence: usize,
    /// Enough evidence overall *and* all `required_evidence` present.
    pub evidence_satisfied: bool,
    /// Only the required-items part of [`Self::evidence_satisfied`].
    pub key_observations_covered: bool,
    /// who / did / why assessments, in that order.
    pub fields: [FieldAssessment; 3],
    /// Accepted only with satisfied evidence and three `Ok` fields.
    pub accepted: bool,
}

/// Record evidence triggered by `inspections` and complete objectives, then
/// return deterministic notice lines for the command output:
/// `evidence recorded: EV-001 — Title` followed by
/// `objective complete: OBJ-001 — Title`.
///
/// Already-discovered items are skipped silently, so repeating an inspection
/// never duplicates journal entries or notices.
pub fn record_discoveries(
    case: &CaseDef,
    discovered: &mut Vec<String>,
    completed: &mut Vec<String>,
    inspections: &[Inspection],
) -> Vec<String> {
    let mut notices = Vec::new();
    for inspection in inspections {
        for def in case.evidence {
            if discovered.iter().any(|seen| seen == def.id) {
                continue;
            }
            if def.trigger.matches(inspection) {
                discovered.push(def.id.to_string());
                notices.push(format!("evidence recorded: {} — {}", def.id, def.title));
            }
        }
    }
    for objective in case.objectives {
        if completed.iter().any(|seen| seen == objective.id) {
            continue;
        }
        if objective
            .requires
            .iter()
            .all(|id| discovered.iter().any(|seen| seen == id))
        {
            completed.push(objective.id.to_string());
            notices.push(format!(
                "objective complete: {} — {}",
                objective.id, objective.title
            ));
        }
    }
    notices
}

/// Grade the player's report against the case rules.
pub fn evaluate(case: &CaseDef, discovered: &[String], report: &Report) -> Evaluation {
    let rules = case.rules;
    let key_observations_covered = rules
        .required_evidence
        .iter()
        .all(|required| discovered.iter().any(|seen| seen == required));
    let evidence_satisfied = discovered.len() >= rules.min_evidence && key_observations_covered;

    let fields =
        [ReportField::Who, ReportField::Did, ReportField::Why].map(|field| FieldAssessment {
            field,
            status: assess(field, report.get(field), rules),
        });

    Evaluation {
        discovered: discovered.len(),
        total: case.evidence.len(),
        min_evidence: rules.min_evidence,
        evidence_satisfied,
        key_observations_covered,
        fields,
        accepted: evidence_satisfied && fields.iter().all(|field| field.status == FieldStatus::Ok),
    }
}

fn assess(field: ReportField, text: Option<&str>, rules: &EvaluationRules) -> FieldStatus {
    let Some(text) = text else {
        return FieldStatus::Missing;
    };
    if text.trim().is_empty() {
        return FieldStatus::Missing;
    }
    let lower = text.to_ascii_lowercase();
    let supported = match field {
        ReportField::Who => rules.who_tokens.iter().any(|token| lower.contains(token)),
        ReportField::Did => {
            lower.contains(rules.did_anchor)
                && rules
                    .did_context_tokens
                    .iter()
                    .any(|token| lower.contains(token))
        }
        ReportField::Why => rules.why_tokens.iter().any(|token| lower.contains(token)),
    };
    if supported {
        FieldStatus::Ok
    } else {
        FieldStatus::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic definitions for exercising the generic mechanics without
    /// touching Case 001 content.
    const TEST_EVIDENCE: &[EvidenceDef] = &[
        EvidenceDef {
            id: "EV-001",
            title: "title",
            observation: "observation",
            source: "/src",
            related: &[],
            time: None,
            trigger: Trigger::Content {
                path: "/a.log",
                needle: "gotcha",
            },
        },
        EvidenceDef {
            id: "EV-002",
            title: "title",
            observation: "observation",
            source: "/src",
            related: &[],
            time: None,
            trigger: Trigger::Metadata { path: "/b.bin" },
        },
        EvidenceDef {
            id: "EV-003",
            title: "title",
            observation: "observation",
            source: "/src",
            related: &[],
            time: None,
            trigger: Trigger::Decode {
                plaintext: "hello world",
            },
        },
    ];

    fn case() -> CaseDef {
        CaseDef {
            id: "CASE-T",
            title: "Test",
            evidence: TEST_EVIDENCE,
            objectives: &[ObjectiveDef {
                id: "OBJ-001",
                title: "objective",
                requires: &["EV-001", "EV-002"],
            }],
            rules: &EvaluationRules {
                min_evidence: 2,
                required_evidence: &["EV-003"],
                who_tokens: &["alice"],
                did_anchor: "export",
                did_context_tokens: &["enable"],
                why_tokens: &["pilot"],
            },
        }
    }

    #[test]
    fn content_trigger_matches_displayed_lines_text_and_runs() {
        let trigger = Trigger::Content {
            path: "/a.log",
            needle: "gotcha",
        };
        assert!(trigger.matches(&Inspection::Text {
            path: "/a.log".into(),
            content: "x\ngotcha here\n".into()
        }));
        assert!(trigger.matches(&Inspection::Lines {
            path: "/a.log".into(),
            lines: vec!["prefix gotcha suffix".into()]
        }));
        assert!(trigger.matches(&Inspection::Bytes {
            path: "/a.log".into(),
            runs: vec!["nothing".into(), "say gotcha now".into()]
        }));
        // Wrong path, missing needle, or an unrelated inspection never match.
        assert!(!trigger.matches(&Inspection::Text {
            path: "/other.log".into(),
            content: "gotcha".into()
        }));
        assert!(!trigger.matches(&Inspection::Text {
            path: "/a.log".into(),
            content: "benign".into()
        }));
        assert!(!trigger.matches(&Inspection::Lines {
            path: "/elsewhere.log".into(),
            lines: vec!["gotcha".into()]
        }));
        assert!(!trigger.matches(&Inspection::Meta {
            path: "/a.log".into()
        }));
        // A needle split across separate runs is not "surfaced".
        assert!(!trigger.matches(&Inspection::Bytes {
            path: "/a.log".into(),
            runs: vec!["got".into(), "cha".into()]
        }));
    }

    #[test]
    fn metadata_and_decode_triggers_are_exact() {
        let meta = Trigger::Metadata { path: "/b.bin" };
        assert!(meta.matches(&Inspection::Meta {
            path: "/b.bin".into()
        }));
        assert!(!meta.matches(&Inspection::Meta {
            path: "/c.bin".into()
        }));
        assert!(!meta.matches(&Inspection::Text {
            path: "/b.bin".into(),
            content: "x".into()
        }));

        let decode = Trigger::Decode {
            plaintext: "hello world",
        };
        assert!(decode.matches(&Inspection::Decoded {
            text: "hello world".into()
        }));
        assert!(!decode.matches(&Inspection::Decoded {
            text: "hello world!".into()
        }));
        assert!(!decode.matches(&Inspection::Text {
            path: "/x".into(),
            content: "hello world".into()
        }));
    }

    #[test]
    fn discoveries_are_recorded_once_with_stable_notices() {
        let case = case();
        let insp = Inspection::Text {
            path: "/a.log".into(),
            content: "gotcha".into(),
        };
        let mut discovered = Vec::new();
        let mut completed = Vec::new();
        let notices = record_discoveries(
            &case,
            &mut discovered,
            &mut completed,
            std::slice::from_ref(&insp),
        );
        assert_eq!(discovered, vec!["EV-001"]);
        // Objectives are not complete until all required evidence exists.
        assert!(completed.is_empty());
        assert_eq!(
            notices,
            vec!["evidence recorded: EV-001 — title".to_string()]
        );

        // Repeating the same inspection records nothing new.
        let notices = record_discoveries(&case, &mut discovered, &mut completed, &[insp]);
        assert_eq!(discovered, vec!["EV-001"]);
        assert!(completed.is_empty());
        assert!(notices.is_empty());
    }

    #[test]
    fn objectives_complete_when_required_evidence_accumulates() {
        let case = case();
        let mut discovered = Vec::new();
        let mut completed = Vec::new();

        let first = record_discoveries(
            &case,
            &mut discovered,
            &mut completed,
            &[Inspection::Meta {
                path: "/b.bin".into(),
            }],
        );
        // The evidence itself is recorded, but one of two required items does
        // not complete the objective.
        assert_eq!(first, vec!["evidence recorded: EV-002 — title".to_string()]);
        assert!(completed.is_empty());

        let second = record_discoveries(
            &case,
            &mut discovered,
            &mut completed,
            &[Inspection::Text {
                path: "/a.log".into(),
                content: "gotcha".into(),
            }],
        );
        assert_eq!(
            second,
            vec![
                "evidence recorded: EV-001 — title".to_string(),
                "objective complete: OBJ-001 — objective".to_string()
            ]
        );
        assert_eq!(completed, vec!["OBJ-001"]);

        // Re-running after completion stays silent.
        let third = record_discoveries(
            &case,
            &mut discovered,
            &mut completed,
            &[Inspection::Text {
                path: "/a.log".into(),
                content: "gotcha".into(),
            }],
        );
        assert!(third.is_empty());
    }

    #[test]
    fn evaluation_needs_evidence_and_three_supported_fields() {
        let case = case();
        let mut report = Report::default();

        // Nothing recorded: everything missing, nothing accepted.
        let eval = evaluate(&case, &[], &report);
        assert_eq!(eval.discovered, 0);
        assert_eq!(eval.total, 3);
        assert_eq!(eval.min_evidence, 2);
        assert!(!eval.evidence_satisfied);
        assert!(!eval.key_observations_covered);
        assert!(eval.fields.iter().all(|f| f.status == FieldStatus::Missing));
        assert!(!eval.accepted);

        // Supported fields but insufficient evidence: still not accepted.
        report.set(ReportField::Who, "alice".to_string());
        report.set(ReportField::Did, "enabled the export".to_string());
        report.set(ReportField::Why, "for the pilot".to_string());
        let eval = evaluate(&case, &["EV-001".to_string()], &report);
        assert!(!eval.evidence_satisfied);
        assert!(!eval.key_observations_covered);
        assert!(eval.fields.iter().all(|f| f.status == FieldStatus::Ok));
        assert!(!eval.accepted);

        // Full evidence but a missing key observation blocks acceptance.
        let eval = evaluate(
            &case,
            &["EV-001".to_string(), "EV-002".to_string()],
            &report,
        );
        assert!(!eval.evidence_satisfied, "required EV-003 is missing");
        assert!(!eval.key_observations_covered);
        assert!(!eval.accepted);

        // The count floor applies independently of the required set: the key
        // observation alone is not enough evidence either.
        let eval = evaluate(&case, &["EV-003".to_string()], &report);
        assert!(eval.key_observations_covered);
        assert!(!eval.evidence_satisfied, "count is below min_evidence");
        assert!(!eval.accepted);

        // Complete evidence plus supported fields: accepted.
        let all = vec![
            "EV-001".to_string(),
            "EV-002".to_string(),
            "EV-003".to_string(),
        ];
        let eval = evaluate(&case, &all, &report);
        assert!(eval.evidence_satisfied);
        assert!(eval.key_observations_covered);
        assert!(eval.accepted);
    }

    #[test]
    fn field_assessment_distinguishes_missing_unsupported_and_ok() {
        let case = case();
        let mut report = Report::default();
        report.set(ReportField::Who, "someone else".to_string());
        report.set(ReportField::Did, "export".to_string()); // anchor only
        report.set(ReportField::Why, "   ".to_string()); // whitespace = missing

        let eval = evaluate(&case, &[], &report);
        let statuses: Vec<FieldStatus> = eval.fields.iter().map(|f| f.status).collect();
        assert_eq!(
            statuses,
            vec![
                FieldStatus::Unsupported, // "someone else" has no who token
                FieldStatus::Unsupported, // "export" lacks a context token
                FieldStatus::Missing,     // whitespace-only
            ]
        );
        // Assessment is case-insensitive.
        report.set(ReportField::Who, "ALICE".to_string());
        report.set(ReportField::Did, "Export ENABLED".to_string());
        report.set(ReportField::Why, "The Pilot".to_string());
        let eval = evaluate(&case, &[], &report);
        assert!(eval.fields.iter().all(|f| f.status == FieldStatus::Ok));
        assert!(!eval.accepted, "supported fields alone never accept");
    }

    #[test]
    fn report_roundtrips_fields() {
        let mut report = Report::default();
        assert!(report.is_empty());
        report.set(ReportField::Did, "changed it".to_string());
        assert!(!report.is_empty());
        assert_eq!(report.get(ReportField::Did), Some("changed it"));
        assert_eq!(report.get(ReportField::Who), None);
        report.set(ReportField::Did, "revised".to_string());
        assert_eq!(report.get(ReportField::Did), Some("revised"));
    }
}
