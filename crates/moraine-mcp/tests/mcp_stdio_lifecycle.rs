//! Subprocess STDIO integration against `moraine mcp`.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use tempfile::tempdir;

fn moraine_bin() -> PathBuf {
    // Prefer workspace CLI binary from tests that depend on moraine-cli? This crate
    // doesn't depend on the bin; use relative path after cargo test builds it.
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/moraine"),
        PathBuf::from("target/debug/moraine"),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .expect("build moraine CLI first: cargo build -p moraine-cli")
}

struct McpClient {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpClient {
    fn spawn(project: &std::path::Path) -> Self {
        let mut child = Command::new(moraine_bin())
            .args(["mcp", "--project"])
            .arg(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn moraine mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&msg).unwrap();
        writeln!(self.stdin, "{line}").expect("write request");
        self.stdin.flush().ok();

        // Read until matching id (skip notifications)
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if std::time::Instant::now() > deadline {
                panic!("timeout waiting for response to {method}");
            }
            let mut buf = String::new();
            let n = self.stdout.read_line(&mut buf).expect("read stdout");
            if n == 0 {
                panic!("EOF from MCP server during {method}");
            }
            let v: Value = serde_json::from_str(buf.trim()).unwrap_or_else(|e| {
                panic!("invalid JSON from server: {e}; line={buf}");
            });
            if v.get("id") == Some(&json!(id)) {
                return v;
            }
            // ignore notifications / other
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&msg).unwrap();
        writeln!(self.stdin, "{line}").expect("write notify");
        self.stdin.flush().ok();
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        let resp = self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        );
        resp
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text_payload(call_result: &Value) -> Value {
    // MCP CallToolResult in result.content[0].text as JSON string
    let result = call_result
        .get("result")
        .unwrap_or_else(|| panic!("no result: {call_result}"));
    let is_error = result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let text = result["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content: {result}"));
    let parsed: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }));
    if is_error {
        json!({ "mcpIsError": true, "body": parsed })
    } else {
        parsed
    }
}

