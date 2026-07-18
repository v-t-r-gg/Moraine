import { describe, expect, it } from "vitest";
import { collabFromLocation, roomIdForPath } from "./collab";

describe("collab", () => {
  it("room id fixture matches Rust", () => {
    expect(roomIdForPath("/tmp/note.md")).toBe("doc_53b4008c");
  });

  it("parses room and sync query", () => {
    expect(collabFromLocation("?room=doc_abc").syncUrl).toBe("ws://127.0.0.1:3099");
    expect(collabFromLocation("?room=doc_abc").roomId).toBe("doc_abc");
    expect(collabFromLocation("?sync=0&room=doc_x").syncUrl).toBeNull();
    expect(collabFromLocation("?sync=ws://h:1").syncUrl).toBe("ws://h:1");
    expect(collabFromLocation("?sync=1").roomId).toBeNull();
    expect(collabFromLocation("").syncUrl).toBeNull();
  });
});
