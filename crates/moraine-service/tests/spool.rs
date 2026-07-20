use moraine_service::{process_spool_file, write_spooled_payload};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn dedupe_spool_writes_single_file() {
    let dir = tempdir().unwrap();
    let spool = dir.path().join("spool");
    std::fs::create_dir_all(&spool).unwrap();
    std::fs::create_dir_all(spool.join("processed")).unwrap();
    std::fs::create_dir_all(spool.join("failed")).unwrap();
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
    std::fs::create_dir_all(&spool).unwrap();
    std::fs::create_dir_all(spool.join("processed")).unwrap();
    std::fs::create_dir_all(spool.join("failed")).unwrap();
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
    assert!(p1
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("event-id-"));
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
    std::fs::create_dir_all(&spool).unwrap();
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

    let path2 = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&mk("e2")).unwrap(),
        ))
        .unwrap();
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
    assert_eq!(md_count, 1, "expected one provisional run, got {md_count}");

    // Confirm via semantic start with session id.
    let confirmed = moraine_core::run_start(moraine_core::RunStartRequest {
        objective: "Implement spool recovery with tests".into(),
        idempotency_key: "mcp-1".into(),
        project: Some(project.clone()),
        session_id: Some("codex-sess-42".into()),
    })
    .unwrap();
    assert_eq!(
        std::fs::read_dir(&runs)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .ok()
                    .map(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                    .unwrap_or(false)
            })
            .count(),
        1
    );
    let meta = moraine_core::load_run_meta_readonly(&confirmed.absolute_path)
        .unwrap()
        .unwrap();
    let agent = meta.agent.unwrap();
    assert!(!agent.provisional);
    assert_eq!(agent.capture_coverage, moraine_core::CaptureCoverage::Full);
}

#[test]
fn invalid_mechanical_event_goes_to_failed() {
    let dir = tempdir().unwrap();
    let spool = dir.path().join("spool");
    let processed = spool.join("processed");
    let failed = spool.join("failed");
    std::fs::create_dir_all(&spool).unwrap();
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
    assert!(failed.join(path.file_name().unwrap()).exists());
}
