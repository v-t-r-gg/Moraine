import { describe, expect, it } from "vitest";
import {
  isProtocolRunMarkdown,
  rangeTouchesManaged,
  suggestionAcceptTouchesManaged,
} from "./managedRegion";
import { Schema } from "@tiptap/pm/model";
import { nodes, marks } from "@tiptap/pm/schema-basic";

// Minimal schema with heading + paragraph for position tests
const schema = new Schema({
  nodes: {
    doc: nodes.doc,
    text: nodes.text,
    paragraph: nodes.paragraph,
    heading: nodes.heading,
    hard_break: nodes.hard_break,
  },
  marks: {
    strong: marks.strong,
  },
});

function docFromHeadings(parts: { level?: number; text: string }[]) {
  const children = parts.map((p) => {
    if (p.level) {
      return schema.node("heading", { level: p.level }, [schema.text(p.text)]);
    }
    return schema.node("paragraph", null, p.text ? [schema.text(p.text)] : []);
  });
  return schema.node("doc", null, children);
}

describe("isProtocolRunMarkdown", () => {
  it("detects protocol projection", () => {
    const md = `# Moraine run record

## Protocol status

> **Managed regions:** note

- **Run ID:** \`x\`

## Human notes

hi
`;
    expect(isProtocolRunMarkdown(md)).toBe(true);
  });

  it("rejects ordinary markdown", () => {
    expect(isProtocolRunMarkdown("# Notes\n\nhello\n")).toBe(false);
    expect(isProtocolRunMarkdown("## Human notes\nonly\n")).toBe(false);
  });
});

describe("managed range detection", () => {
  it("treats content before Human notes body as managed", () => {
    const doc = docFromHeadings([
      { level: 1, text: "Moraine run record" },
      { level: 2, text: "Objective" },
      { text: "do things" },
      { level: 2, text: "Human notes" },
      { text: "free form" },
    ]);
    // Positions: rough — first content is managed
    expect(suggestionAcceptTouchesManaged(doc, 1, 5)).toBe(true);
    // After human notes heading: free form area — find body pos then test past it
    // Walk to get a pos after Human notes
    let afterNotes = 0;
    doc.descendants((node, pos) => {
      if (node.type.name === "heading" && node.textContent === "Human notes") {
        afterNotes = pos + node.nodeSize;
      }
    });
    expect(suggestionAcceptTouchesManaged(doc, afterNotes, afterNotes + 2)).toBe(
      false,
    );
    expect(rangeTouchesManaged(doc, afterNotes, afterNotes + 3)).toBe(false);
  });
});
