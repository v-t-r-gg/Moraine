//! Plan setup operations from a SetupIntent.

use std::path::Path;

use moraine_core::resolve_existing_project;
use uuid::Uuid;

use crate::agent::adapter_for;
use crate::apply::compute_witness;
use crate::error::{ProvisionError, Result};
use crate::service::ServiceManager;
use crate::suite::SuitePaths;
use crate::types::{ProvisionOpKind, ProvisionOperation, ProvisionWarning, SetupIntent, SetupPlan};

pub fn plan(intent: SetupIntent, service: &dyn ServiceManager) -> Result<SetupPlan> {
    let suite = SuitePaths::discover();
    let absolute_cli = suite.absolute_cli();
    if !absolute_cli.is_absolute() {
        return Err(ProvisionError::msg(format!(
            "resolved CLI path is not absolute: {}",
            absolute_cli.display()
        )));
    }

    let mut operations = Vec::new();
    let mut warnings = Vec::new();
    let mut product_summary = Vec::new();

    let initialized = resolve_existing_project(Some(&intent.project)).is_ok();
    if !initialized {
        // Not reversible: we never delete project ledgers on rollback.
        operations.push(op(
            ProvisionOpKind::InitializeProject,
            format!("Create local records for {}", intent.project.display()),
            false,
        ));
        product_summary.push(format!("Initialize “{}”", display_name(&intent.project)));
    } else {
        product_summary.push(format!(
            "Project “{}” is already prepared",
            display_name(&intent.project)
        ));
    }

    let adapter = adapter_for(intent.agent);
    let det = adapter.detect()?;
    if !det.detected {
        warnings.push(ProvisionWarning {
            code: "agent_not_detected".into(),
            message: format!(
                "{} was not found; configuration will still be written for when it is installed",
                adapter.display_name()
            ),
            technical_detail: Some("agent executable not on PATH".into()),
        });
    }
    let _integ_plan = adapter.plan_install(&intent.project, &absolute_cli)?;
    operations.push(op(
        ProvisionOpKind::ConfigureAgent,
        format!("Connect {} for this project", adapter.display_name()),
        true, // reversible via snapshot restore of config files
    ));
    product_summary.push(format!(
        "Connect {} for this project",
        adapter.display_name()
    ));

    let svc_state = service.inspect()?;
    if !intent.skip_service {
        // Install/repair when registration missing or invalid (wrong ExecStart).
        if !svc_state.registration_present || !svc_state.registration_valid || !svc_state.installed
        {
            operations.push(op(
                ProvisionOpKind::InstallService,
                "Enable background capture".into(),
                true,
            ));
            product_summary.push("Enable background capture".into());
        }
        if intent.enable_autostart {
            // Reversible via ServiceManager::disable_autostart.
            operations.push(op(
                ProvisionOpKind::EnableAutostart,
                "Keep capture available after restart".into(),
                true,
            ));
            product_summary.push("Keep capture available after restart".into());
        }
        if !svc_state.running {
            operations.push(op(
                ProvisionOpKind::StartService,
                "Start background capture".into(),
                true,
            ));
            product_summary.push("Start background capture".into());
        }
    } else {
        warnings.push(ProvisionWarning {
            code: "service_skipped".into(),
            message: "Background capture setup was skipped for this plan".into(),
            technical_detail: Some("skip_service=true".into()),
        });
    }

    operations.push(op(
        ProvisionOpKind::SelfTest,
        "Test local capture and verify a run is discoverable".into(),
        false,
    ));
    product_summary.push("Verify capture works end-to-end".into());
    product_summary.push(format!("Keep records inside {}", intent.project.display()));

    let absolute_cli_s = absolute_cli.display().to_string();
    let state_witness = compute_witness(&intent, service, &absolute_cli_s)?;

    Ok(SetupPlan {
        plan_id: Uuid::new_v4(),
        intent,
        operations,
        warnings,
        absolute_cli: absolute_cli_s,
        product_summary,
        state_witness,
    })
}

fn op(kind: ProvisionOpKind, detail: String, reversible: bool) -> ProvisionOperation {
    ProvisionOperation {
        id: kind.id().into(),
        kind,
        product_label: kind.product_label().into(),
        detail,
        reversible,
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string()
}
