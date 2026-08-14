import { describe, expect, it, beforeEach } from "vitest";
import {
  legacyParseCounts,
  parseQpMeta,
  qpBool,
  qpCount,
  qpInt,
  qpString,
  recordLegacyParse,
  resetLegacyParseCounts,
} from "./toolMeta";

describe("parseQpMeta", () => {
  it("parses a well-formed qp meta", () => {
    expect(
      parseQpMeta({
        v: 1,
        kind: "file_write",
        ok: true,
        data: { path: "/a", bytes_written: 12 },
      }),
    ).toEqual({
      v: 1,
      kind: "file_write",
      ok: true,
      data: { path: "/a", bytes_written: 12 },
    });
  });

  it.each([
    ["null", null],
    ["non-object", "qp"],
    ["array", []],
    ["future version", { v: 2, kind: "file_write", ok: true, data: {} }],
    ["missing kind", { v: 1, ok: true, data: {} }],
    ["empty kind", { v: 1, kind: "", ok: true, data: {} }],
    ["non-boolean ok", { v: 1, kind: "shell", ok: "yes", data: {} }],
    ["non-record data", { v: 1, kind: "shell", ok: true, data: [] }],
  ])("rejects %s without throwing", (_label, raw) => {
    expect(parseQpMeta(raw)).toBeNull();
  });

  it("keeps unknown kinds (forward-compatible)", () => {
    const meta = parseQpMeta({ v: 1, kind: "future_kind", ok: true, data: {} });
    expect(meta?.kind).toBe("future_kind");
  });
});

describe("qp accessors", () => {
  const meta = parseQpMeta({
    v: 1,
    kind: "shell",
    ok: false,
    data: { exit_code: 0, sandboxed: false, violation: "denied", bad: -1 },
  });

  it("reads typed fields and refuses everything else", () => {
    expect(qpCount(meta, "exit_code")).toBe(0);
    expect(qpCount(meta, "bad")).toBeNull(); // 负数不是 count
    expect(qpCount(meta, "missing")).toBeNull();
    expect(qpCount(null, "exit_code")).toBeNull();
    expect(qpBool(meta, "sandboxed")).toBe(false);
    expect(qpBool(meta, "exit_code")).toBeNull();
    expect(qpString(meta, "violation")).toBe("denied");
    expect(qpString(meta, "sandboxed")).toBeNull();
  });

  it("qpInt keeps negative exit codes (timeout=-1, signal kills)", () => {
    const shellMeta = parseQpMeta({
      v: 1,
      kind: "shell",
      ok: false,
      data: { exit_code: -1 },
    });
    expect(qpInt(shellMeta, "exit_code")).toBe(-1);
    expect(qpCount(shellMeta, "exit_code")).toBeNull();
    expect(qpInt(shellMeta, "missing")).toBeNull();
    expect(qpInt(null, "exit_code")).toBeNull();
  });
});

describe("legacy parse counter", () => {
  beforeEach(() => resetLegacyParseCounts());

  it("counts per seam in dev", () => {
    recordLegacyParse("F1:bytes-regex");
    recordLegacyParse("F1:bytes-regex");
    recordLegacyParse("F7:lcs-estimate");
    expect(legacyParseCounts()).toEqual({
      "F1:bytes-regex": 2,
      "F7:lcs-estimate": 1,
    });
  });
});
