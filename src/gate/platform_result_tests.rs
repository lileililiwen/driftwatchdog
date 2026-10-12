
use super::*;
use crate::gate::types::{EvidenceRef, GateSeverity};

fn result(id: &str, status: GateStatus) -> GateResult {
    GateResult {
        gate_id: id.to_string(),
        source: "test".to_string(),
        status,
        severity: GateSeverity::Info,
        findings: vec![],
        evidence: vec![],
        missing_evidence: vec![],
        diagnostic: None,
        remediation: None,
    }
}

fn passing() -> GateResult {
    result("tests", GateStatus::Pass)
}

fn run<'a>(results: &'a [GateResult], aggregate: GateStatus) -> PlatformRun<'a> {
    PlatformRun {
        gate_id: "sha256:0123456789abcdef",
        pipeline: PLATFORM_PIPELINE,
        profile: "rust-product",
        evaluated_at: "2026-10-12T10:00:00Z",
        subject_ref: Some("9f1c0de"),
        aggregate,
        results,
    }
}

// --- the mapping table -----------------------------------------------

#[test]
fn pass_maps_to_passed() {
    assert_eq!(
        map_status(GateStatus::Pass),
        StatusMapping::Representable(PlatformStatus::Passed)
    );
}

#[test]
fn fail_maps_to_failed() {
    assert_eq!(
        map_status(GateStatus::Fail),
        StatusMapping::Representable(PlatformStatus::Failed)
    );
}

#[test]
fn not_applicable_maps_to_skipped() {
    assert_eq!(
        map_status(GateStatus::NotApplicable),
        StatusMapping::Representable(PlatformStatus::Skipped)
    );
}

#[test]
fn review_required_is_unrepresentable() {
    assert_eq!(
        map_status(GateStatus::ReviewRequired),
        StatusMapping::Unrepresentable("REVIEW_REQUIRED")
    );
}

#[test]
fn unbound_is_unrepresentable() {
    assert_eq!(
        map_status(GateStatus::Unbound),
        StatusMapping::Unrepresentable("UNBOUND")
    );
}

/// The table is total, so no state can slip through un-mapped. The
/// `errored` value is reachable only here, which is the point: the
/// emitter never constructs it.
#[test]
fn every_status_has_an_arm_and_errored_is_never_produced() {
    let all = [
        GateStatus::Pass,
        GateStatus::Fail,
        GateStatus::ReviewRequired,
        GateStatus::NotApplicable,
        GateStatus::Unbound,
    ];
    for s in all {
        let produced = match map_status(s) {
            StatusMapping::Representable(p) => Some(p),
            StatusMapping::Unrepresentable(_) => None,
        };
        assert_ne!(
            produced,
            Some(PlatformStatus::Errored),
            "{s:?} must never become `errored`"
        );
    }
}

#[test]
fn platform_status_wire_strings_match_the_schema_enum() {
    assert_eq!(PlatformStatus::Passed.as_str(), "passed");
    assert_eq!(PlatformStatus::Failed.as_str(), "failed");
    assert_eq!(PlatformStatus::Errored.as_str(), "errored");
    assert_eq!(PlatformStatus::Skipped.as_str(), "skipped");
    for s in [
        PlatformStatus::Passed,
        PlatformStatus::Failed,
        PlatformStatus::Errored,
        PlatformStatus::Skipped,
    ] {
        assert_eq!(PlatformStatus::parse(s.as_str()), Some(s));
    }
    assert_eq!(PlatformStatus::parse("PASS"), None);
}

// --- pattern validators ----------------------------------------------

#[test]
fn gate_id_pattern_accepts_and_rejects_the_right_values() {
    for ok in [
        "a",
        "sha256:0123456789abcdef",
        "driftwatchdog",
        "A.b_c:d-e",
        "9",
    ] {
        assert!(matches_gate_id(ok), "{ok} should match");
    }
    for bad in [
        "",
        "-leading",
        ".leading",
        "_leading",
        ":leading",
        "has space",
        "héllo",
    ] {
        assert!(!matches_gate_id(bad), "{bad} should not match");
    }
    // 128 characters is the cap; 129 is not.
    assert!(matches_gate_id(&"a".repeat(128)));
    assert!(!matches_gate_id(&"a".repeat(129)));
}

