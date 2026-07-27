import type { StreamMessage } from "../../lib/stream";
import { useChatStore } from "../../stores/chat";
import { ApprovalCard } from "./ApprovalCard";
import { MessageContent } from "./MessageContent";
import { ProgressCard } from "./ProgressCard";
import { ReasoningBlock } from "./ReasoningBlock";
import { buildToolPair, toolData, ToolCard } from "./ToolCard";

interface MessageListProps {
  messages: StreamMessage[];
}

interface Turn {
  id: string;
  role: "user" | "assistant";
  messages: StreamMessage[];
}

export function MessageList({ messages }: MessageListProps) {
  const turns = groupIntoTurns(messages);
  const pendingApprovals = useChatStore((state) => state.pendingApprovals);
  return (
    <div className="mx-auto w-full max-w-3xl px-6 pb-8 pt-6">
      {turns.map((turn) =>
        turn.role === "user" ? (
          <UserTurn key={turn.id} messages={turn.messages} />
        ) : (
          <AssistantTurn key={turn.id} messages={turn.messages} />
        ),
      )}
      {pendingApprovals.map((approval) => (
        <ApprovalCard key={approval.request_id} approval={approval} />
      ))}
    </div>
  );
}

function UserTurn({ messages }: { messages: StreamMessage[] }) {
  return (
    <div className="mb-6 flex justify-end">
      <div className="max-w-[82%] rounded-bubble bg-bubble-user px-4 py-2.5">
        {messages.map((message) => (
          <MessageContent
            key={message.id}
            content={message.content}
            markdown={false}
          />
        ))}
      </div>
    </div>
  );
}

function AssistantTurn({ messages }: { messages: StreamMessage[] }) {
  const pairedOutputs = new Set<string>();

  return (
    <div className="mb-8">
      {messages.map((message) => {
        if (message.type === "reasoning") {
          return <ReasoningBlock key={message.id} message={message} />;
        }
        if (message.type === "progress") {
          return <ProgressCard key={message.id} message={message} />;
        }
        if (isToolCall(message.type)) {
          const callId = stringValue(toolData(message).call_id);
          const output = messages.find(
            (candidate) =>
              isToolOutput(candidate.type) &&
              stringValue(toolData(candidate).call_id) === callId,
          );
          if (output) pairedOutputs.add(output.id);
          return (
            <ToolCard
              key={message.id}
              pair={buildToolPair(message, output ?? null)}
            />
          );
        }
        if (isToolOutput(message.type)) {
          if (pairedOutputs.has(message.id)) return null;
          return (
            <ToolCard
              key={message.id}
              pair={buildToolPair(null, message)}
            />
          );
        }

        if (message.content.length === 0) return null;
        return (
          <div key={message.id} className="py-1">
            <MessageContent content={message.content} markdown />
          </div>
        );
      })}
    </div>
  );
}

export function groupIntoTurns(messages: StreamMessage[]): Turn[] {
  const turns: Turn[] = [];
  for (const message of messages) {
    if (message.role === "user") {
      turns.push({ id: message.id, role: "user", messages: [message] });
      continue;
    }
    const last = turns.at(-1);
    if (!last || last.role === "user") {
      turns.push({
        id: `assistant-${message.id}`,
        role: "assistant",
        messages: [message],
      });
    } else {
      last.messages.push(message);
    }
  }
  return turns;
}

function isToolCall(type: StreamMessage["type"]) {
  return (
    type === "plugin_call" ||
    type === "function_call" ||
    type === "mcp_tool_call"
  );
}

function isToolOutput(type: StreamMessage["type"]) {
  return (
    type === "plugin_call_output" ||
    type === "function_call_output" ||
    type === "mcp_tool_call_output"
  );
}

function stringValue(value: unknown) {
  return typeof value === "string" ? value : "";
}
