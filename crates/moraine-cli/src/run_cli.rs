//! Agent-run protocol CLI surface (`project` / `run` subcommands).

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::Subcommand;
use moraine_core::{
    find_run_by_id, init_project, resolve_existing_project, run_checkpoint, run_ready, run_resume,
    run_show, run_start, CheckpointInput, Error as CoreError, RunShowOptions, RunStartRequest,
};
use serde_json::json;
use uuid::Uuid;

use crate::relay::launch_desktop;

const EXIT_OK: i32 = 0;
const EXIT_ERR: i32 = 1;
const EXIT_NOT_FOUND: i32 = 2;

#[derive(Debug, Subcommand)]
pub enum ProjectCmd {
    /// Create or discover a Moraine project under `.moraine` (idempotent)
    Init {
        /// Project path (default: current directory / Git root)
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum RunCmd {
    /// Start a new agent run (auto-creates project + record path)
    Start {
        #[arg(long)]
        objective: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Compact resume/status packet for a run
    Show {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        include_markdown: bool,
        #[arg(long)]
        json: bool,
    },
    /// Append a structured checkpoint (JSON file or `-` for stdin)
    Checkpoint {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        expected_hash: String,
        #[arg(long)]
        idempotency_key: String,
        /// Path to JSON checkpoint payload, or `-` for stdin
        #[arg(long)]
        input: String,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Mark run ready for human review (not human approval)
    Ready {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        expected_hash: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Return a ready run to active work
    Resume {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        expected_hash: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Open a run in the desktop app by run id
    Open {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

pub fn dispatch_project(cmd: ProjectCmd) -> Result<i32> {
    match cmd {
        ProjectCmd::Init { path, json } => {
            // Project commands always use JSON envelope when --json; default json true for agents
            let result = match init_project(path.as_deref()) {
                Ok(r) => r,
                Err(e) => return Ok(emit_core(json, &e)),
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "project": {
                            "id": result.project_id.to_string(),
                            "root": result.project_root,
                            "moraineDir": result.moraine_dir,
                            "runsDir": result.runs_dir,
                            "created": result.created,
                        }
                    }))?
                );
            } else {
                println!(
                    "project {} root={} created={}",
                    result.project_id,
                    result.project_root.display(),
                    result.created
                );
            }
            Ok(EXIT_OK)
        }
    }
}

pub fn dispatch_run(cmd: RunCmd) -> Result<i32> {
    match cmd {
        RunCmd::Start {
            objective,
            idempotency_key,
            project,
            json,
        } => {
            if !json {
                eprintln!("hint: prefer --json for agent automation");
            }
            match run_start(RunStartRequest {
                objective,
                idempotency_key,
                project,
            }) {
                Ok(r) => {
                    emit_ok(
                        json,
                        json!({
                            "ok": true,
                            "run": {
                                "id": r.run_id.to_string(),
                                "state": r.state.as_str(),
                                "recordPath": r.record_path,
                                "contentHash": r.content_hash,
                                "recordRevision": r.record_revision,
                                "projectId": r.project_id.map(|u| u.to_string()),
                                "idempotentReplay": r.idempotent_replay,
                            },
                            "git": r.git,
                        }),
                    )?;
                    Ok(EXIT_OK)
                }
                Err(e) => Ok(emit_core(json, &e)),
            }
        }
        RunCmd::Show {
            run_id,
            project,
            include_markdown,
            json,
        } => {
            let id = match parse_uuid(json, &run_id) {
                Ok(u) => u,
                Err(c) => return Ok(c),
            };
            match run_show(
                project.as_deref(),
                id,
                RunShowOptions {
                    include_markdown,
                    ..Default::default()
                },
            ) {
                Ok(p) => {
                    emit_ok(json, json!({ "ok": true, "run": p }))?;
                    Ok(EXIT_OK)
                }
                Err(e) => Ok(emit_core(json, &e)),
            }
        }
        RunCmd::Checkpoint {
            run_id,
            expected_hash,
            idempotency_key,
            input,
            project,
            json,
        } => {
            let id = match parse_uuid(json, &run_id) {
                Ok(u) => u,
                Err(c) => return Ok(c),
            };
            let body = match read_input(&input) {
                Ok(b) => b,
                Err(e) => {
                    return Ok(emit_protocol(
                        json,
                        "invalid_checkpoint",
                        &format!("could not read checkpoint input: {e}"),
                        json!({ "input": input }),
                    ));
                }
            };
            let payload: CheckpointInput = match serde_json::from_str(&body) {
                Ok(p) => p,
                Err(e) => {
                    return Ok(emit_protocol(
                        json,
                        "invalid_checkpoint",
                        &format!("checkpoint JSON parse error: {e}"),
                        json!({}),
                    ));
                }
            };
            match run_checkpoint(
                project.as_deref(),
                id,
                &expected_hash,
                &idempotency_key,
                payload,
            ) {
                Ok(r) => {
                    emit_ok(
                        json,
                        json!({
                            "ok": true,
                            "run": {
                                "id": r.run_id.to_string(),
                                "state": r.state.as_str(),
                                "recordPath": r.record_path,
                                "contentHash": r.content_hash,
                                "recordRevision": r.record_revision,
                                "opId": r.op_id.map(|u| u.to_string()),
                                "idempotentReplay": r.idempotent_replay,
                                "reviewState": r.review_state,
                                "decisionCurrent": r.decision_current,
                            },
                            "git": r.git,
                        }),
                    )?;
                    Ok(EXIT_OK)
                }
                Err(e) => Ok(emit_core(json, &e)),
            }
        }
        RunCmd::Ready {
            run_id,
            expected_hash,
            idempotency_key,
            summary,
            project,
            json,
        } => {
            let id = match parse_uuid(json, &run_id) {
                Ok(u) => u,
                Err(c) => return Ok(c),
            };
            match run_ready(
                project.as_deref(),
                id,
                &expected_hash,
                &idempotency_key,
                summary,
            ) {
                Ok(r) => {
                    emit_ok(
                        json,
                        json!({
                            "ok": true,
                            "run": {
                                "id": r.run_id.to_string(),
                                "state": r.state.as_str(),
                                "recordPath": r.record_path,
                                "contentHash": r.content_hash,
                                "recordRevision": r.record_revision,
                                "opId": r.op_id.map(|u| u.to_string()),
                                "idempotentReplay": r.idempotent_replay,
                                "reviewState": r.review_state,
                                "decisionCurrent": r.decision_current,
                            },
                            "git": r.git,
                        }),
                    )?;
                    Ok(EXIT_OK)
                }
                Err(e) => Ok(emit_core(json, &e)),
            }
        }
        RunCmd::Resume {
            run_id,
            expected_hash,
            idempotency_key,
            reason,
            project,
            json,
        } => {
            let id = match parse_uuid(json, &run_id) {
                Ok(u) => u,
                Err(c) => return Ok(c),
            };
            match run_resume(
                project.as_deref(),
                id,
                &expected_hash,
                &idempotency_key,
                reason,
            ) {
                Ok(r) => {
                    emit_ok(
                        json,
                        json!({
                            "ok": true,
                            "run": {
                                "id": r.run_id.to_string(),
                                "state": r.state.as_str(),
                                "recordPath": r.record_path,
                                "contentHash": r.content_hash,
                                "recordRevision": r.record_revision,
                                "opId": r.op_id.map(|u| u.to_string()),
                                "idempotentReplay": r.idempotent_replay,
                                "reviewState": r.review_state,
                                "decisionCurrent": r.decision_current,
                            },
                            "git": r.git,
                        }),
                    )?;
                    Ok(EXIT_OK)
                }
                Err(e) => Ok(emit_core(json, &e)),
            }
        }
        RunCmd::Open {
            run_id,
            project,
            json,
        } => {
            let id = match parse_uuid(json, &run_id) {
                Ok(u) => u,
                Err(c) => return Ok(c),
            };
            let project = match resolve_existing_project(project.as_deref()) {
                Ok(p) => p,
                Err(e) => return Ok(emit_core(json, &e)),
            };
            let (path, _) = match find_run_by_id(&project.project_root, id) {
                Ok(v) => v,
                Err(e) => return Ok(emit_core(json, &e)),
            };
            let launched = launch_desktop(&path).unwrap_or(false);
            if !launched {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "ok": false,
                            "error": {
                                "code": "desktop_launch_failed",
                                "message": "Could not launch the Moraine desktop app",
                                "details": {
                                    "recordPath": path,
                                    "runId": id.to_string(),
                                    "hint": format!("MORAINE_OPEN={} npm run tauri:dev", path.display()),
                                }
                            },
                            "code": EXIT_ERR,
                        }))?
                    );
                } else {
                    eprintln!(
                        "could not launch desktop app; open manually:\n  MORAINE_OPEN={} npm run tauri:dev",
                        path.display()
                    );
                }
                return Ok(EXIT_ERR);
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "run": {
                            "id": id.to_string(),
                            "path": path,
                            "launched": true,
                        }
                    }))?
                );
            } else {
                println!("opened {}", path.display());
            }
            Ok(EXIT_OK)
        }
    }
}