#[test]
fn pipeline_pattern_accepts_and_rejects_the_right_values() {
    for ok in [
        "driftwatchdog-gate",
        "rust-product",
        "minimal",
        "a.b_c-d",
        "a0",
    ] {
        assert!(matches_pipeline(ok), "{ok} should match");
    }
    for bad in ["", "Rust", "0abc", "has space", "with@at", "héllo"] {
        assert!(!matches_pipeline(bad), "{bad} should not match");
    }
    assert!(matches_pipeline(&"a".repeat(128)));
    assert!(!matches_pipeline(&"a".repeat(129)));
}

#[test]
fn check_id_pattern_accepts_and_rejects_the_right_values() {
    for ok in [
        "tests",
        "governance-quality",
        "exemption-budget-2",
        "source-file-size",
        "c0ffee",
        "a",
        "a.b_c-d",
    ] {
        assert!(matches_check_id(ok), "{ok} should match");
    }
    // The exact cases the change is about.
    for bad in ["", "L1-configuration", "0abc", "My Check", "résumé", "a:b"] {
        assert!(!matches_check_id(bad), "{bad} should not match");
    }
    assert!(matches_check_id(&"a".repeat(64)));
    assert!(!matches_check_id(&"a".repeat(65)));
}

// --- build: the happy paths ------------------------------------------

#[test]
fn a_clean_pass_builds_a_conforming_document() {
    let results = vec![passing(), result("build", GateStatus::Pass)];
    let doc = build(run(&results, GateStatus::Pass)).unwrap();
    assert_eq!(doc.gate_id, "sha256:0123456789abcdef");
    assert_eq!(doc.pipeline, PLATFORM_PIPELINE);
    assert_eq!(doc.evaluated_at, "2026-10-12T10:00:00Z");
    assert_eq!(doc.result, "passed");
    assert_eq!(doc.subject_ref.as_deref(), Some("9f1c0de"));
    assert_eq!(doc.driftwatch_profile, "rust-product");
    assert_eq!(doc.checks.len(), 2);
    assert_eq!(doc.checks[0].check_id, "tests");
    assert_eq!(doc.checks[0].status, "passed");
    assert_eq!(doc.checks[0].driftwatch_gate_id, "tests");
    assert!(doc.evidence_refs.is_empty());
}

#[test]
fn a_clean_fail_builds_a_failing_document() {
    let results = vec![result("tests", GateStatus::Fail)];
    let doc = build(run(&results, GateStatus::Fail)).unwrap();
    assert_eq!(doc.result, "failed");
    assert_eq!(doc.checks[0].status, "failed");
}

#[test]
fn a_not_applicable_check_is_emitted_as_skipped() {
    let results = vec![result("ux-review", GateStatus::NotApplicable)];
    let doc = build(run(&results, GateStatus::Pass)).unwrap();
    assert_eq!(doc.result, "passed");
    assert_eq!(doc.checks[0].status, "skipped");
    assert_eq!(doc.checks[0].check_id, "ux-review");
}

#[test]
fn an_optional_check_still_appears_with_its_own_status() {
    // Optionality is not representable in the platform document. The
    // check is emitted with its real status and the root `result` stays
    // authoritative, which is what the contract says decides a deploy.
    let results = vec![passing(), result("advisory", GateStatus::Fail)];
    let doc = build(run(&results, GateStatus::Pass)).unwrap();
    assert_eq!(doc.result, "passed");
    assert_eq!(doc.checks[1].status, "failed");
}

#[test]
fn diagnostic_becomes_the_check_detail() {
    let mut r = result("tests", GateStatus::Fail);
    r.diagnostic = Some("cargo test: 3 failures".to_string());
    let doc = build(run(&[r], GateStatus::Fail)).unwrap();
    assert_eq!(
        doc.checks[0].detail.as_deref(),
        Some("cargo test: 3 failures")
    );
}

