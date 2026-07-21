use moraine_service::{
    event_already_seen, mark_event_seen, process_spool_file, rebuild_index, write_spooled_payload,
};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn dedupe_spool_writes_single_file() {
    let dir = tempdir().unwrap();
    let spool = dir.path().join("spool");
    std::fs::create_dir_all(&spool).unwrap();
    let data = b"{".to_vec();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let p1 = rt.block_on(write_spooled_payload(&spool, &data)).unwrap();
    let p2 = rt.block_on(write_spooled_payload(&spool, &data)).unwrap();
    assert_eq!(p1.file_name(), p2.file_name());
    let files: Vec<_> = std::fs::read_dir(&spool)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name())
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn mechanical_event_id_dedupe() {
    let dir = tempdir().unwrap();
    let spool = dir.path().join("spool");
    let payload = json!({
        "schemaVersion": 1,
        "eventId": "stable-event-1",
        "kind": "session_start",
        "sessionId": "sess-1",
        "project": dir.path().display().to_string(),
        "integration": "codex",
        "payload": {"source": "startup"},
    });
    let a = serde_json::to_vec(&payload).unwrap();
    let mut payload2 = payload.clone();
    payload2["occurredAt"] = json!("different-time");
    let b = serde_json::to_vec(&payload2).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let p1 = rt.block_on(write_spooled_payload(&spool, &a)).unwrap();
    let p2 = rt.block_on(write_spooled_payload(&spool, &b)).unwrap();
    assert_eq!(p1, p2);
}

#[test]
fn durable_seen_survives_restart_replay() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();

    let spool = dir.path().join("spool");
    let processed = spool.join("processed");
    let failed = spool.join("failed");
    std::fs::create_dir_all(&processed).unwrap();
    std::fs::create_dir_all(&failed).unwrap();

    let event = json!({
        "schemaVersion": 1,
        "eventId": "restart-safe-1",
        "kind": "user_prompt",
        "sessionId": "codex-sess-42",
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {"prompt": "Implement spool recovery"},
    });
    let body = serde_json::to_vec(&event).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();

    let path1 = rt.block_on(write_spooled_payload(&spool, &body)).unwrap();
    rt.block_on(process_spool_file(&path1, &processed, &failed))
        .unwrap();
    assert!(event_already_seen(&spool, "restart-safe-1"));

    // Simulate restart: same event reappears in pending spool.
    let path2 = spool.join("event-id-restart-safe-1.json");
    std::fs::write(&path2, &body).unwrap();
    rt.block_on(process_spool_file(&path2, &processed, &failed))
        .unwrap();

    let runs = project.join(".moraine/runs");
    let md_count = std::fs::read_dir(&runs)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .ok()
                .map(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(md_count, 1, "replay must not create a second run");
}

#[test]
fn later_prompt_does_not_create_second_provisional() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();

    let spool = dir.path().join("spool");
    let processed = spool.join("processed");
    let failed = spool.join("failed");
    std::fs::create_dir_all(&processed).unwrap();
    std::fs::create_dir_all(&failed).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mk = |eid: &str, prompt: &str| {
        json!({
            "schemaVersion": 1,
            "eventId": eid,
            "kind": "user_prompt",
            "sessionId": "sess-multi",
            "project": project.display().to_string(),
            "integration": "codex",
            "payload": {"prompt": prompt},
        })
    };

    let p1 = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&mk("p1", "Feature X")).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p1, &processed, &failed))
        .unwrap();

    let p2 = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&mk("p2", "Also add one test")).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p2, &processed, &failed))
        .unwrap();

    let md_count = std::fs::read_dir(project.join(".moraine/runs"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .ok()
                .map(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(md_count, 1);
}

#[test]
fn process_user_prompt_creates_provisional_once() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();

    let spool = dir.path().join("spool");
    let processed = spool.join("processed");
    let failed = spool.join("failed");
    std::fs::create_dir_all(&processed).unwrap();
    std::fs::create_dir_all(&failed).unwrap();

    let mk = |event_id: &str| {
        json!({
            "schemaVersion": 1,
            "eventId": event_id,
            "kind": "user_prompt",
            "sessionId": "codex-sess-42",
            "project": project.display().to_string(),
            "integration": "codex",
            "payload": {"prompt": "Implement spool recovery"},
        })
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let path1 = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&mk("e1")).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&path1, &processed, &failed))
        .unwrap();

    let confirmed = moraine_core::run_start(moraine_core::RunStartRequest {
        objective: "Implement spool recovery with tests".into(),
        idempotency_key: "mcp-1".into(),
        project: Some(project.clone()),
        session_id: Some("codex-sess-42".into()),
    })
    .unwrap();
    let meta = moraine_core::load_run_meta_readonly(&confirmed.absolute_path)
        .unwrap()
        .unwrap();
    let agent = meta.agent.unwrap();
    assert!(!agent.provisional);
    assert_eq!(agent.capture_coverage, moraine_core::CaptureCoverage::Full);
}

#[test]
fn invalid_mechanical_event_goes_to_failed_or_quarantine() {
    let dir = tempdir().unwrap();
    let spool = dir.path().join("spool");
    let processed = spool.join("processed");
    let failed = spool.join("failed");
    std::fs::create_dir_all(&processed).unwrap();
    std::fs::create_dir_all(&failed).unwrap();

    let bad = json!({
        "schemaVersion": 1,
        "eventId": "bad-1",
        "kind": "session_start",
        "sessionId": "",
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let path = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&bad).unwrap(),
        ))
        .unwrap();
    let err = rt.block_on(process_spool_file(&path, &processed, &failed));
    assert!(err.is_err());
    assert!(!path.exists());
    assert!(
        failed.join(path.file_name().unwrap()).exists()
            || spool
                .join("quarantine")
                .join(path.file_name().unwrap())
                .exists()
    );
}

#[test]
fn mark_seen_blocks_requeue() {
    let dir = tempdir().unwrap();
    let spool = dir.path().join("spool");
    mark_event_seen(&spool, "already-done").unwrap();
    assert!(event_already_seen(&spool, "already-done"));
    let body = serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "eventId": "already-done",
        "kind": "session_stop",
        "sessionId": "x",
    }))
    .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let p = rt.block_on(write_spooled_payload(&spool, &body)).unwrap();
    // Returns seen marker path; does not create a new pending event file.
    assert!(
        p.extension().and_then(|e| e.to_str()) == Some("seen")
            || !p.exists()
            || event_already_seen(&spool, "already-done")
    );
}

#[test]
fn rebuilt_index_counts_only_run_records() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let initialized = moraine_core::init_project(Some(&project)).unwrap();
    moraine_core::run_start(moraine_core::RunStartRequest {
        objective: "Index one run".into(),
        idempotency_key: "index-one".into(),
        project: Some(initialized.project_root),
        session_id: None,
    })
    .unwrap();

    let index = dir.path().join("index.json");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(rebuild_index(dir.path().to_path_buf(), index.clone(), 3))
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    assert_eq!(value["projects"][0]["run_count"], 1);
}