fn parse_uuid(json_mode: bool, s: &str) -> std::result::Result<Uuid, i32> {
    match Uuid::from_str(s.trim()) {
        Ok(u) => Ok(u),
        Err(_) => {
            let code = emit_protocol(
                json_mode,
                "run_not_found",
                &format!("invalid run id: {s}"),
                json!({ "runId": s }),
            );
            Err(code)
        }
    }
}

fn read_input(spec: &str) -> Result<String> {
    if spec == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(std::fs::read_to_string(spec).with_context(|| format!("read input {spec}"))?)
    }
}

fn emit_ok(_json_mode: bool, value: serde_json::Value) -> Result<()> {
    // Protocol responses are always machine-readable JSON on stdout.
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn emit_core(json_mode: bool, err: &CoreError) -> i32 {
    let code = match err {
        CoreError::NotFound(_)
        | CoreError::RunNotFound { .. }
        | CoreError::ProjectNotFound { .. } => EXIT_NOT_FOUND,
        _ => EXIT_ERR,
    };
    let details = err.to_json_value();
    let code_str = details
        .get("code")
        .and_then(|c| c.as_str())
        .unwrap_or_else(|| err.protocol_code());
    if json_mode {
        let _ = writeln!(
            io::stdout(),
            "{}",
            json!({
                "ok": false,
                "error": {
                    "code": code_str,
                    "message": details.get("message").cloned().unwrap_or(json!(err.to_string())),
                    "details": details,
                },
                "code": code,
            })
        );
    } else {
        eprintln!("error: {err}");
    }
    code
}

fn emit_protocol(json_mode: bool, code: &str, message: &str, details: serde_json::Value) -> i32 {
    if json_mode {
        let _ = writeln!(
            io::stdout(),
            "{}",
            json!({
                "ok": false,
                "error": {
                    "code": code,
                    "message": message,
                    "details": details,
                },
                "code": EXIT_ERR,
            })
        );
    } else {
        eprintln!("error: {message}");
    }
    EXIT_ERR
}