#[test]
fn an_absent_diagnostic_is_omitted_rather_than_null() {
    let doc = build(run(&[passing()], GateStatus::Pass)).unwrap();
    assert_eq!(doc.checks[0].detail, None);
    let json = serde_json::to_string(&doc).unwrap();
    assert!(!json.contains("\"detail\""), "detail omitted: {json}");
}

#[test]
fn an_empty_diagnostic_is_omitted_rather_than_emitted_empty() {
    let mut r = passing();
    r.diagnostic = Some(String::new());
    let doc = build(run(&[r], GateStatus::Pass)).unwrap();
    assert_eq!(doc.checks[0].detail, None);
}

#[test]
fn the_document_serialises_to_the_contract_field_names() {
    let doc = build(run(&[passing()], GateStatus::Pass)).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    for required in ["gate_id", "pipeline", "evaluated_at", "result"] {
        assert!(value.get(required).is_some(), "missing {required}");
    }
    assert_eq!(value["checks"][0]["check_id"], "tests");
    assert_eq!(value["checks"][0]["status"], "passed");
    // Additional properties the schema permits.
    assert_eq!(value["checks"][0]["driftwatch_gate_id"], "tests");
    assert_eq!(value["driftwatch_profile"], "rust-product");
    // Never emitted: driftwatchdog has no tenant concept.
    assert!(value.get("tenant_id").is_none());
}

// --- refusals ---------------------------------------------------------

#[test]
fn aggregate_review_required_refuses() {
    let err = build(run(&[passing()], GateStatus::ReviewRequired)).unwrap_err();
    assert_eq!(
        err,
        Refusal::AggregateStatus {
            status: "REVIEW_REQUIRED",
            because: REVIEW_REQUIRED_BECAUSE,
        }
    );
    assert!(err.to_string().contains("REVIEW_REQUIRED"));
    assert!(err.to_string().contains("no human-decision state"));
}

#[test]
fn aggregate_unbound_refuses() {
    let err = build(run(&[passing()], GateStatus::Unbound)).unwrap_err();
    assert_eq!(
        err,
        Refusal::AggregateStatus {
            status: "UNBOUND",
            because: UNBOUND_BECAUSE,
        }
    );
    assert!(err.to_string().contains("UNBOUND"));
}

#[test]
fn a_check_under_review_refuses_and_names_the_ids() {
    let results = vec![passing(), result("slow", GateStatus::ReviewRequired)];
    let err = build(run(&results, GateStatus::Pass)).unwrap_err();
    assert_eq!(
        err,
        Refusal::CheckStatus {
            status: "REVIEW_REQUIRED",
            ids: "slow".to_string(),
        }
    );
    assert!(err.to_string().contains("slow"));
}

#[test]
fn every_unrepresentable_check_id_is_named_once_and_sorted() {
    let results = vec![
        result("zzz", GateStatus::Unbound),
        result("aaa", GateStatus::Unbound),
        result("aaa", GateStatus::Unbound),
        result("mmm", GateStatus::ReviewRequired),
    ];
    let err = build(run(&results, GateStatus::Pass)).unwrap_err();
    // BTreeMap iteration is ordered, so REVIEW_REQUIRED is reported first.
    assert_eq!(
        err,
        Refusal::CheckStatus {
            status: "REVIEW_REQUIRED",
            ids: "mmm".to_string(),
        }
    );
    let results = vec![
        result("zzz", GateStatus::Unbound),
        result("aaa", GateStatus::Unbound),
        result("aaa", GateStatus::Unbound),
    ];
    let err = build(run(&results, GateStatus::Pass)).unwrap_err();
    assert_eq!(
        err,
        Refusal::CheckStatus {
            status: "UNBOUND",
            ids: "aaa, zzz".to_string(),
        }
    );
}

/// The floor is "refuse on a required check". This implementation is
/// stricter: an optional pending review coerced to `skipped` would
/// publish the same false claim, so it refuses too.
#[test]
fn an_optional_review_under_review_also_refuses() {
    let results = vec![passing(), result("advisory", GateStatus::ReviewRequired)];
    let err = build(run(&results, GateStatus::Pass)).unwrap_err();
    assert_eq!(
        err,
        Refusal::CheckStatus {
            status: "REVIEW_REQUIRED",
            ids: "advisory".to_string(),
        }
    );
}

