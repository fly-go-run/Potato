import { describe, expect, it } from "vitest";
import { buildOutboundContent, findOversizedFile } from "./uploads";

describe("buildOutboundContent", () => {
  it("assembles text, image and regular file blocks in upload order", () => {
    expect(
      buildOutboundContent("请处理附件", [
        {
          url: "stored-image.png",
          filename: "photo.png",
          mimeType: "image/png",
        },
        {
          url: "stored-report.pdf",
          filename: "report.pdf",
          mimeType: "application/pdf",
        },
      ]),
    ).toEqual([
      { type: "text", text: "请处理附件" },
      { type: "image", image_url: "stored-image.png" },
      {
        type: "file",
        file_url: "stored-report.pdf",
        filename: "report.pdf",
      },
    ]);
  });
});

describe("findOversizedFile", () => {
  it("finds the first file over the configured per-file limit", () => {
    const files = [
      { name: "small.png", size: 1024 },
      { name: "large.png", size: 2 * 1024 * 1024 },
    ];
    expect(findOversizedFile(files, 1)?.name).toBe("large.png");
  });

  it("allows every file when the backend reports no limit", () => {
    expect(
      findOversizedFile([{ name: "large.bin", size: Number.MAX_SAFE_INTEGER }], null),
    ).toBeNull();
  });
});
