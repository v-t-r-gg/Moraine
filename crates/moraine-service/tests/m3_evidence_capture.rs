use moraine_core::{load_evidence_record, run_start, RunStartRequest};
use moraine_service::{event_already_seen, process_spool_file, write_spooled_payload};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn m3_evidence_capture_full_flow() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    moraine_core::init_project(Some(&project)).unwrap();

    let spool = dir.path().join("spool");
    let processed = spool.join("processed");
    let failed = spool.join("failed");
    fs::create_dir_all(&processed).unwrap();
    fs::create_dir_all(&failed).unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();

    // 1. Session start & prompt -> establishes provisional run (Run 1)
    let session_id = "sess-m3-1";
    let start_event = json!({
        "schemaVersion": 1,
        "eventId": "ev-start-1",
        "kind": "session_start",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {"source": "startup"}
    });
    let p_start = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&start_event).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_start, &processed, &failed))
        .unwrap();

    let prompt_event = json!({
        "schemaVersion": 1,
        "eventId": "ev-prompt-1",
        "kind": "user_prompt",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {"prompt": "Run tests and inspect files"}
    });
    let p_prompt = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&prompt_event).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_prompt, &processed, &failed))
        .unwrap();

    // Get provisional run id
    let session_key = format!("codex:{}:{}", get_project_id(&project), session_id);
    let session_rec = moraine_core::load_session(&project, &session_key)
        .unwrap()
        .unwrap();
    let run1_id = session_rec.capture_active_run_id.unwrap();
    assert_eq!(session_rec.active_provisional_run_id, Some(run1_id));

    // 2. Successful test command (start -> finish pairing)
    let cmd1_start = json!({
        "schemaVersion": 1,
        "eventId": "ev-cmd1-start",
        "kind": "command_started",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {
            "tool": "shell",
            "callId": "call-cmd-1",
            "command": "cargo test -p moraine-core",
            "workingDirectory": project.display().to_string()
        }
    });
    let p_cmd1_s = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&cmd1_start).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_cmd1_s, &processed, &failed))
        .unwrap();

    let cmd1_finish = json!({
        "schemaVersion": 1,
        "eventId": "ev-cmd1-finish",
        "kind": "command_finished",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {
            "tool": "shell",
            "callId": "call-cmd-1",
            "command": "cargo test -p moraine-core",
            "workingDirectory": project.display().to_string(),
            "exitCode": 0,
            "output": "test result: ok. 79 passed; 0 failed"
        }
    });
    let p_cmd1_f = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&cmd1_finish).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_cmd1_f, &processed, &failed))
        .unwrap();

    // 3. Failing test command (exit code 101)
    let cmd2_finish = json!({
        "schemaVersion": 1,
        "eventId": "ev-cmd2-finish",
        "kind": "command_finished",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {
            "tool": "shell",
            "callId": "call-cmd-2",
            "command": "cargo test -p failing-crate --token sk-1234567890123456789012345",
            "workingDirectory": project.display().to_string(),
            "exitCode": 101,
            "output": "error: build failed\nsecret_pass=mysecret123"
        }
    });
    let p_cmd2 = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&cmd2_finish).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_cmd2, &processed, &failed))
        .unwrap();

    // 4. Non-shell tool exercise
    let tool_finish = json!({
        "schemaVersion": 1,
        "eventId": "ev-tool-1",
        "kind": "tool_finished",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {
            "tool": "read_file",
            "callId": "call-tool-1",
            "command": "read_file src/lib.rs",
            "workingDirectory": project.display().to_string(),
            "exitCode": 0,
            "output": "file content ok"
        }
    });
    let p_tool = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&tool_finish).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_tool, &processed, &failed))
        .unwrap();

    // Verify evidence files under .moraine/evidence/<run1_id>/
    let ev_dir = project.join(".moraine/evidence").join(run1_id.to_string());
    assert!(ev_dir.exists());

    let ev_count = fs::read_dir(&ev_dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .and_then(|x| x.to_str())
                == Some("json")
        })
        .count();
    assert_eq!(ev_count, 3, "Run 1 must have 3 evidence records");

    // 5. Secret redaction check
    let redacted_ev_path = fs::read_dir(&ev_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.extension().and_then(|x| x.to_str()) == Some("json")
                && fs::read_to_string(p).unwrap().contains("failing-crate")
        })
        .unwrap();
    let redacted_json = fs::read_to_string(&redacted_ev_path).unwrap();
    assert!(!redacted_json.contains("sk-1234567890123456789012345"));
    assert!(!redacted_json.contains("mysecret123"));
    assert!(redacted_json.contains("[REDACTED]"));

    // 6. Confirm the provisional run via a first explicit run_start, then issue a second
    //    run_start to establish a genuinely new run (Run 2). The first call confirms the
    //    provisional, so it must return run1_id. The second call has no provisional to
    //    absorb, so it creates a new run.
    let confirm_res = run_start(RunStartRequest {
        objective: "Run tests and inspect files".to_string(),
        idempotency_key: "run-1-confirm-key".to_string(),
        project: Some(project.clone()),
        session_id: Some(session_id.to_string()),
    })
    .unwrap();
    // Confirming the provisional must return the same run1_id.
    assert_eq!(
        confirm_res.run_id, run1_id,
        "Confirming provisional must yield run1_id"
    );

    let run2_res = run_start(RunStartRequest {
        objective: "Second semantic run".to_string(),
        idempotency_key: "run-2-key".to_string(),
        project: Some(project.clone()),
        session_id: Some(session_id.to_string()),
    })
    .unwrap();
    let run2_id = run2_res.run_id;
    assert_ne!(
        run1_id, run2_id,
        "Second run_start after provisional confirmed must be a new run"
    );

    let session_rec2 = moraine_core::load_session(&project, &session_key)
        .unwrap()
        .unwrap();
    assert_eq!(session_rec2.capture_active_run_id, Some(run2_id));

    // 7. Evidence in Run 2
    let cmd3_finish = json!({
        "schemaVersion": 1,
        "eventId": "ev-cmd3-finish",
        "kind": "command_finished",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {
            "tool": "shell",
            "callId": "call-cmd-3",
            "command": "cargo check",
            "exitCode": 0,
            "output": "Finished dev profile"
        }
    });
    let p_cmd3 = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&cmd3_finish).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_cmd3, &processed, &failed))
        .unwrap();

    let ev2_dir = project.join(".moraine/evidence").join(run2_id.to_string());
    assert!(ev2_dir.exists());
    let ev2_count = fs::read_dir(&ev2_dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .and_then(|x| x.to_str())
                == Some("json")
        })
        .count();
    assert_eq!(ev2_count, 1, "Run 2 must have 1 evidence record attached");

    // 8. Replay deduplication check
    assert!(event_already_seen(&spool, "ev-cmd3-finish"));
    let replay_path = spool.join("event-id-ev-cmd3-finish.json");
    fs::write(&replay_path, serde_json::to_vec(&cmd3_finish).unwrap()).unwrap();
    rt.block_on(process_spool_file(&replay_path, &processed, &failed))
        .unwrap();

    let ev2_count_after = fs::read_dir(&ev2_dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .and_then(|x| x.to_str())
                == Some("json")
        })
        .count();
    assert_eq!(
        ev2_count_after, 1,
        "Replaying same event must not duplicate evidence"
    );

    // 9. Oversized output truncation check
    let huge_output = "X".repeat(30_000);
    let cmd_huge = json!({
        "schemaVersion": 1,
        "eventId": "ev-huge-finish",
        "kind": "command_finished",
        "sessionId": session_id,
        "project": project.display().to_string(),
        "integration": "codex",
        "payload": {
            "tool": "shell",
            "callId": "call-cmd-huge",
            "command": "cat huge.log",
            "exitCode": 0,
            "output": huge_output
        }
    });
    let p_huge = rt
        .block_on(write_spooled_payload(
            &spool,
            &serde_json::to_vec(&cmd_huge).unwrap(),
        ))
        .unwrap();
    rt.block_on(process_spool_file(&p_huge, &processed, &failed))
        .unwrap();

    let huge_ev = load_evidence_record(
        &project,
        Some(run2_id),
        uuid::Uuid::parse_str("ev-huge-finish").unwrap_or_else(|_| uuid::Uuid::nil()),
    )
    .unwrap()
    .or_else(|| {
        // Find in ev2_dir
        fs::read_dir(&ev2_dir).unwrap().flatten().find_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) == Some("json") {
                let raw = fs::read_to_string(&path).unwrap();
                let rec: moraine_core::EvidenceRecord = serde_json::from_str(&raw).unwrap();
                if rec.call_id.as_deref() == Some("call-cmd-huge") {
                    return Some(rec);
                }
            }
            None
        })
    })
    .unwrap();

    let out_meta = huge_ev.output.unwrap();
    assert!(
        out_meta.truncated,
        "Oversized output must be marked truncated"
    );
    assert_eq!(out_meta.byte_count, 30_000);

    let excerpt_path = project.join(out_meta.excerpt_path.unwrap());
    assert!(excerpt_path.exists());
    let excerpt_bytes = fs::read(&excerpt_path).unwrap();
    assert_eq!(
        excerpt_bytes.len(),
        16_384,
        "Excerpt must be bounded to 16KB"
    );

    // 10. Markdown record check — find run1's .md by run_id (has the 3 evidence records)
    let runs_dir = project.join(".moraine/runs");
    let run1_md_path = {
        let mut found = None;
        for entry in fs::read_dir(&runs_dir).unwrap().flatten() {
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) == Some("md") {
                let sidecar = p.with_extension("md.moraine.json");
                if sidecar.exists() {
                    let raw = fs::read_to_string(&sidecar).unwrap();
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&raw) {
                        if meta["run"]["id"].as_str() == Some(&run1_id.to_string()) {
                            found = Some(p);
                            break;
                        }
                    }
                }
            }
        }
        found.expect("run1 markdown not found")
    };
    let md_content = fs::read_to_string(&run1_md_path).unwrap();
    // Debug: print markdown for diagnosis
    eprintln!("--- run1.md ---\n{md_content}\n---");
    assert!(
        md_content.contains("## Evidence"),
        "run.md must contain ## Evidence section"
    );
    assert!(
        md_content
            .contains("`[moraine_captured]` **shell**: `cargo test -p moraine-core` (exit 0)"),
        "run.md must show the captured command evidence line"
    );
}

fn get_project_id(project_root: &std::path::Path) -> uuid::Uuid {
    moraine_core::resolve_existing_project(Some(project_root))
        .unwrap()
        .project_id
}
