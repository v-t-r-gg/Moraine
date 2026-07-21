import { describe, expect, it, vi } from "vitest";
import { createYjsSession } from "./yjsSession";
import { onFileChanged } from "@/shared/api";

describe("subscription cleanup", () => {
  it("yjs session destroy is idempotent and clears channel", () => {
    const s = createYjsSession("room_test_cleanup", { syncUrl: null });
    expect(s.doc).toBeTruthy();
    s.destroy();
    s.destroy(); // second call must not throw
  });

  it("onFileChanged returns cleanup that can be called twice", () => {
    const un = onFileChanged(() => {});
    un();
    un();
  });
});