#[test]
fn a_non_conforming_check_id_refuses_and_names_the_pattern() {
    let results = vec![result("L1-configuration", GateStatus::Pass)];
    let err = build(run(&results, GateStatus::Pass)).unwrap_err();
    assert_eq!(
        err,
        Refusal::CheckId {
            value: "L1-configuration".to_string(),
            pattern: CHECK_ID_PATTERN,
            max: MAX_CHECK_ID_CHARS,
        }
    );
    let text = err.to_string();
    assert!(text.contains("L1-configuration"), "{text}");
    assert!(text.contains("^[a-z][a-z0-9_.-]{0,63}$"), "{text}");
    assert!(text.contains("no reversible slug"), "{text}");
}

#[test]
fn a_sixty_five_character_check_id_refuses() {
    let long = "a".repeat(MAX_CHECK_ID_CHARS + 1);
    let err = build(run(&[result(&long, GateStatus::Pass)], GateStatus::Pass)).unwrap_err();
    assert!(matches!(err, Refusal::CheckId { .. }), "{err}");
    // 64 is fine; 65 is not.
    let ok = "a".repeat(MAX_CHECK_ID_CHARS);
    assert!(build(run(&[result(&ok, GateStatus::Pass)], GateStatus::Pass)).is_ok());
}

#[test]
fn an_ellipsis_from_the_upstream_id_bound_refuses_rather_than_emitting() {
    // `commands/gate.rs` truncates a check id to 128 characters with an
    // ellipsis. The truncated value contains a character outside the
    // platform alphabet, so the emitter reports it instead of shipping it.
    let truncated = format!("{}…", "a".repeat(10));
    let err = build(run(
        &[result(&truncated, GateStatus::Pass)],
        GateStatus::Pass,
    ))
    .unwrap_err();
    assert!(matches!(err, Refusal::CheckId { .. }), "{err}");
}

#[test]
fn an_over_length_diagnostic_refuses_with_the_check_and_length() {
    let mut r = result("tests", GateStatus::Fail);
    r.diagnostic = Some("x".repeat(MAX_DETAIL_CHARS + 1));
    let err = build(run(&[r], GateStatus::Fail)).unwrap_err();
    assert_eq!(
        err,
        Refusal::Detail {
            check_id: "tests".to_string(),
            length: MAX_DETAIL_CHARS + 1,
            max: MAX_DETAIL_CHARS,
        }
    );
    let text = err.to_string();
    assert!(text.contains("tests"), "{text}");
    assert!(text.contains("does not truncate"), "{text}");
}

#[test]
fn a_diagnostic_at_exactly_the_limit_is_emitted() {
    let mut r = result("tests", GateStatus::Fail);
    r.diagnostic = Some("x".repeat(MAX_DETAIL_CHARS));
    let doc = build(run(&[r], GateStatus::Fail)).unwrap();
    assert_eq!(
        doc.checks[0].detail.as_ref().unwrap().chars().count(),
        MAX_DETAIL_CHARS
    );
}

/// The schema counts code points; `str::len()` counts bytes. A
/// 300-byte, 300-character multi-byte diagnostic must still be measured
/// correctly, and one over the code-point limit must refuse even when it
/// is under the byte limit in the other direction.
#[test]
fn detail_length_is_measured_in_code_points_not_bytes() {
    let mut r = result("tests", GateStatus::Fail);
    // 512 two-byte characters: 1024 bytes, 512 code points. Conforming.
    r.diagnostic = Some("é".repeat(MAX_DETAIL_CHARS));
    let doc = build(run(&[r.clone()], GateStatus::Fail)).unwrap();
    assert_eq!(
        doc.checks[0].detail.as_ref().unwrap().chars().count(),
        MAX_DETAIL_CHARS
    );

    // 513 two-byte characters: 1026 bytes, 513 code points. Not
    // conforming, and the refusal reports 513 — not 1026.
    r.diagnostic = Some("é".repeat(MAX_DETAIL_CHARS + 1));
    let err = build(run(&[r], GateStatus::Fail)).unwrap_err();
    assert_eq!(
        err,
        Refusal::Detail {
            check_id: "tests".to_string(),
            length: MAX_DETAIL_CHARS + 1,
            max: MAX_DETAIL_CHARS,
        }
    );
}

