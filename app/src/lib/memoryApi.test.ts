import { afterEach, expect, it, vi } from "vitest";
import { memoryApi } from "./memory";

afterEach(() => vi.unstubAllGlobals());
it("submits the editor baseline and surfaces a conflict without retrying", async () => {
  vi.stubGlobal("localStorage", { getItem: () => null });
  const fetchMock = vi.fn().mockResolvedValue(Response.json({ detail: "changed elsewhere" }, { status: 409 }));
  vi.stubGlobal("fetch", fetchMock);
  await expect(memoryApi.update("note.md", "draft", "original")).rejects.toMatchObject({ status: 409 });
  expect(fetchMock).toHaveBeenCalledTimes(1);
  expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({ content: "draft", expected_content: "original" });
});
