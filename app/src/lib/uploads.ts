export interface UploadedAttachment {
  url: string;
  filename: string;
  mimeType: string;
}

export type OutboundContentBlock =
  | { type: "text"; text: string }
  | { type: "image"; image_url: string }
  | { type: "file"; file_url: string; filename: string };

export function buildOutboundContent(
  text: string,
  attachments: UploadedAttachment[],
): OutboundContentBlock[] {
  return [
    { type: "text", text },
    ...attachments.map(
      (attachment): OutboundContentBlock =>
        attachment.mimeType.startsWith("image/")
          ? { type: "image", image_url: attachment.url }
          : {
              type: "file",
              file_url: attachment.url,
              filename: attachment.filename,
            },
    ),
  ];
}

export function findOversizedFile(
  files: Array<Pick<File, "name" | "size">>,
  limitMb: number | null,
): Pick<File, "name" | "size"> | null {
  if (limitMb === null) return null;
  const maximumBytes = limitMb * 1024 * 1024;
  return files.find((file) => file.size > maximumBytes) ?? null;
}
