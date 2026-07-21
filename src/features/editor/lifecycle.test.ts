import { describe, expect, it, vi } from "vitest";
import { createYjsSession } from "./yjsSession";
import { subscribeFileChanged, type TauriListen } from "@/shared/api";

describe("subscription cleanup", () => {
  it("yjs session destroy is idempotent", () => {
    const s = createYjsSession("room_test_cleanup", { syncUrl: null });
    expect(s.doc).toBeTruthy();
    s.destroy();
    s.destroy();
  });
});

describe("subscribeFileChanged Strict Mode / late listen", () => {
  it("cleanup before listen resolves unsubscribes the late registration", async () => {
    let resolveListen!: (fn: () => void) => void;
    const lateUnlisten = vi.fn();
    const handler = vi.fn();

    const listen: TauriListen = vi.fn(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = resolve;
        }),
    );

    const unlisten = subscribeFileChanged(listen, handler);
    // Strict Mode: cleanup before async listen settles.
    unlisten();

    resolveListen(lateUnlisten);
    await Promise.resolve();
    await Promise.resolve();

    expect(lateUnlisten).toHaveBeenCalledTimes(1);
    expect(handler).not.toHaveBeenCalled();
  });

  it("remount keeps a single active listener; prior cleanup does not leak", async () => {
    const resolvers: Array<(fn: () => void) => void> = [];
    const handlers: Array<(e: { payload: unknown }) => void> = [];
    const unlistenFns: Array<ReturnType<typeof vi.fn>> = [];

    const listen: TauriListen = vi.fn((_event, handler) => {
      handlers.push(handler as (e: { payload: unknown }) => void);
      return new Promise<() => void>((resolve) => {
        resolvers.push(resolve);
      });
    });

    const h1 = vi.fn();
    const h2 = vi.fn();

    // First mount → cleanup (Strict Mode)
    const u1 = subscribeFileChanged(listen, h1);
    u1();
    const un1 = vi.fn();
    resolvers[0]!(un1);
    await Promise.resolve();
    await Promise.resolve();
    expect(un1).toHaveBeenCalledTimes(1);

    // Second mount
    const u2 = subscribeFileChanged(listen, h2);
    const un2 = vi.fn();
    resolvers[1]!(un2);
    await Promise.resolve();
    await Promise.resolve();

    expect(listen).toHaveBeenCalledTimes(2);

    // Only second handler is active (first was cancelled before resolve).
    handlers[0]!({
      payload: { path: "/a.md", change: "modify", documentId: "1" },
    });
    handlers[1]!({
      payload: { path: "/b.md", change: "modify", documentId: "2" },
    });
    expect(h1).not.toHaveBeenCalled();
    expect(h2).toHaveBeenCalledTimes(1);

    u2();
    expect(un2).toHaveBeenCalledTimes(1);

    // After second cleanup, further events must not reach h2.
    handlers[1]!({
      payload: { path: "/c.md", change: "modify", documentId: "3" },
    });
    expect(h2).toHaveBeenCalledTimes(1);
  });
});
