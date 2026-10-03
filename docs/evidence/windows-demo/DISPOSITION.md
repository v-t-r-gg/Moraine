# Windows demo disposition

**Disposition: not_executed**

This note is the evidence record for the Windows follow-along demo. It is
not W2-E acceptance and it does not change README Product Ready, ROADMAP, or
the platform table.

| Field | Value |
|---|---|
| Milestone | Windows demo suite (staged). W2-E remains separate. |
| Disposition | `not_executed` |
| Public claims promoted | no |
| Windows Product Ready | No |
| Installation supported | no |
| Host | Script and docs were prepared on Linux. No Windows 11 x86-64 standard-user session ran. |
| Archive produced | no |
| Agent run | none |
| Run id | none |
| Tool activity fidelity | not observed |

Draft PR #26 describes the W2-E evidence shape. This session did not fill
that package. Hosted `windows-latest` output belongs in the CI log. It is
not this file.

## Gates

Status is `not_executed` unless a live session recorded it. None of these
ran here.

| Gate | Status |
|---|---|
| environment_recorded | not_executed |
| account_a_standard_user | not_executed |
| suite_staged_no_elevation | not_executed |
| preflight_cli | not_executed |
| native_desktop_onboarding | not_executed |
| task_scheduler_registration | not_executed |
| login_autostart | not_executed |
| named_pipe_capture | not_executed |
| real_codex_activity | not_executed |
| capture_without_desktop | not_executed |
| demand_lifecycle | not_executed |
| no_service_console_window | not_executed |
| cross_account_pipe_denial | not_executed |
| graphical_health_repair | not_executed |
| graphical_rollback | not_executed |
| uninstall_preserves_ledgers | not_executed |
| evidence_sanitized | not_executed |
| demo_zip_built_on_windows_11 | not_executed |
| second_window_stage_without_elevation | not_executed |
| doctor_coherent_suite_without_install_sh | not_executed |
| real_agent_run_markdown_and_sidecar | not_executed |
| moraine_open_shows_run | not_executed |
| service_stop_does_not_fail_agent | not_executed |

Account B pipe denial was not run. Do not infer it from the owner-DACL unit
test.

## Why this is not a pass

A green Linux build is not a Windows demo. No zip SHA-256 was produced. No
`moraine doctor` ids were observed on Windows. No UAC observation was made.
No agent run id exists.
