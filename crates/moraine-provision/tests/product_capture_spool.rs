//! Real dual-event spool path: distinct event IDs must both be accepted (not deduped).

use std::fs;

use moraine_provision::product_capture_event_ids;
use moraine_service::{event_already_seen, process_spool_file, write_spooled_payload};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn dual_hook_events_with_distinct_ids_both_spool_and_process() {
    let spool = tempdir().unwrap();
    let processed = spool.path().join("processed");
    let failed = spool.path().join("failed");
    fs::create_dir_all(&processed).unwrap();
    fs::create_dir_all(&failed).unwrap();

    let verification_id = Uuid::new_v4().to_string();
    let (start_id, prompt_id) = product_capture_event_ids(&verification_id);
    assert_ne!(start_id, prompt_id, "mechanical event IDs must differ");

    let session = format!("sess-{verification_id}");
    let project = tempdir().unwrap();
    moraine_core::init_project(Some(project.path())).unwrap();

    let start_payload = json!({
        "schemaVersion": 1,
        "eventId": start_id,
        "kind": "session_start",
        "sessionId": session,
        "project": project.path().display().to_string(),
        "integration": "codex",
        "payload": { "source": "startup" }
    });
    let prompt_payload = json!({
        "schemaVersion": 1,
        "eventId": prompt_id,
        "kind": "user_prompt",
        "sessionId": session,
        "project": project.path().display().to_string(),
        "integration": "codex",
        "payload": {
            "prompt": format!("Moraine self-test verification_id={verification_id}")
        }
    });

    let p1 = write_spooled_payload(spool.path(), &serde_json::to_vec(&start_payload).unwrap())
        .await
        .unwrap();
    let p2 = write_spooled_payload(spool.path(), &serde_json::to_vec(&prompt_payload).unwrap())
        .await
        .unwrap();
    assert_ne!(
        p1, p2,
        "distinct eventIds must produce distinct pending spool files"
    );
    assert!(p1.is_file() && p2.is_file());

    // Same ID would be deduped — prove the negative.
    let dup = write_spooled_payload(spool.path(), &serde_json::to_vec(&start_payload).unwrap())
        .await
        .unwrap();
    // Returns seen marker or same path, not a second pending file with new content.
    assert!(
        dup.to_string_lossy().contains("seen") || dup == p1,
        "duplicate eventId must not create a second pending event: {dup:?}"
    );

    process_spool_file(&p1, &processed, &failed)
        .await
        .expect("process session_start");
    process_spool_file(&p2, &processed, &failed)
        .await
        .expect("process user_prompt");

    assert!(
        event_already_seen(spool.path(), &start_id),
        "start event must be marked seen"
    );
    assert!(
        event_already_seen(spool.path(), &prompt_id),
        "prompt event must be marked seen"
    );

    // Prompt processing should create a session-bound provisional run with verification_id.
    let runs = moraine_core::list_run_summaries(
        project.path(),
        moraine_core::resolve_existing_project(Some(project.path()))
            .unwrap()
            .project_id,
    );
    assert!(
        runs.iter().any(|r| r.objective.contains(&verification_id)),
        "prompt event must create a run objective with verification_id; runs={runs:?}"
    );
}

#[test]
fn product_capture_event_ids_are_distinct() {
    let v = "abc-123";
    let (a, b) = product_capture_event_ids(v);
    assert_ne!(a, b);
    assert!(a.contains(v) && b.contains(v));
    assert!(a.ends_with("session-start"));
    assert!(b.ends_with("user-prompt"));
}

#[tokio::test]
async fn same_event_id_dedupes_second_payload() {
    let spool = tempdir().unwrap();
    let id = "same-id-collision";
    let body = |kind: &str| {
        json!({
            "schemaVersion": 1,
            "eventId": id,
            "kind": kind,
            "sessionId": "s1",
            "project": "/tmp/x",
            "integration": "codex",
            "payload": {}
        })
    };
    let p1 = write_spooled_payload(
        spool.path(),
        &serde_json::to_vec(&body("session_start")).unwrap(),
    )
    .await
    .unwrap();
    let p2 = write_spooled_payload(
        spool.path(),
        &serde_json::to_vec(&body("user_prompt")).unwrap(),
    )
    .await
    .unwrap();
    // Second write must not produce a second pending event file with different content.
    assert!(
        p1 == p2 || p2.to_string_lossy().contains("seen") || !p2.is_file() || p2 == p1,
        "same eventId must dedupe: p1={p1:?} p2={p2:?}"
    );
    // Only one pending event-id file under spool root.
    let pending: Vec<_> = fs::read_dir(spool.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("event-id-") && n.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        pending.len(),
        1,
        "expected single pending file for deduped id, got {pending:?}"
    );
}