#[test]
fn an_over_length_subject_ref_refuses() {
    let results = [passing()];
    let mut r = run(&results, GateStatus::Pass);
    let long_revision = "r".repeat(MAX_SUBJECT_REF_CHARS + 1);
    r.subject_ref = Some(&long_revision);
    let err = build(r).unwrap_err();
    assert_eq!(
        err,
        Refusal::SubjectRef {
            length: MAX_SUBJECT_REF_CHARS + 1,
            max: MAX_SUBJECT_REF_CHARS,
        }
    );
    assert!(err.to_string().contains("does not truncate"));
}

#[test]
fn a_non_rfc3339_timestamp_refuses() {
    let results = [passing()];
    let mut r = run(&results, GateStatus::Pass);
    r.evaluated_at = "2026-10-12 10:00:00";
    let err = build(r).unwrap_err();
    assert_eq!(
        err,
        Refusal::EvaluatedAt {
            value: "2026-10-12 10:00:00".to_string()
        }
    );
    assert!(err.to_string().contains("RFC 3339"));
}

#[test]
fn an_rfc3339_timestamp_with_an_offset_is_accepted() {
    let results = [passing()];
    let mut r = run(&results, GateStatus::Pass);
    r.evaluated_at = "2026-10-12T12:00:00+02:00";
    assert_eq!(build(r).unwrap().evaluated_at, "2026-10-12T12:00:00+02:00");
}

#[test]
fn a_non_conforming_gate_id_refuses() {
    let results = [passing()];
    let mut r = run(&results, GateStatus::Pass);
    r.gate_id = "sha256/abc";
    let err = build(r).unwrap_err();
    assert_eq!(
        err,
        Refusal::GateId {
            value: "sha256/abc".to_string(),
            pattern: GATE_ID_PATTERN,
        }
    );
}

#[test]
fn a_non_conforming_pipeline_refuses() {
    let results = [passing()];
    let mut r = run(&results, GateStatus::Pass);
    r.pipeline = "Bad Pipeline";
    let err = build(r).unwrap_err();
    assert_eq!(
        err,
        Refusal::Pipeline {
            value: "Bad Pipeline".to_string(),
            pattern: PIPELINE_PATTERN,
        }
    );
}

/// The CLI always passes the constant, so this is a guard against the
/// constant ever drifting out of the pattern rather than a live path.
#[test]
fn the_shipped_pipeline_constant_conforms() {
    assert!(matches_pipeline(PLATFORM_PIPELINE));
    assert!(PLATFORM_PIPELINE
        .chars()
        .next()
        .unwrap()
        .is_ascii_lowercase());
}

#[test]
fn an_empty_evidence_key_refuses() {
    let mut r = passing();
    r.evidence.push(EvidenceRef {
        key: String::new(),
        digest: None,
        media_type: None,
        byte_size: None,
        preview: None,
    });
    let err = build(run(&[r], GateStatus::Pass)).unwrap_err();
    assert_eq!(
        err,
        Refusal::EvidenceRef {
            key: String::new(),
            min: 1,
            max: MAX_EVIDENCE_REF_CHARS,
        }
    );
    assert!(err.to_string().contains("1 to 512"));
}

#[test]
fn an_over_length_evidence_key_refuses() {
    let mut r = passing();
    r.evidence.push(EvidenceRef {
        key: "k".repeat(MAX_EVIDENCE_REF_CHARS + 1),
        digest: None,
        media_type: None,
        byte_size: None,
        preview: None,
    });
    let err = build(run(&[r], GateStatus::Pass)).unwrap_err();
    assert!(matches!(err, Refusal::EvidenceRef { .. }), "{err}");
}

#[test]
fn evidence_refs_are_distinct_and_in_first_seen_order() {
    let mut a = passing();
    a.evidence = vec![key("tests:report"), key("shared:artifact")];
    let mut b = result("build", GateStatus::Pass);
    b.evidence = vec![key("shared:artifact"), key("build:report")];
    let results = vec![a, b];
    let doc = build(run(&results, GateStatus::Pass)).unwrap();
    assert_eq!(
        doc.evidence_refs,
        vec![
            "tests:report".to_string(),
            "shared:artifact".to_string(),
            "build:report".to_string()
        ]
    );
}

