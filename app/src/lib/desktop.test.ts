import { afterEach, describe, expect, it, vi } from "vitest";
import {
  getDesktopWindowStatePreference,
  hasDesktopHostBridge,
  listenDesktopEvent,
  resetDesktopWindowState,
  runDesktopCloseAction,
  setDesktopWindowStatePreference,
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

  it("bridges window geometry preferences through native commands", async () => {
    const invoke = vi.fn(async (command: string) => {
      if (command === "get_window_state_preference") {
        return { remember: true, width: 1440, height: 900, maximized: false };
      }
      return undefined;
    });
    installDesktopHost(invoke, () => 1);

    await expect(getDesktopWindowStatePreference()).resolves.toEqual({
      remember: true,
      width: 1440,
      height: 900,
      maximized: false,
    });
    await expect(setDesktopWindowStatePreference(false)).resolves.toBe(true);
    await expect(resetDesktopWindowState()).resolves.toBe(true);

    expect(invoke).toHaveBeenCalledWith(
      "get_window_state_preference",
      undefined,
    );
    expect(invoke).toHaveBeenCalledWith("set_window_state_preference", {
      remember: false,
    });
    expect(invoke).toHaveBeenCalledWith("reset_window_state", undefined);
  });

  it("no-ops window geometry helpers without a desktop bridge", async () => {
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: { location: { search: "?desktop=1" } },
    });

    await expect(getDesktopWindowStatePreference()).resolves.toBeNull();
    await expect(setDesktopWindowStatePreference(true)).resolves.toBe(false);
    await expect(resetDesktopWindowState()).resolves.toBe(false);
  });
});
