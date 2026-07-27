import type { RateLimitedFrame } from "./protocol/types";

export type ChatBannerState =
  | {
      tone: "warn";
      message: string;
      alternatives: RateLimitedFrame["alternatives"];
    }
  | {
      tone: "danger";
      message: string;
      alternatives: [];
    }
  | null;

export function getChatBanner(
  error: string | null,
  rateLimited: RateLimitedFrame | null,
): ChatBannerState {
  if (rateLimited) {
    return {
      tone: "warn",
      message: rateLimited.error,
      alternatives: rateLimited.alternatives,
    };
  }
  if (error) {
    return { tone: "danger", message: error, alternatives: [] };
  }
  return null;
}
