//! M5: rebuildable discovery index + nonmutation of run bundles.
use moraine_core::{init_project, run_checkpoint, run_start, CheckpointInput, RunStartRequest};
use moraine_service::{index_revision, list_project_runs, read_index_projects, rebuild_index};
use std::fs;
use tempfile::tempdir;

#[test]
fn rebuild_index_is_monotonic_and_nonmutating() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    let init = init_project(Some(&project)).unwrap();
    let start = run_start(RunStartRequest {
        objective: "Index rebuild nonmutation".into(),
        idempotency_key: "idx-start".into(),
        project: Some(init.project_root.clone()),
        session_id: None,
    })
    .unwrap();
    run_checkpoint(
        Some(&init.project_root),
        start.run_id,
        &start.content_hash,
        "idx-cp",
        CheckpointInput {
            summary: "checkpoint for index".into(),
            actions: vec![],
            rationales: vec![],
            evidence: vec![],
            risks: vec![],
            open_questions: vec![],
        },
    )
    .unwrap();

    let md = start.absolute_path.clone();
    let side = moraine_core::moraine_sidecar_path(&md);
    let before_md = fs::read(&md).unwrap();
    let before_side = fs::read(&side).unwrap();
    let before_hash = moraine_core::content_hash(&String::from_utf8_lossy(&before_md));

    let spool = dir.path().join("spool");
    fs::create_dir_all(&spool).unwrap();
    let out = spool.join("index.json");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(rebuild_index(dir.path().to_path_buf(), out.clone(), 4))
        .unwrap();
    let rev1 = index_revision(&spool);
    assert!(rev1 >= 1, "revision should start at 1 after first rebuild");

    let doc = read_index_projects(&spool).expect("index present");
    let projects = doc
        .get("projects")
        .and_then(|p| p.as_array())
        .expect("projects array");
    assert!(!projects.is_empty());
    assert!(projects.iter().any(|p| {
        p.get("projectId")
            .and_then(|v| v.as_str())
            .map(|id| id == init.project_id.to_string())
            .unwrap_or(false)
    }));

    let runs = list_project_runs(&init.project_root).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].checkpoint_count, 1);
    assert_eq!(runs[0].integrity, "current");

    // Second rebuild bumps revision only (index cache).
    rt.block_on(rebuild_index(dir.path().to_path_buf(), out.clone(), 4))
        .unwrap();
    let rev2 = index_revision(&spool);
    assert!(rev2 > rev1, "revision must be monotonic: {rev1} -> {rev2}");

    assert_eq!(
        fs::read(&md).unwrap(),
        before_md,
        "markdown mutated by rebuild"
    );
    assert_eq!(
        fs::read(&side).unwrap(),
        before_side,
        "sidecar mutated by rebuild"
    );
    let after_hash = moraine_core::content_hash(&String::from_utf8_lossy(&fs::read(&md).unwrap()));
    assert_eq!(before_hash, after_hash);
}

#[test]
fn broken_run_does_not_suppress_healthy() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    let init = init_project(Some(&project)).unwrap();
    let start = run_start(RunStartRequest {
        objective: "Healthy run".into(),
        idempotency_key: "healthy-start".into(),
        project: Some(init.project_root.clone()),
        session_id: None,
    })
    .unwrap();
    let _ = start;

    // Drop a broken markdown+sidecar next to healthy runs.
    let runs_dir = init.project_root.join(".moraine").join("runs");
    let bad = runs_dir.join("broken-run.md");
    fs::write(&bad, "# broken\n").unwrap();
    fs::write(format!("{}.moraine.json", bad.display()), "{not-json").unwrap();

    let list = list_project_runs(&init.project_root).unwrap();
    assert!(
        list.iter()
            .any(|r| r.objective == "Healthy run" && r.integrity == "current"),
        "healthy run missing: {list:?}"
    );
    assert!(
        list.iter().any(|r| r.integrity == "malformed_sidecar"),
        "broken run should surface: {list:?}"
    );
}

#[test]
fn summarize_scale_many_runs() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    let init = init_project(Some(&project)).unwrap();
    for i in 0..40 {
        run_start(RunStartRequest {
            objective: format!("Scale run {i}"),
            idempotency_key: format!("scale-{i}"),
            project: Some(init.project_root.clone()),
            session_id: None,
        })
        .unwrap();
    }
    let t0 = std::time::Instant::now();
    let runs = list_project_runs(&init.project_root).unwrap();
    let elapsed = t0.elapsed();
    assert_eq!(runs.len(), 40);
    assert!(
        elapsed.as_millis() < 5_000,
        "listing 40 runs took too long: {elapsed:?}"
    );
}
