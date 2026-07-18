import { describe, expect, it } from "vitest";
import * as Y from "yjs";
import {
  applyAcceptToText,
  commentsMap,
  listComments,
  setResolved,
  upsertComment,
  type CommentRecord,
} from "./comments";

function sample(
  id: string,
  kind: "comment" | "suggestion" = "comment",
  resolved = false,
): CommentRecord {
  return {
    id,
    body: kind === "suggestion" ? "new" : "note",
    author: "A",
    quote: "old",
    createdAt: "2020-01-01T00:00:00.000Z",
    resolved,
    kind,
  };
}

describe("annotations", () => {
  it("lists comments and suggestions", () => {
    const doc = new Y.Doc();
    const map = commentsMap(doc);
    upsertComment(map, sample("1"));
    upsertComment(map, sample("2", "suggestion"));
    const all = listComments(map, false);
    expect(all).toHaveLength(2);
    expect(all.find((c) => c.id === "2")?.kind).toBe("suggestion");
  });

  it("accept replaces quote once", () => {
    expect(applyAcceptToText("hello old world", "old", "new")).toBe("hello new world");
    expect(applyAcceptToText("hello world", "missing", "x")).toBeNull();
  });

  it("resolve suggestion without changing text", () => {
    const doc = new Y.Doc();
    const map = commentsMap(doc);
    upsertComment(map, sample("s1", "suggestion"));
    setResolved(map, "s1", true);
    expect(listComments(map, false)).toHaveLength(0);
    expect(map.get("s1")?.resolved).toBe(true);
  });
});
