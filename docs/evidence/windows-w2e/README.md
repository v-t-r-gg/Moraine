# W2-E supporting artifacts

Populate this directory only with **sanitized** acceptance evidence from a real
Windows 11 standard-user session.

```text
docs/evidence/windows-w2e/
├── environment.json
├── binary-hashes.txt
├── acceptance-summary.json
├── redacted-task-definition.xml
├── redacted-task-acl.txt
├── command-results/
│   ├── preflight/
│   ├── runtime/
│   ├── lifecycle/
│   ├── autostart/
│   └── uninstall/
├── screenshots/
│   ├── onboarding-project-selection.png
│   ├── plan-review.png
│   ├── setup-complete-ready.png
│   └── verification-run.png
└── README.md
```

## Do not commit

* passwords
* complete user SIDs
* personal usernames
* unrelated environment variables
* private project content
* unsanitized Codex prompts
* raw `.moraine` run records by default
* absolute paths containing personal account names (redact first)

## Current state

Artifacts are **not yet collected**. The parent report
[`W2E_WINDOWS_11_STANDARD_USER.md`](../W2E_WINDOWS_11_STANDARD_USER.md) remains
`NOT EXECUTED` until a live session fills this tree and all mandatory gates pass.
