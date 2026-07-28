import { describe, expect, it } from "vitest";
import fixture from "./provision.contract.fixture.json";
import type { ApplyOutcomeDto } from "./provision";

function checkedOutcome(value: typeof fixture): ApplyOutcomeDto {
  expect(value.outcome).toBe("rollbackRequired");
  expect(value.receipt.snapshots.map((snapshot) => snapshot.kind)).toEqual([
    "existing",
    "absent",
  ]);
  expect(value.receipt.servicePrestate.registration.originalHash).toBe("def456");
  expect(value.receipt.transactionEnabledAutostart).toBe(true);
  expect(value.receipt.transactionStartedService).toBe(true);
  expect(value.receipt.transactionWroteUnit).toBe(true);
  expect(value.receipt.transactionInitializedProject).toBe(true);
  expect(value.receipt.retainedChanges).toHaveLength(1);
  return value as ApplyOutcomeDto;
}

describe("Rust provisioning JSON contract", () => {
  it("preserves rollback state required by the desktop", () => {
    const outcome = checkedOutcome(fixture);
    expect(outcome.outcome).toBe("rollbackRequired");
    if (outcome.outcome === "rollbackRequired") {
      expect(outcome.originalError).toBe("install failed");
      expect(outcome.rollbackError).toBe("prior service restart failed");
    }
  });
});
