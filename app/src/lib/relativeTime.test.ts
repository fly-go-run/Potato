import { describe, expect, it } from "vitest";
import { relativeTime } from "./relativeTime";

const NOW = Date.parse("2026-07-28T12:00:00Z");

describe("relativeTime", () => {
  it("returns null for missing or unparsable values", () => {
    expect(relativeTime(null, NOW)).toBeNull();
    expect(relativeTime(undefined, NOW)).toBeNull();
    expect(relativeTime("", NOW)).toBeNull();
    expect(relativeTime("not-a-date", NOW)).toBeNull();
  });

  it("treats anything under a minute as just now", () => {
    expect(relativeTime("2026-07-28T11:59:01Z", NOW)?.key).toBe("time.justNow");
    expect(relativeTime("2026-07-28T12:00:00Z", NOW)?.key).toBe("time.justNow");
  });

  it("tolerates future timestamps from clock skew", () => {
    expect(relativeTime("2026-07-28T12:05:00Z", NOW)?.key).toBe("time.justNow");
  });

  it("floors to minutes, hours and days", () => {
    expect(relativeTime("2026-07-28T11:58:30Z", NOW)).toEqual({
      key: "time.minutesAgo",
      params: { count: 1 },
    });
    expect(relativeTime("2026-07-28T11:01:00Z", NOW)).toEqual({
      key: "time.minutesAgo",
      params: { count: 59 },
    });
    expect(relativeTime("2026-07-28T09:30:00Z", NOW)).toEqual({
      key: "time.hoursAgo",
      params: { count: 2 },
    });
    expect(relativeTime("2026-07-27T12:00:00Z", NOW)).toEqual({
      key: "time.daysAgo",
      params: { count: 1 },
    });
    expect(relativeTime("2026-06-23T12:00:00Z", NOW)).toEqual({
      key: "time.daysAgo",
      params: { count: 35 },
    });
  });

  it("accepts offset-carrying ISO strings from the backend", () => {
    expect(relativeTime("2026-07-28T11:00:00+00:00", NOW)).toEqual({
      key: "time.hoursAgo",
      params: { count: 1 },
    });
  });
});