fn key(k: &str) -> EvidenceRef {
    EvidenceRef {
        key: k.to_string(),
        digest: Some("sha256:abc".to_string()),
        media_type: Some("application/json".to_string()),
        byte_size: Some(42),
        preview: Some("ok".to_string()),
    }
}

/// Redaction is a security control and stays; bounding is a data loss
/// and is refused instead.
#[test]
fn detail_is_redacted_before_it_is_measured() {
    let mut r = result("tests", GateStatus::Fail);
    r.diagnostic = Some("failed with password=hunter2".to_string());
    let doc = build(run(&[r], GateStatus::Fail)).unwrap();
    let detail = doc.checks[0].detail.as_deref().unwrap();
    assert!(!detail.contains("hunter2"), "secret survived: {detail}");
}

// --- ordering ---------------------------------------------------------

/// A substantive refusal must not be masked by a cosmetic one: a run
/// that is both under review and carries a bad id reports the status.
#[test]
fn status_refusals_are_evaluated_before_identity_and_shape() {
    let results = vec![result("L1-configuration", GateStatus::ReviewRequired)];
    let mut r = run(&results, GateStatus::Pass);
    r.gate_id = "bad id";
    r.pipeline = "Bad Pipeline";
    r.evaluated_at = "not a timestamp";
    let err = build(r).unwrap_err();
    assert_eq!(
        err,
        Refusal::CheckStatus {
            status: "REVIEW_REQUIRED",
            ids: "L1-configuration".to_string(),
        }
    );
}

#[test]
fn root_identity_is_evaluated_before_per_check_shape() {
    // Both are wrong here: the check id cannot be represented and the
    // timestamp is not a date-time. Root identity is evaluated first, so
    // the diagnostic is the timestamp — a run whose identity cannot even
    // be read should be told that, not which of its checks is odd.
    let results = vec![result("L1-configuration", GateStatus::Pass)];
    let mut r = run(&results, GateStatus::Pass);
    r.evaluated_at = "not a timestamp";
    let err = build(r).unwrap_err();
    assert!(
        matches!(err, Refusal::EvaluatedAt { .. }),
        "root identity must be reported before the check id: {err}"
    );
}

#[test]
fn the_timestamp_is_checked_before_per_check_shape() {
    // Conforming ids, so the timestamp is the only thing that can fail.
    let results = vec![result("tests", GateStatus::Pass)];
    let mut r = run(&results, GateStatus::Pass);
    r.evaluated_at = "not a timestamp";
    assert!(matches!(build(r).unwrap_err(), Refusal::EvaluatedAt { .. }));
}

/// No refusal ever yields a document. `build` returns `Err`, so there is
/// nothing to print; this asserts the shape of the API rather than a
/// runtime behaviour.
#[test]
fn a_refusal_produces_no_document() {
    let results = vec![result("L1-configuration", GateStatus::Pass)];
    assert!(build(run(&results, GateStatus::Pass)).is_err());
    let results = vec![result("tests", GateStatus::ReviewRequired)];
    assert!(build(run(&results, GateStatus::Pass)).is_err());
    assert!(build(run(&[], GateStatus::Unbound)).is_err());
}

/// A run with no results at all is still a document: an empty
/// `checks` array and a `passed` result are both truthful.
#[test]
fn an_empty_run_builds_an_empty_checks_array() {
    let doc = build(run(&[], GateStatus::Pass)).unwrap();
    assert!(doc.checks.is_empty());
    assert_eq!(doc.result, "passed");
}

#[test]
fn an_absent_subject_ref_is_emitted_as_null() {
    let results = [passing()];
    let mut r = run(&results, GateStatus::Pass);
    r.subject_ref = None;
    let doc = build(r).unwrap();
    assert_eq!(doc.subject_ref, None);
    let json = serde_json::to_string(&doc).unwrap();
    assert!(json.contains("\"subject_ref\":null"), "{json}");
}
