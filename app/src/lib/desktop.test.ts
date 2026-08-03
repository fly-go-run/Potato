import { afterEach, describe, expect, it, vi } from "vitest";
import {
  hasDesktopHostBridge,
  listenDesktopEvent,
  runDesktopCloseAction,
} from "./desktop";

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");

afterEach(() => {
  if (originalWindow) {
    Object.defineProperty(globalThis, "window", originalWindow);
  } else {
    delete (globalThis as { window?: unknown }).window;
  }
});

function installDesktopHost(
  invoke: (command: string, args?: unknown) => Promise<unknown>,
  transformCallback?: (callback: (event: unknown) => void) => number,
) {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      location: { search: "?desktop=1" },
      __TAURI_INTERNALS__: { invoke, transformCallback },
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: vi.fn() },
    },
  });
}

describe("desktop host bridge", () => {
  it("does not enable desktop behavior for a normal browser page", () => {
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: { location: { search: "" } },
    });

    expect(hasDesktopHostBridge()).toBe(false);
  });

  it("registers and removes Tauri events through the injected bridge", async () => {
    let callback: ((event: unknown) => void) | undefined;
    const invoke = vi.fn(async (command: string) => {
      if (command === "plugin:event|listen") return 42;
      return undefined;
    });
    installDesktopHost(invoke, (registered) => {
      callback = registered;
      return 9;
    });
    const received = vi.fn();

    const unlisten = await listenDesktopEvent<string>(
      "qwenpaw-close-requested",
      received,
    );

    expect(invoke).toHaveBeenCalledWith("plugin:event|listen", {
      event: "qwenpaw-close-requested",
      target: { kind: "Any" },
      handler: 9,
    });
    callback?.({ payload: "close" });
    expect(received).toHaveBeenCalledWith("close");

    unlisten?.();
    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith("plugin:event|unlisten", {
      event: "qwenpaw-close-requested",
      eventId: 42,
    });
  });

  it("maps close choices to their native commands", async () => {
    const invoke = vi.fn(async () => undefined);
    installDesktopHost(invoke, () => 1);

    await runDesktopCloseAction("minimize-to-tray");
    await runDesktopCloseAction("quit");

    expect(invoke).toHaveBeenNthCalledWith(1, "minimize_to_tray", undefined);
    expect(invoke).toHaveBeenNthCalledWith(2, "quit_app", undefined);
  });
});
