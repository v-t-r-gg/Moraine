//! Read-only multi-agent capture fidelity reporting.
//!
//! Capability (adapter can emit) is separate from observation (Moraine recorded
//! durable facts). Legacy [`CaptureCoverage`] remains a compact compatibility field.

use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::project::{find_run_by_id, resolve_existing_project};
use super::session::{load_session, namespace_session_key, sessions_dir, SessionRecord};
use super::types::{AgentRunState, CaptureCoverage, EvidenceProvenance};
use crate::error::{Error, Result};

/// Fidelity report schema (independent of run sidecar schema).
pub const CAPTURE_FIDELITY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Supported,
    NotSupported,
    Unknown,
}

impl CapabilitySupport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::NotSupported => "not_supported",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    Observed,
    NotObserved,
    NotSupported,
    Unknown,
}

impl ObservationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::NotObserved => "not_observed",
            Self::NotSupported => "not_supported",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureDimension {
    SessionLifecycle,
    PromptActivity,
    ToolActivity,
    SemanticStart,
    Checkpoints,
    MechanicalEvidence,
    AgentReportedEvidence,
    ReviewFindings,
}

impl CaptureDimension {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionLifecycle => "session_lifecycle",
            Self::PromptActivity => "prompt_activity",
            Self::ToolActivity => "tool_activity",
            Self::SemanticStart => "semantic_start",
            Self::Checkpoints => "checkpoints",
            Self::MechanicalEvidence => "mechanical_evidence",
            Self::AgentReportedEvidence => "agent_reported_evidence",
            Self::ReviewFindings => "review_findings",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SessionLifecycle => "Session lifecycle",
            Self::PromptActivity => "Prompt activity",
            Self::ToolActivity => "Tool activity",
            Self::SemanticStart => "Semantic start",
            Self::Checkpoints => "Checkpoints",
            Self::MechanicalEvidence => "Mechanical evidence",
            Self::AgentReportedEvidence => "Agent-reported evidence",
            Self::ReviewFindings => "Review findings",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDimensionReport {
    pub dimension: CaptureDimension,
    pub capability: CapabilitySupport,
    pub observation: ObservationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_count: Option<u64>,
    /// When false, counts are lower bounds (e.g. migrated historical sessions).
    pub count_is_complete: bool,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureGap {
    pub dimension: CaptureDimension,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureFidelityReport {
    pub schema_version: u32,
    pub run_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration: Option<String>,
    pub legacy_coverage: CaptureCoverage,
    pub provisional: bool,
    pub session_bound: bool,
    pub dimensions: Vec<CaptureDimensionReport>,
    pub gaps: Vec<CaptureGap>,
}

/// What categories of mechanical observation an integration can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureCapabilityProfile {
    pub integration_id: &'static str,
    pub session_lifecycle: CapabilitySupport,
    pub prompt_activity: CapabilitySupport,
    pub tool_activity: CapabilitySupport,
    pub semantic_protocol: CapabilitySupport,
}

/// Authoritative built-in capability table (agent-neutral API, fixed IDs).
pub fn capability_profile_for_integration(integration: &str) -> CaptureCapabilityProfile {
    match integration.trim() {
        "codex" => CaptureCapabilityProfile {
            integration_id: "codex",
            session_lifecycle: CapabilitySupport::Supported,
            prompt_activity: CapabilitySupport::Supported,
            tool_activity: CapabilitySupport::Supported,
            semantic_protocol: CapabilitySupport::Supported,
        },
        "claude-code" => CaptureCapabilityProfile {
            integration_id: "claude-code",
            session_lifecycle: CapabilitySupport::Supported,
            prompt_activity: CapabilitySupport::Supported,
            tool_activity: CapabilitySupport::NotSupported,
            semantic_protocol: CapabilitySupport::Supported,
        },
        _ => CaptureCapabilityProfile {
            integration_id: "unknown",
            session_lifecycle: CapabilitySupport::Unknown,
            prompt_activity: CapabilitySupport::Unknown,
            tool_activity: CapabilitySupport::Unknown,
            semantic_protocol: CapabilitySupport::Unknown,
        },
    }
}

pub fn human_legacy_coverage_label(coverage: CaptureCoverage) -> &'static str {
    match coverage {
        CaptureCoverage::Full => "Mechanical + semantic observed",
        CaptureCoverage::MechanicalOnly => "Mechanical observed",
        CaptureCoverage::SemanticOnly => "Semantic observed",
        CaptureCoverage::Partial => "Partial observation",
        CaptureCoverage::Unknown => "Coverage unknown",
    }
}

fn combine_observation(capability: CapabilitySupport, fact_present: bool) -> ObservationState {
    match (capability, fact_present) {
        (CapabilitySupport::Supported, true) => ObservationState::Observed,
        (CapabilitySupport::Supported, false) => ObservationState::NotObserved,
        (CapabilitySupport::NotSupported, false) => ObservationState::NotSupported,
        (CapabilitySupport::NotSupported, true) => ObservationState::Observed,
        (CapabilitySupport::Unknown, true) => ObservationState::Observed,
        (CapabilitySupport::Unknown, false) => ObservationState::Unknown,
    }
}

fn dim(
    dimension: CaptureDimension,
    capability: CapabilitySupport,
    fact_present: bool,
    exact_count: Option<u64>,
    count_is_complete: bool,
    explanation: String,
) -> CaptureDimensionReport {
    CaptureDimensionReport {
        dimension,
        capability,
        observation: combine_observation(capability, fact_present),
        exact_count,
        count_is_complete,
        explanation,
    }
}

/// Derive legacy coverage from mechanical vs semantic observation (compatibility).
pub fn derive_capture_coverage(
    provisional: bool,
    session: Option<&SessionRecord>,
    checkpoint_count: usize,
) -> CaptureCoverage {
    let mechanical = session.map(|s| s.has_mechanical_hooks()).unwrap_or(false);
    let semantic = !provisional || checkpoint_count > 0;
    match (mechanical, semantic, provisional) {
        (true, true, false) => CaptureCoverage::Full,
        (true, _, true) => CaptureCoverage::MechanicalOnly,
        (true, false, false) => CaptureCoverage::MechanicalOnly,
        (false, true, false) => CaptureCoverage::SemanticOnly,
        (false, true, true) => CaptureCoverage::Unknown,
        (false, false, _) => CaptureCoverage::Unknown,
    }
}

fn count_source(session: &SessionRecord, sources: &[&str]) -> (u64, bool) {
    let mut total = 0u64;
    for s in sources {
        if let Some(n) = session.observation_counts.get(*s) {
            total = total.saturating_add(*n);
        } else if session.sources_seen.iter().any(|x| x == s) {
            // Historical presence without exact count.
            total = total.saturating_add(1);
        }
    }
    (total, session.observation_counts_complete)
}

fn lifecycle_present(session: &SessionRecord) -> bool {
    session.sources_seen.iter().any(|s| {
        matches!(
            s.as_str(),
            "startup" | "resume" | "clear" | "compact" | "session_start" | "stop" | "session_stop"
        )
    })
}

fn prompt_present(session: &SessionRecord) -> bool {
    session.sources_seen.iter().any(|s| s == "user_prompt")
        || session.observation_counts.contains_key("user_prompt")
}

fn tool_sources_present(session: &SessionRecord) -> bool {
    session.sources_seen.iter().any(|s| {
        matches!(
            s.as_str(),
            "command_started"
                | "command_finished"
                | "tool_started"
                | "tool_finished"
                | "artifact_observed"
        ) || s.starts_with("tool_")
            || s.starts_with("command_")
    })
}

fn mechanical_evidence_count(agent: &AgentRunState) -> u64 {
    agent
        .evidence
        .iter()
        .filter(|e| {
            matches!(
                e.provenance,
                EvidenceProvenance::InvocationObserved
                    | EvidenceProvenance::ResultObserved
                    | EvidenceProvenance::MoraineCaptured
            )
        })
        .count() as u64
}

fn agent_reported_evidence_count(agent: &AgentRunState) -> u64 {
    let from_run = agent
        .evidence
        .iter()
        .filter(|e| e.provenance == EvidenceProvenance::AgentReported)
        .count() as u64;
    let from_checkpoints = agent
        .checkpoints
        .iter()
        .flat_map(|c| c.evidence.iter())
        .filter(|e| e.provenance == EvidenceProvenance::AgentReported)
        .count() as u64;
    from_run.saturating_add(from_checkpoints)
}

/// Find session envelopes that list this run (rebuildable; may be empty).
pub fn find_sessions_for_run(project_root: &Path, run_id: Uuid) -> Vec<SessionRecord> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(sessions_dir(project_root)) else {
        return out;
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(rec) = serde_json::from_str::<SessionRecord>(&raw) else {
            continue;
        };
        if rec.run_ids.contains(&run_id)
            || rec.active_provisional_run_id == Some(run_id)
            || rec.capture_active_run_id == Some(run_id)
        {
            out.push(rec);
        }
    }
    out
}

