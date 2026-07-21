import { describe, expect, it } from "vitest";
import { resolveSessionConfig, roomIdForPath } from "./yjsSession";

describe("session config", () => {
  it("room id fixture matches Rust", () => {
    expect(roomIdForPath("/tmp/note.md")).toBe("doc_53b4008c");
  });

  it("parses room and sync query", () => {
    expect(resolveSessionConfig("?room=doc_abc").syncUrl).toBe("ws://127.0.0.1:3099");
    expect(resolveSessionConfig("?room=doc_abc").roomId).toBe("doc_abc");
    expect(resolveSessionConfig("?sync=0&room=doc_x").syncUrl).toBeNull();
    expect(resolveSessionConfig("?sync=ws://h:1").syncUrl).toBe("ws://h:1");
    expect(resolveSessionConfig("?sync=1").roomId).toBeNull();
    expect(resolveSessionConfig("").syncUrl).toBeNull();
  });
});
