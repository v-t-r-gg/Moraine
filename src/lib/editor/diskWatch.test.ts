import { describe, expect, it } from "vitest";
import { classifyDiskEvent, isMarkdownSidecarPath } from "./diskWatch";

const baseEv = {
  path: "/tmp/run.md",
  change: "modify",
  documentId: "doc-1",
};

describe("diskWatch classification", () => {
  it("ignores sidecar paths", () => {
    expect(isMarkdownSidecarPath("/tmp/run.md.moraine.json")).toBe(true);
    expect(
      classifyDiskEvent({
        event: { ...baseEv, path: "/tmp/run.md.moraine.json", contentChanged: true },
        openDocumentId: "doc-1",
        knownPersistedHash: "aaa",
        lastHandledExternalHash: null,
        dirty: false,
        saving: false,
      }),
    ).toBe("ignore_sidecar");
  });

  it("ignores equal hash (own save / no-op)", () => {
    expect(
      classifyDiskEvent({
        event: {
          ...baseEv,
          diskContentHash: "hash1",
          knownContentHash: "hash1",
          contentChanged: false,
        },
        openDocumentId: "doc-1",
        knownPersistedHash: "hash1",
        lastHandledExternalHash: null,
        dirty: false,
        saving: false,
      }),
    ).toBe("ignore_same_hash");
  });

  it("ignores duplicate external hash", () => {
    expect(
      classifyDiskEvent({
        event: { ...baseEv, diskContentHash: "ext", contentChanged: true },
        openDocumentId: "doc-1",
        knownPersistedHash: "old",
        lastHandledExternalHash: "ext",
        dirty: false,
        saving: false,
      }),
    ).toBe("ignore_duplicate");
  });

  it("defers while saving", () => {
    expect(
      classifyDiskEvent({
        event: { ...baseEv, diskContentHash: "new", contentChanged: true },
        openDocumentId: "doc-1",
        knownPersistedHash: "old",
        lastHandledExternalHash: null,
        dirty: false,
        saving: true,
      }),
    ).toBe("ignore_while_saving");
  });

  it("clean external edit", () => {
    expect(
      classifyDiskEvent({
        event: { ...baseEv, diskContentHash: "new", contentChanged: true },
        openDocumentId: "doc-1",
        knownPersistedHash: "old",
        lastHandledExternalHash: null,
        dirty: false,
        saving: false,
      }),
    ).toBe("external_clean");
  });

  it("dirty external edit", () => {
    expect(
      classifyDiskEvent({
        event: { ...baseEv, diskContentHash: "new", contentChanged: true },
        openDocumentId: "doc-1",
        knownPersistedHash: "old",
        lastHandledExternalHash: null,
        dirty: true,
        saving: false,
      }),
    ).toBe("external_dirty");
  });

  it("ignores other document ids", () => {
    expect(
      classifyDiskEvent({
        event: { ...baseEv, documentId: "other", diskContentHash: "x", contentChanged: true },
        openDocumentId: "doc-1",
        knownPersistedHash: "y",
        lastHandledExternalHash: null,
        dirty: false,
        saving: false,
      }),
    ).toBe("ignore_same_hash");
  });
});
