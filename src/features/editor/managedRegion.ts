/**
 * Authority model A for agent-protocol run records:
 * - Content before (and including) the `## Human notes` heading is Moraine-managed.
 * - Only the Human notes body is free-form editable.
 * - Comment marks may still target managed content; text replacements (suggestion accept) may not.
 */

import type { Node as PMNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { ReplaceAroundStep, ReplaceStep } from "@tiptap/pm/transform";

export const MANAGED_REGION_PLUGIN_KEY = new PluginKey("moraineManagedRegion");

const HUMAN_NOTES_TITLE = "Human notes";

/** Detect protocol-created Markdown (projection + managed notice). */
export function isProtocolRunMarkdown(md: string): boolean {
  if (!md.includes("## Human notes")) return false;
  if (!md.includes("## Protocol status")) return false;
  // Stable notice from core renderer (see agent_protocol/markdown.rs).
  return (
    md.includes("Managed regions") ||
    md.includes("**Run ID:**") ||
    md.includes("**Lifecycle:**")
  );
}

/**
 * Document position of the start of the Human notes *heading* node.
 * Everything from 0..pos is managed (including the heading).
 * Content after the heading node is the editable Human notes body.
 * Returns null when not a protocol structure (no Human notes heading).
 */
export function findHumanNotesHeadingPos(doc: PMNode): number | null {
  let found: number | null = null;
  doc.descendants((node, pos) => {
    if (found != null) return false;
    if (node.type.name === "heading" && Number(node.attrs.level) === 2) {
      if (node.textContent.trim() === HUMAN_NOTES_TITLE) {
        found = pos;
        return false;
      }
    }
    return true;
  });
  return found;
}

/** End position of the Human notes heading node (start of body content). */
export function findHumanNotesBodyPos(doc: PMNode): number | null {
  let body: number | null = null;
  doc.descendants((node, pos) => {
    if (body != null) return false;
    if (node.type.name === "heading" && Number(node.attrs.level) === 2) {
      if (node.textContent.trim() === HUMAN_NOTES_TITLE) {
        body = pos + node.nodeSize;
        return false;
      }
    }
    return true;
  });
  return body;
}

/** True if [from, to) overlaps the managed region (before Human notes body). */
export function rangeTouchesManaged(doc: PMNode, from: number, to: number): boolean {
  const bodyPos = findHumanNotesBodyPos(doc);
  if (bodyPos == null) return false;
  // Managed is [0, bodyPos). Ranges that start or end before bodyPos touch managed text.
  const a = Math.min(from, to);
  const b = Math.max(from, to);
  return a < bodyPos && b > 0 && a < b ? a < bodyPos : a < bodyPos;
}

/** True if a suggestion accept at [from,to) would mutate managed text. */
export function suggestionAcceptTouchesManaged(
  doc: PMNode,
  from: number,
  to: number,
): boolean {
  const bodyPos = findHumanNotesBodyPos(doc);
  if (bodyPos == null) return false;
  return from < bodyPos || to < bodyPos;
}

/**
 * ProseMirror plugin: when protocolMode is true, block document structure/text
 * changes that touch the managed region. Mark-only steps (comments) are allowed.
 */
export function createManagedRegionPlugin(getEnabled: () => boolean): Plugin {
  return new Plugin({
    key: MANAGED_REGION_PLUGIN_KEY,
    filterTransaction(tr, state) {
      if (!getEnabled()) return true;
      if (!tr.docChanged) return true;

      const bodyPos = findHumanNotesBodyPos(state.doc);
      if (bodyPos == null) return true;

      for (const step of tr.steps) {
        if (step instanceof ReplaceStep || step instanceof ReplaceAroundStep) {
          const from = step.from;
          const to = step.to;
          // Any replace that starts before the Human notes body is blocked.
          if (from < bodyPos) {
            return false;
          }
          // Also block if mapping would shift managed content via replace that spans into it.
          if (to < bodyPos) {
            return false;
          }
        }
        // Allow AddMarkStep / RemoveMarkStep (comments on managed text).
      }
      return true;
    },
  });
}