#[test]
fn mcp_initialize_tools_and_lifecycle() {
    let dir = tempdir().unwrap();
    let mut c = McpClient::spawn(dir.path());

    let init = c.request(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "moraine-test", "version": "0.0.1" }
        }),
    );
    assert!(init.get("result").is_some(), "{init}");
    let instructions = init["result"]["instructions"].as_str().unwrap_or("");
    assert!(instructions.contains("run_start"));
    assert!(instructions.len() <= 1800);
    let head: String = instructions.chars().take(512).collect();
    assert!(head.contains("run_ready") || head.contains("run_start"));

    c.notify("notifications/initialized", json!({}));

    let tools = c.request("tools/list", json!({}));
    let list = &tools["result"]["tools"];
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        {
            let mut n = names.clone();
            n.sort();
            n
        },
        {
            let mut e = vec![
                "get_finding",
                "list_findings",
                "respond_to_finding",
                "run_checkpoint",
                "run_ready",
                "run_resume",
                "run_show",
                "run_start",
            ];
            e.sort();
            e
        }
    );
    for bad in ["decide", "approved", "run_open"] {
        assert!(!names.iter().any(|n| n.contains(bad)), "{names:?}");
    }
    let tools_bytes = serde_json::to_vec(&tools["result"]).unwrap().len();
    assert!(tools_bytes < 12 * 1024, "tools list {tools_bytes}");

    let start = text_payload(&c.call_tool(
        "run_start",
        json!({
            "objective": "MCP lifecycle test",
            "idempotencyKey": "mcp-life-start"
        }),
    ));
    assert!(start.get("runId").is_some(), "{start}");
    let run_id = start["runId"].as_str().unwrap().to_string();
    let hash = start["contentHash"].as_str().unwrap().to_string();
    let start_bytes = serde_json::to_vec(&start).unwrap().len();
    assert!(start_bytes < 2048, "start size {start_bytes}");

    let cp = text_payload(&c.call_tool(
        "run_checkpoint",
        json!({
            "runId": run_id,
            "expectedHash": hash,
            "idempotencyKey": "mcp-life-cp1",
            "summary": "First checkpoint via MCP",
            "actions": ["called run_checkpoint"],
            "evidence": [{
                "kind": "command_result",
                "label": "unit",
                "command": "cargo test -p moraine-mcp",
                "exitCode": 0
            }]
        }),
    ));
    assert_eq!(cp["state"], "active");
    let hash2 = cp["contentHash"].as_str().unwrap().to_string();

    let show = text_payload(&c.call_tool("run_show", json!({ "runId": run_id })));
    assert_eq!(show["checkpointCount"], 1);
    assert!(show.get("markdown").is_none() || show["markdown"].is_null());
    let show_bytes = serde_json::to_vec(&show).unwrap().len();
    assert!(show_bytes < 4096, "show size {show_bytes}");

    let ready = text_payload(&c.call_tool(
        "run_ready",
        json!({
            "runId": run_id,
            "expectedHash": hash2,
            "idempotencyKey": "mcp-life-ready",
            "summary": "ready via mcp"
        }),
    ));
    assert_eq!(ready["state"], "ready_for_review");
    let hash3 = ready["contentHash"].as_str().unwrap().to_string();

    // Human decision via core CLI path
    let abs = dir.path().join(start["recordPath"].as_str().unwrap());
    let out = std::process::Command::new(moraine_bin())
        .args([
            "decide",
            abs.to_str().unwrap(),
            "--decision",
            "approved",
            "--reviewer",
            "mcp-test",
            "--expected-hash",
            &hash3,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let resume = text_payload(&c.call_tool(
        "run_resume",
        json!({
            "runId": run_id,
            "expectedHash": hash3,
            "idempotencyKey": "mcp-life-resume",
            "reason": "more work"
        }),
    ));
    assert_eq!(resume["state"], "active");
    assert_eq!(resume["reviewState"], "stale");

    // Invalid run id domain error without killing server
    let bad = text_payload(&c.call_tool(
        "run_show",
        json!({ "runId": "00000000-0000-4000-8000-000000000099" }),
    ));
    assert_eq!(bad["mcpIsError"], true);
    assert_eq!(bad["body"]["error"]["code"], "run_not_found");
}

#[test]
fn mcp_findings_list_get_respond_idempotency() {
    use moraine_core::{create_finding, CreateFindingRequest, FindingKind};
    use uuid::Uuid;

    let dir = tempdir().unwrap();
    let mut c = McpClient::spawn(dir.path());
    c.request(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "findings-test", "version": "0" }
        }),
    );
    c.notify("notifications/initialized", json!({}));

    let start = text_payload(&c.call_tool(
        "run_start",
        json!({
            "objective": "findings MCP path",
            "idempotencyKey": "find-mcp-start"
        }),
    ));
    let run_id = start["runId"].as_str().unwrap().to_string();
    let hash = start["contentHash"].as_str().unwrap().to_string();

    let cp = text_payload(&c.call_tool(
        "run_checkpoint",
        json!({
            "runId": run_id,
            "expectedHash": hash,
            "idempotencyKey": "find-mcp-cp",
            "summary": "Checkpoint for finding",
            "actions": ["did work"]
        }),
    ));
    assert!(cp.get("contentHash").is_some(), "{cp}");
    let cp_op_id = cp["opId"].as_str().unwrap().to_string();
    let run_uuid = Uuid::parse_str(&run_id).unwrap();
    let checkpoint_op_id = Uuid::parse_str(&cp_op_id).unwrap();

    // Human create via the real host service (not MCP).
    let created = create_finding(
        Some(dir.path()),
        run_uuid,
        CreateFindingRequest {
            kind: FindingKind::Clarification,
            body: "Please clarify the validation step.".into(),
            checkpoint_op_id,
        },
    )
    .expect("create_finding");
    let finding_id = created.finding_id.to_string();

    let listed = text_payload(&c.call_tool(
        "list_findings",
        json!({ "runId": run_id }),
    ));
    assert_eq!(listed["count"], 1, "{listed}");
    assert_eq!(listed["findings"][0]["findingId"], finding_id);
    assert_eq!(listed["findings"][0]["target"]["checkpointOpId"], cp_op_id);
    assert!(
        listed["findings"][0]["target"]["checkpointSummary"]
            .as_str()
            .unwrap()
            .contains("Checkpoint"),
        "{listed}"
    );

    let got = text_payload(&c.call_tool(
        "get_finding",
        json!({ "runId": run_id, "findingId": finding_id }),
    ));
    assert_eq!(got["findingId"], finding_id);
    assert_eq!(got["targetSnapshot"]["opId"], cp_op_id);
    assert_eq!(got["thread"][0]["itemKind"], "finding");
    assert_eq!(got["thread"][0]["body"], "Please clarify the validation step.");
    assert!(!got["target"]["snapshotHash"].as_str().unwrap().is_empty());

    let resp = text_payload(&c.call_tool(
        "respond_to_finding",
        json!({
            "runId": run_id,
            "findingId": finding_id,
            "body": "Validation was cargo test -p widget.",
            "idempotencyKey": "find-mcp-resp-1"
        }),
    ));
    assert_eq!(resp["idempotentReplay"], false, "{resp}");
    assert!(resp.get("responseId").is_some(), "{resp}");

    let replay = text_payload(&c.call_tool(
        "respond_to_finding",
        json!({
            "runId": run_id,
            "findingId": finding_id,
            "body": "Validation was cargo test -p widget.",
            "idempotencyKey": "find-mcp-resp-1"
        }),
    ));
    assert_eq!(replay["idempotentReplay"], true, "{replay}");
    assert_eq!(replay["responseId"], resp["responseId"]);

    let conflict = text_payload(&c.call_tool(
        "respond_to_finding",
        json!({
            "runId": run_id,
            "findingId": finding_id,
            "body": "Different payload",
            "idempotencyKey": "find-mcp-resp-1"
        }),
    ));
    assert_eq!(conflict["mcpIsError"], true, "{conflict}");
    assert_eq!(
        conflict["body"]["error"]["code"],
        "idempotency_conflict",
        "{conflict}"
    );

    let got2 = text_payload(&c.call_tool(
        "get_finding",
        json!({ "runId": run_id, "findingId": finding_id }),
    ));
    assert_eq!(got2["thread"].as_array().unwrap().len(), 2);
    assert_eq!(got2["thread"][1]["itemKind"], "response");
    assert_eq!(
        got2["thread"][1]["body"],
        "Validation was cargo test -p widget."
    );

    // Authorization/integrity: wrong finding id
    let missing = text_payload(&c.call_tool(
        "get_finding",
        json!({
            "runId": run_id,
            "findingId": "00000000-0000-4000-8000-000000000099"
        }),
    ));
    assert_eq!(missing["mcpIsError"], true);
    assert_eq!(missing["body"]["error"]["code"], "finding_not_found");
}

