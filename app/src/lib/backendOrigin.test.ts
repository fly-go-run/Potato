import { afterEach, describe, expect, it, vi } from "vitest";
import {
  backendHttpOriginFromPort,
  getBackendOrigin,
  isBackendHostedConsole,
  needsDesktopBackendOrigin,
  probeBackendVersion,
  resetBackendOrigin,
  resolveBackendUrl,
  setBackendOrigin,
  waitForBackendOrigin,
} from "./backendOrigin";

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");

afterEach(() => {
  resetBackendOrigin();
  if (originalWindow) {
    Object.defineProperty(globalThis, "window", originalWindow);
  } else {
    delete (globalThis as { window?: unknown }).window;
  }
});

function stubWindow(partial: {
  protocol?: string;
  hostname?: string;
  pathname?: string;
  search?: string;
  internals?: unknown;
}) {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      location: {
        protocol: partial.protocol ?? "http:",
        hostname: partial.hostname ?? "localhost",
        pathname: partial.pathname ?? "/",
        search: partial.search ?? "",
      },
      __TAURI_INTERNALS__: partial.internals,
    },
  });
}

describe("backend origin", () => {
  it("leaves relative API paths unchanged until an origin is set", () => {
    expect(resolveBackendUrl("/api/version")).toBe("/api/version");
    expect(resolveBackendUrl("https://cdn.example/file.png")).toBe(
      "https://cdn.example/file.png",
    );
  });

  it("prefixes relative paths after the sidecar origin is known", () => {
    setBackendOrigin("http://127.0.0.1:8090/");
    expect(getBackendOrigin()).toBe("http://127.0.0.1:8090");
    expect(resolveBackendUrl("/api/chats")).toBe(
      "http://127.0.0.1:8090/api/chats",
    );
    expect(backendHttpOriginFromPort(8090)).toBe("http://127.0.0.1:8090");
  });

  it("detects the legacy backend-hosted console path", () => {
    stubWindow({
      hostname: "127.0.0.1",
      pathname: "/console",
    });
    expect(isBackendHostedConsole()).toBe(true);
    expect(needsDesktopBackendOrigin()).toBe(false);
  });

  it("requires a sidecar origin for the bundled desktop app", () => {
    stubWindow({
      protocol: "tauri:",
      hostname: "localhost",
      pathname: "/",
      internals: { invoke: vi.fn() },
    });
    expect(isBackendHostedConsole()).toBe(false);
    expect(needsDesktopBackendOrigin()).toBe(true);
  });

  it("resolves waiters immediately in the browser", async () => {
    stubWindow({ hostname: "localhost", pathname: "/" });
    await expect(waitForBackendOrigin(20)).resolves.toBe("");
  });

  it("resolves waiters when the sidecar origin is published", async () => {
    stubWindow({
      protocol: "tauri:",
      internals: {
        invoke: vi.fn(async () => null),
      },
    });
    const pending = waitForBackendOrigin(500);
    setBackendOrigin("http://127.0.0.1:8090");
    await expect(pending).resolves.toBe("http://127.0.0.1:8090");
  });
});

describe("probeBackendVersion", () => {
  it("returns true when /api/version is healthy", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("ok", { status: 200 })),
    );
    await expect(probeBackendVersion("http://127.0.0.1:8090")).resolves.toBe(
      true,
    );
    expect(fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:8090/api/version",
      expect.objectContaining({ cache: "no-store" }),
    );
    vi.unstubAllGlobals();
  });

  it("returns false when the sidecar is not answering", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    await expect(probeBackendVersion("http://127.0.0.1:8090")).resolves.toBe(
      false,
    );
    vi.unstubAllGlobals();
  });
});
