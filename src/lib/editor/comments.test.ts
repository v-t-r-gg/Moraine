import { describe, expect, it } from "vitest";
import * as Y from "yjs";
import {
  commentsMap,
  listComments,
  setResolved,
  upsertComment,
  type CommentRecord,
} from "./comments";

function sample(id: string, resolved = false): CommentRecord {
  return {
    id,
    body: "note",
    author: "A",
    quote: "hello",
    createdAt: "2020-01-01T00:00:00.000Z",
    resolved,
  };
}

describe("comments yjs map", () => {
  it("lists and resolves without clobbering other threads", () => {
    const doc = new Y.Doc();
    const map = commentsMap(doc);
    upsertComment(map, sample("1"));
    upsertComment(map, sample("2"));
    expect(listComments(map, false)).toHaveLength(2);

    setResolved(map, "1", true);
    expect(listComments(map, false)).toHaveLength(1);
    expect(listComments(map, true)).toHaveLength(2);
    expect(map.get("2")?.resolved).toBe(false);
  });
});