#[test]
fn mcp_idempotent_start_and_revision_conflict() {
    let dir = tempdir().unwrap();
    let mut c = McpClient::spawn(dir.path());
    c.request(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "t", "version": "0" }
        }),
    );
    c.notify("notifications/initialized", json!({}));

    let a = text_payload(&c.call_tool(
        "run_start",
        json!({ "objective": "same", "idempotencyKey": "once" }),
    ));
    let b = text_payload(&c.call_tool(
        "run_start",
        json!({ "objective": "same", "idempotencyKey": "once" }),
    ));
    assert_eq!(a["runId"], b["runId"]);
    assert_eq!(b["idempotentReplay"], true);

    let run_id = a["runId"].as_str().unwrap();
    let hash = a["contentHash"].as_str().unwrap();

    // Concurrent-ish sequential: first checkpoint wins hash; second with stale hash fails
    let ok = text_payload(&c.call_tool(
        "run_checkpoint",
        json!({
            "runId": run_id,
            "expectedHash": hash,
            "idempotencyKey": "cp-a",
            "summary": "A"
        }),
    ));
    assert!(ok.get("contentHash").is_some(), "{ok}");
    let stale = text_payload(&c.call_tool(
        "run_checkpoint",
        json!({
            "runId": run_id,
            "expectedHash": hash,
            "idempotencyKey": "cp-b",
            "summary": "B"
        }),
    ));
    assert_eq!(stale["mcpIsError"], true);
    assert_eq!(stale["body"]["error"]["code"], "revision_conflict");
}