pub fn find_session_for_run(project_root: &Path, run_id: Uuid) -> Option<SessionRecord> {
    find_sessions_for_run(project_root, run_id)
        .into_iter()
        .next()
}

pub fn find_session_for_run_with_agent(
    project_root: &Path,
    run_id: Uuid,
    agent: &AgentRunState,
) -> Option<SessionRecord> {
    let mut candidates = find_sessions_for_run(project_root, run_id);
    if let Some(sid) = agent.session_id.as_deref() {
        // Runs store the durable session_key (not only the external id).
        if let Some(idx) = candidates.iter().position(|s| {
            s.session_key == sid
                || s.external_session_id == sid
                || s.session_key.ends_with(&format!(":{sid}"))
        }) {
            return Some(candidates.swap_remove(idx));
        }
        if let Ok(Some(rec)) = load_session(project_root, sid) {
            return Some(rec);
        }
        let project_id = agent
            .project_id
            .or_else(|| candidates.first().map(|s| s.project_id))
            .unwrap_or(Uuid::nil());
        for integration in ["claude-code", "codex", "unknown"] {
            let key = namespace_session_key(integration, project_id, sid);
            if let Ok(Some(rec)) = load_session(project_root, &key) {
                return Some(rec);
            }
        }
    }
    candidates.sort_by_key(|s| {
        std::cmp::Reverse(
            s.sources_seen
                .len()
                .saturating_add(s.observation_counts.len()),
        )
    });
    candidates.into_iter().next()
}

