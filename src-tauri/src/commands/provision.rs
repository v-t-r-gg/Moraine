//! Desktop provisioning control plane — calls moraine-provision only (no CLI scrape).

use std::path::PathBuf;

use moraine_provision::{
    apply_default, health_default, inspect_default, plan as plan_setup, repair_default,
    rollback_default, verify, AgentKind, ApplyOutcome, HealthReport, RepairAction, RepairResult,
    SetupIntent, SetupPlan, SetupReceipt, SystemState, VerificationReport,
};
use serde::Deserialize;

fn map_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupIntentDto {
    pub project: String,
    #[serde(default = "default_agent")]
    pub agent: String,
    #[serde(default = "default_true")]
    pub enable_autostart: bool,
    #[serde(default)]
    pub skip_service: bool,
}

fn default_agent() -> String {
    "codex".into()
}

fn default_true() -> bool {
    true
}

impl SetupIntentDto {
    fn into_intent(self) -> Result<SetupIntent, String> {
        let agent = AgentKind::parse(&self.agent)
            .ok_or_else(|| format!("unsupported agent '{}'", self.agent))?;
        Ok(SetupIntent {
            project: PathBuf::from(self.project),
            agent,
            enable_autostart: self.enable_autostart,
            skip_service: self.skip_service,
        })
    }
}

fn outcome_to_receipt(outcome: ApplyOutcome) -> SetupReceipt {
    outcome.receipt().clone()
}

/// Full system inspection for first-run / settings.
#[tauri::command]
pub fn provision_inspect() -> Result<SystemState, String> {
    inspect_default().map_err(map_err)
}

/// Plan setup for a project + agent without applying.
#[tauri::command]
pub fn provision_plan(intent: SetupIntentDto) -> Result<SetupPlan, String> {
    let intent = intent.into_intent()?;
    let svc = moraine_provision::default_service_manager();
    plan_setup(intent, svc.as_ref()).map_err(map_err)
}

/// Apply by re-planning from intent (legacy). Prefer `provision_apply_plan`.
#[tauri::command]
pub fn provision_apply(intent: SetupIntentDto) -> Result<SetupReceipt, String> {
    let intent = intent.into_intent()?;
    let svc = moraine_provision::default_service_manager();
    let plan = plan_setup(intent, svc.as_ref()).map_err(map_err)?;
    let outcome = apply_default(plan).map_err(map_err)?;
    Ok(outcome_to_receipt(outcome))
}

/// Apply the **exact** user-approved plan (rejects stale state witness).
#[tauri::command]
pub fn provision_apply_plan(plan: SetupPlan) -> Result<SetupReceipt, String> {
    let outcome = apply_default(plan).map_err(map_err)?;
    Ok(outcome_to_receipt(outcome))
}

/// Full apply outcome including RolledBack / RollbackRequired.
#[tauri::command]
pub fn provision_apply_plan_outcome(plan: SetupPlan) -> Result<ApplyOutcome, String> {
    apply_default(plan).map_err(map_err)
}

/// Rollback a failed apply using its receipt.
#[tauri::command]
pub fn provision_rollback(receipt: SetupReceipt) -> Result<(), String> {
    rollback_default(receipt).map_err(map_err)
}

/// Strict self-test / verify for Ready.
#[tauri::command]
pub fn provision_verify(intent: SetupIntentDto) -> Result<VerificationReport, String> {
    let intent = intent.into_intent()?;
    verify(&intent).map_err(map_err)
}

/// Structured health checks with optional Fix actions.
#[tauri::command]
pub fn provision_health(
    project: Option<String>,
    agent: Option<String>,
) -> Result<HealthReport, String> {
    let path = project.as_ref().map(PathBuf::from);
    let kind = agent
        .as_deref()
        .and_then(AgentKind::parse)
        .or(Some(AgentKind::Codex));
    health_default(path.as_deref(), kind).map_err(map_err)
}

/// Execute a repair action from a health check Fix button.
#[tauri::command]
pub fn provision_repair(action: RepairAction) -> Result<RepairResult, String> {
    repair_default(&action).map_err(map_err)
}

/// One-shot enable: plan + apply + self-test.
#[tauri::command]
pub fn provision_enable(intent: SetupIntentDto) -> Result<SetupReceipt, String> {
    let intent = intent.into_intent()?;
    let outcome = moraine_provision::enable_project_default(intent).map_err(map_err)?;
    Ok(outcome_to_receipt(outcome))
}

/// Initialize a project folder (may not already contain .moraine/).
#[tauri::command]
pub fn provision_init_project(path: String) -> Result<serde_json::Value, String> {
    let p = PathBuf::from(path);
    let r = moraine_core::init_project(Some(&p)).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "ok": true,
        "projectRoot": r.project_root.display().to_string(),
        "projectId": r.project_id.to_string(),
        "created": r.created,
    }))
}
