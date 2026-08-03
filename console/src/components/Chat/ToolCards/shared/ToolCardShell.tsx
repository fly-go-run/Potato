/**
 * ToolCardShell — universal wrapper for tool cards.
 *
 * Renders the compact `<details>/<summary>` layout used by ChatV2 tool
 * blocks: icon + label on a single line, expandable body underneath.
 */

import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { RightOutlined } from "@ant-design/icons";
import type { ToolCallContent } from "./types";
import DefaultBlock from "./DefaultBlock";
import { formatToolDuration, stringifyResult } from "./utils";
import styles from "./toolCards.module.less";

export interface ToolCardShellProps {
  /** Full ToolCallContent (name, params, result, status). */
  content: ToolCallContent;
  /** Whether the parent message is still streaming. */
  isStreaming?: boolean;
  /** Icon element (antd icon). */
  icon: React.ReactNode;
  /** Human-readable title to show in the summary line. */
  title: string;
  /** Optional inline result shown after the title when status === done. */
  inlineResult?: string | null;
  /** Optional badge elements (line counts, diff counts). */
  badges?: React.ReactNode;
  /** Show a compact live/completed duration in the summary row. */
  showDuration?: boolean;
  /** Expandable body content. */
  children?: React.ReactNode;
}

const ToolCardShell: React.FC<ToolCardShellProps> = ({
  content,
  icon,
  title,
  inlineResult,
  badges,
  showDuration = false,
  children,
}) => {
  const { t } = useTranslation();
  // The tool status is authoritative. The parent stream can finish before a
  // tool result arrives, so gating the spinner on isStreaming makes a real
  // running command look completed.
  const isLoading = content.status === "calling";
  const isError = content.status === "error";
  const startedAtRef = useRef<number | null>(
    content.status === "calling" ? Date.now() : null,
  );
  const [trackedDurationMs, setTrackedDurationMs] = useState<number | null>(
    content.durationMs ?? (content.status === "calling" ? 0 : null),
  );

  useEffect(() => {
    if (!showDuration) return;

    if (content.durationMs != null) {
      setTrackedDurationMs(content.durationMs);
      return;
    }

    if (content.status !== "calling") {
      if (startedAtRef.current != null) {
        setTrackedDurationMs(Date.now() - startedAtRef.current);
      }
      return;
    }

    if (startedAtRef.current == null) startedAtRef.current = Date.now();
    const updateDuration = () => {
      if (startedAtRef.current != null) {
        setTrackedDurationMs(Date.now() - startedAtRef.current);
      }
    };

    updateDuration();
    const timer = window.setInterval(updateDuration, 1000);
    return () => window.clearInterval(timer);
  }, [content.durationMs, content.status, showDuration]);

  const durationText = showDuration
    ? formatToolDuration(content.durationMs ?? trackedDurationMs)
    : "";
  const inputProgress = content.inputProgress;
  const inputPreview = inputProgress
    ? `${inputProgress.truncated ? "…\n" : ""}${inputProgress.preview}`
    : "";

  return (
    // Keep the execution log closed on first render. The summary remains a
    // useful progress row, while the full command/output is one click away.
    <details
      className={`${styles.toolCallCompact} ${
        isLoading ? styles.toolCallCompactLoading : ""
      } ${isError ? styles.toolCallCompactError : ""}`}
    >
      <summary className={styles.toolCallCompactSummary}>
        {isLoading ? (
          <span className={styles.toolCallSpinner} />
        ) : (
          <span
            className={`${styles.toolCallIcon} ${
              isError ? styles.toolCallIconError : styles.toolCallIconSuccess
            }`}
          >
            {icon}
          </span>
        )}
        <span className={styles.toolCallLabel} title={title}>
          {title}
          {isLoading && ` ${t("tool.loading")}`}
        </span>
        {isLoading && inputProgress && (
          <span className={styles.toolCallInputProgress}>
            {t("tool.inputProgress", {
              count: inputProgress.characterCount,
            })}
          </span>
        )}
        {!isLoading && !isError && badges}
        {inlineResult && (
          <span className={styles.toolCallInlineResult} title={inlineResult}>
            {inlineResult}
          </span>
        )}
        {durationText && (
          <span className={styles.toolCallDuration}>{durationText}</span>
        )}
        <span className={styles.toolCallChevron} aria-hidden="true">
          <RightOutlined />
        </span>
      </summary>
      {isError ? (
        <>
          <DefaultBlock
            title="Input"
            content={JSON.stringify(content.params, null, 2)}
          />
          <DefaultBlock
            title="Error"
            content={stringifyResult(content.result)}
          />
        </>
      ) : (
        <>
          {isLoading && inputPreview && (
            <DefaultBlock
              title={t("tool.rawInputPreview")}
              content={inputPreview}
            />
          )}
          {children}
        </>
      )}
    </details>
  );
};

export default ToolCardShell;