/// Build a fidelity report from durable project state (read-only).
pub fn capture_fidelity_report(
    project: Option<&Path>,
    run_id: Uuid,
    capability_profile: &CaptureCapabilityProfile,
) -> Result<CaptureFidelityReport> {
    let resolved = resolve_existing_project(project)?;
    let (_md_path, meta) = find_run_by_id(&resolved.project_root, run_id)?;
    let agent = meta
        .agent
        .as_ref()
        .ok_or_else(|| Error::other(format!("run {run_id} is not a protocol run")))?;

    let mut session = find_session_for_run_with_agent(&resolved.project_root, run_id, agent);
    // Prefer reconstructing from the run's bound external session id + project id.
    if session.is_none() {
        if let Some(ext) = agent.session_id.as_deref() {
            for integration in ["claude-code", "codex", "unknown"] {
                let key = namespace_session_key(integration, resolved.project_id, ext);
                if let Ok(Some(rec)) = load_session(&resolved.project_root, &key) {
                    session = Some(rec);
                    break;
                }
            }
        }
    }
    // Prefer session integration; fall back to external-session prefix heuristics.
    let integration = session
        .as_ref()
        .map(|s| s.integration.clone())
        .filter(|s| !s.is_empty() && s != "unknown")
        .or_else(|| {
            agent.session_id.as_ref().and_then(|s| {
                if s.starts_with("claude-code:") {
                    Some("claude-code".into())
                } else if s.starts_with("codex:") {
                    Some("codex".into())
                } else {
                    None
                }
            })
        })
        .or_else(|| {
            if capability_profile.integration_id != "unknown" {
                Some(capability_profile.integration_id.to_string())
            } else {
                None
            }
        });

    let profile = if let Some(ref id) = integration {
        capability_profile_for_integration(id)
    } else {
        *capability_profile
    };

    let session_bound = session.is_some() || agent.session_id.is_some();
    let counts_complete = session
        .as_ref()
        .map(|s| s.observation_counts_complete)
        .unwrap_or(false);

    let lifecycle_fact = session.as_ref().map(lifecycle_present).unwrap_or(false);
    let prompt_fact = session.as_ref().map(prompt_present).unwrap_or(false);
    let tool_fact = session.as_ref().map(tool_sources_present).unwrap_or(false)
        || mechanical_evidence_count(agent) > 0;
    let semantic_start = !agent.provisional;
    let checkpoint_n = agent.checkpoints.len() as u64;
    let mech_ev = mechanical_evidence_count(agent);
    let agent_ev = agent_reported_evidence_count(agent);
    let findings_n = agent.findings.len() as u64;

    let (lifecycle_count, _) = session
        .as_ref()
        .map(|s| {
            count_source(
                s,
                &[
                    "startup",
                    "resume",
                    "clear",
                    "compact",
                    "session_start",
                    "stop",
                    "session_stop",
                ],
            )
        })
        .unwrap_or((0, false));
    let (prompt_count, _) = session
        .as_ref()
        .map(|s| count_source(s, &["user_prompt"]))
        .unwrap_or((0, false));

    let mut dimensions = vec![
        dim(
            CaptureDimension::SessionLifecycle,
            profile.session_lifecycle,
            lifecycle_fact,
            if lifecycle_fact {
                Some(lifecycle_count.max(1))
            } else {
                Some(0)
            },
            counts_complete,
            if lifecycle_fact {
                if counts_complete {
                    "Session lifecycle events were recorded for this run.".to_string()
                } else {
                    "Session lifecycle activity was recorded; exact historical counts are incomplete."
                        .into()
                }
            } else {
                match profile.session_lifecycle {
                    CapabilitySupport::NotSupported => {
                        "This adapter does not emit session lifecycle observations.".to_string()
                    }
                    CapabilitySupport::Unknown => {
                        "Session lifecycle observation state is unknown for this integration."
                            .into()
                    }
                    CapabilitySupport::Supported => {
                        "No session lifecycle observations were recorded for this run.".to_string()
                    }
                }
            },
        ),
        dim(
            CaptureDimension::PromptActivity,
            profile.prompt_activity,
            prompt_fact,
            if prompt_fact {
                Some(prompt_count.max(1))
            } else {
                Some(0)
            },
            counts_complete,
            if prompt_fact {
                "Prompt activity was observed (prompt text may be withheld by privacy defaults)."
                    .into()
            } else {
                match profile.prompt_activity {
                    CapabilitySupport::NotSupported => {
                        "This adapter does not emit prompt activity observations.".to_string()
                    }
                    CapabilitySupport::Unknown => {
                        "Prompt activity observation state is unknown for this integration.".into()
                    }
                    CapabilitySupport::Supported => {
                        "No prompt activity was recorded for this run.".to_string()
                    }
                }
            },
        ),
        dim(
            CaptureDimension::ToolActivity,
            profile.tool_activity,
            tool_fact,
            if tool_fact {
                Some(mech_ev.max(1))
            } else {
                Some(0)
            },
            true,
            if tool_fact {
                "Tool or command activity produced durable observations.".to_string()
            } else {
                match profile.tool_activity {
                    CapabilitySupport::NotSupported => {
                        "Tool activity is not supported by this adapter.".to_string()
                    }
                    CapabilitySupport::Unknown => {
                        "Tool activity observation state is unknown for this integration."
                            .to_string()
                    }
                    CapabilitySupport::Supported => {
                        "No tool or command activity was recorded for this run.".to_string()
                    }
                }
            },
        ),
        dim(
            CaptureDimension::SemanticStart,
            profile.semantic_protocol,
            semantic_start,
            None,
            true,
            if semantic_start {
                "The run is semantically confirmed (not provisional).".to_string()
            } else {
                "The run remains provisional; semantic start was not recorded.".to_string()
            },
        ),
        dim(
            CaptureDimension::Checkpoints,
            profile.semantic_protocol,
            checkpoint_n > 0,
            Some(checkpoint_n),
            true,
            if checkpoint_n > 0 {
                format!("Semantic checkpoints were recorded ({checkpoint_n}).")
            } else {
                "No semantic checkpoints were recorded.".to_string()
            },
        ),
        dim(
            CaptureDimension::MechanicalEvidence,
            CapabilitySupport::Supported,
            mech_ev > 0,
            Some(mech_ev),
            true,
            if mech_ev > 0 {
                format!(
                    "Mechanical evidence with Moraine-observed provenance was recorded ({mech_ev})."
                )
            } else {
                "No mechanical evidence with Moraine-observed provenance was recorded.".to_string()
            },
        ),
        dim(
            CaptureDimension::AgentReportedEvidence,
            CapabilitySupport::Supported,
            agent_ev > 0,
            Some(agent_ev),
            true,
            if agent_ev > 0 {
                format!(
                    "Agent-reported evidence claims were recorded ({agent_ev}); not Moraine-verified execution."
                )
            } else {
                "No agent-reported evidence claims were recorded.".to_string()
            },
        ),
        dim(
            CaptureDimension::ReviewFindings,
            CapabilitySupport::Supported,
            findings_n > 0,
            Some(findings_n),
            true,
            if findings_n > 0 {
                format!("Review findings were recorded ({findings_n}); this is descriptive, not a verdict.")
            } else {
                "No review findings were recorded.".to_string()
            },
        ),
    ];

    // Stabilize dimension order (already fixed).
    let _ = &mut dimensions;

    let mut gaps = Vec::new();
    for d in &dimensions {
        if d.observation == ObservationState::NotObserved
            && d.capability == CapabilitySupport::Supported
        {
            gaps.push(CaptureGap {
                dimension: d.dimension,
                reason: d.explanation.clone(),
            });
        }
    }

    let legacy =
        derive_capture_coverage(agent.provisional, session.as_ref(), checkpoint_n as usize);

    Ok(CaptureFidelityReport {
        schema_version: CAPTURE_FIDELITY_SCHEMA_VERSION,
        run_id,
        integration,
        legacy_coverage: legacy,
        provisional: agent.provisional,
        session_bound,
        dimensions,
        gaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_protocol::session::SessionRecord;
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn empty_session(sources: &[&str]) -> SessionRecord {
        SessionRecord {
            schema_version: 3,
            session_key: "codex:x:y".to_string(),
            external_session_id: "y".to_string(),
            integration: "codex".to_string(),
            project_id: Uuid::nil(),
            project_root: "/tmp".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            active_provisional_run_id: None,
            capture_active_run_id: None,
            run_ids: vec![],
            initial_task: None,
            prompt_context: vec![],
            sources_seen: sources.iter().map(|s| (*s).to_string()).collect(),
            observation_counts: BTreeMap::new(),
            observation_counts_complete: true,
        }
    }

    #[test]
    fn legacy_classification_table() {
        let mechanical = empty_session(&["startup", "user_prompt"]);
        let none = empty_session(&[]);
        assert_eq!(
            derive_capture_coverage(false, Some(&mechanical), 1),
            CaptureCoverage::Full
        );
        assert_eq!(
            derive_capture_coverage(true, Some(&mechanical), 0),
            CaptureCoverage::MechanicalOnly
        );
        assert_eq!(
            derive_capture_coverage(false, Some(&none), 1),
            CaptureCoverage::SemanticOnly
        );
        assert_eq!(
            derive_capture_coverage(false, None, 1),
            CaptureCoverage::SemanticOnly
        );
        assert_eq!(
            derive_capture_coverage(true, None, 0),
            CaptureCoverage::Unknown
        );
        // Non-provisional with no mechanical session is still semantic-only.
        assert_eq!(
            derive_capture_coverage(false, Some(&none), 0),
            CaptureCoverage::SemanticOnly
        );
        // provisional with checkpoint still mechanical_only if hooks present
        assert_eq!(
            derive_capture_coverage(true, Some(&mechanical), 2),
            CaptureCoverage::MechanicalOnly
        );
    }

    #[test]
    fn claude_profile_tool_not_supported() {
        let p = capability_profile_for_integration("claude-code");
        assert_eq!(p.tool_activity, CapabilitySupport::NotSupported);
        assert_eq!(p.session_lifecycle, CapabilitySupport::Supported);
    }

    #[test]
    fn codex_profile_tools_supported() {
        let p = capability_profile_for_integration("codex");
        assert_eq!(p.tool_activity, CapabilitySupport::Supported);
    }

    #[test]
    fn combine_observation_matrix() {
        assert_eq!(
            combine_observation(CapabilitySupport::Supported, true),
            ObservationState::Observed
        );
        assert_eq!(
            combine_observation(CapabilitySupport::Supported, false),
            ObservationState::NotObserved
        );
        assert_eq!(
            combine_observation(CapabilitySupport::NotSupported, false),
            ObservationState::NotSupported
        );
        assert_eq!(
            combine_observation(CapabilitySupport::Unknown, false),
            ObservationState::Unknown
        );
        assert_eq!(
            combine_observation(CapabilitySupport::Unknown, true),
            ObservationState::Observed
        );
    }

    #[test]
    fn full_label_is_not_complete_knowledge() {
        assert_eq!(
            human_legacy_coverage_label(CaptureCoverage::Full),
            "Mechanical + semantic observed"
        );
    }

    #[test]
    fn capture_coverage_serialization_byte_compatible() {
        // Existing serialized values must remain stable.
        for (value, expected) in [
            (CaptureCoverage::Full, "\"full\""),
            (CaptureCoverage::MechanicalOnly, "\"mechanical_only\""),
            (CaptureCoverage::SemanticOnly, "\"semantic_only\""),
            (CaptureCoverage::Partial, "\"partial\""),
            (CaptureCoverage::Unknown, "\"unknown\""),
        ] {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
            let back: CaptureCoverage = serde_json::from_str(expected).unwrap();
            assert_eq!(back, value);
        }
    }

    #[test]
    fn report_serialization_is_deterministic() {
        let report = CaptureFidelityReport {
            schema_version: 1,
            run_id: Uuid::nil(),
            integration: Some("codex".into()),
            legacy_coverage: CaptureCoverage::MechanicalOnly,
            provisional: true,
            session_bound: true,
            dimensions: vec![dim(
                CaptureDimension::SessionLifecycle,
                CapabilitySupport::Supported,
                true,
                Some(1),
                true,
                "Session lifecycle events were recorded for this run.".into(),
            )],
            gaps: vec![],
        };
        let a = serde_json::to_string(&report).unwrap();
        let b = serde_json::to_string(&report).unwrap();
        assert_eq!(a, b);
        let again: CaptureFidelityReport = serde_json::from_str(&a).unwrap();
        assert_eq!(again, report);
    }
}
