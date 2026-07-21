// @ts-nocheck — Node filesystem APIs for structural source checks (vitest only).
/**
 * Structural checks for the checkpoint findings desktop surface.
 * Full GUI playthrough is out of scope; host commands are covered in Rust tests.
 *
 * Reads shipped source via filesystem at test runtime (Node vitest env).
 */
import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import { describe, expect, it } from "vitest";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

describe("checkpoint findings desktop surface", () => {
  it("ships create-finding form and chronological thread on checkpoint detail", () => {
    const panel = readFileSync(
      join(root, "src/lib/components/CheckpointFindingsPanel.svelte"),
      "utf8",
    );
    expect(panel).toContain("Add finding");
    expect(panel).toContain("createFinding");
    expect(panel).toContain("Finding thread");
    expect(panel).toContain("thread.thread");
    expect(panel).toContain("Human finding");
    expect(panel).toContain("Agent response");
    expect(panel).toContain("Mark addressed");
    expect(panel).toContain("no verdict");
    // No verdict UX controls (word-boundary style checks)
    expect(panel).not.toMatch(/\bapprove\b/i);
    expect(panel).not.toMatch(/\breject\b/i);
    expect(panel).not.toMatch(/pass\/fail/i);
  });

  it("wires real host API bindings (not stubs that skip the ledger)", () => {
    const api = readFileSync(join(root, "src/lib/api.ts"), "utf8");
    expect(api).toContain('invoke("create_finding_cmd"');
    expect(api).toContain('invoke("list_findings_cmd"');
    expect(api).toContain('invoke("get_finding_cmd"');
    expect(api).toContain('invoke("change_finding_state_cmd"');
    expect(api).toContain('invoke("get_run_checkpoints_cmd"');

    const page = readFileSync(join(root, "src/routes/+page.svelte"), "utf8");
    expect(page).toContain("CheckpointFindingsPanel");
    expect(page).toContain("findingsRefreshToken");

    const tauri = readFileSync(join(root, "src-tauri/src/commands/findings.rs"), "utf8");
    expect(tauri).toContain("create_finding_at_path");
    expect(tauri).toContain("change_finding_state_at_path");
    expect(tauri).toContain("get_finding_at_path");
    expect(tauri).toContain("load_run_checkpoints_detail");
  });
});
