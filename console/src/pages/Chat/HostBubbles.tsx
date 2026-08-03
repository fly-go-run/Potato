/**
 * pages/Chat/HostBubbles.tsx — host-side wrappers around the vendor's
 * AgentScopeRuntime{Request,Response}Card components.
 *
 * Why wrappers:
 * - Plugin extensions (chat.request.render / prepend / append and the
 *   response equivalents) need a render seam SDK doesn't expose.
 * - We register HostRequestCard / HostResponseCard into options.cards so the
 *   SDK Cards dispatcher invokes them instead of the vendor defaults.
 * - The wrapper itself subscribes to the chat extension registry via hooks,
 *   so it re-renders when plugins register/dispose — no need to rebuild the
 *   parent useMemo (and avoid re-mounting bubbles on every plugin change).
 *
 * The vendor's Card components are deep-imported because they're not in the
 * package's top-level exports. If the SDK reorganizes its internal paths,
 * update the two import statements below.
 */
import React from "react";
import VendorRequestCardOriginal from "@agentscope-ai/chat/lib/AgentScopeRuntimeWebUI/core/AgentScopeRuntime/Request/Card";
import VendorResponseCardOriginal from "@agentscope-ai/chat/lib/AgentScopeRuntimeWebUI/core/AgentScopeRuntime/Response/Card";
import {
  useChatScalarSnapshot,
  useChatListSnapshot,
} from "../../plugins/registry/useChatExtensions";
import { ChatScalar, ChatList } from "../../plugins/registry/slotKeys";
import { PluginSlotBoundary } from "../../plugins/registry/PluginSlotBoundary";
import type {
  ChatRequestData,
  ChatResponseData,
} from "../../plugins/registry/types";
import type {
  IAgentScopeRuntimeRequest,
  IAgentScopeRuntimeResponse,
} from "@agentscope-ai/chat/lib/AgentScopeRuntimeWebUI/core/AgentScopeRuntime/types";

// Vendor `.d.ts` doesn't yet describe the contentPrepend/contentAppend
// slots we added in the patched .js (Response/Card.js + Request/Card.js).
// Keep that integration boundary explicit instead of widening the whole card
// API to `any`.
type VendorRequestCardProps = {
  data: IAgentScopeRuntimeRequest;
  contentPrepend?: React.ReactNode;
  contentAppend?: React.ReactNode;
};
type VendorResponseCardProps = {
  data: IAgentScopeRuntimeResponse;
  isLast?: boolean;
  contentPrepend?: React.ReactNode;
  contentAppend?: React.ReactNode;
};

const VendorRequestCard =
  VendorRequestCardOriginal as React.ComponentType<VendorRequestCardProps>;
const VendorResponseCard =
  VendorResponseCardOriginal as React.ComponentType<VendorResponseCardProps>;

function asVendorRequestData(
  value: ChatRequestData,
): IAgentScopeRuntimeRequest | null {
  return Array.isArray(value.input)
    ? (value as unknown as IAgentScopeRuntimeRequest)
    : null;
}

function asVendorResponseData(
  value: ChatResponseData,
): IAgentScopeRuntimeResponse | null {
  const isValid =
    typeof value.id === "string" &&
    typeof value.status === "string" &&
    typeof value.created_at === "number" &&
    Array.isArray(value.output);
  return isValid ? (value as unknown as IAgentScopeRuntimeResponse) : null;
}

function sortByOrder<T extends { item: { order?: number } }>(arr: T[]): T[] {
  return arr
    .slice()
    .sort((a, b) => (a.item.order ?? 100) - (b.item.order ?? 100));
}

export function HostRequestCard(props: { data: ChatRequestData }) {
  const extScalar = useChatScalarSnapshot();
  const extLists = useChatListSnapshot();

  const renderEntry = extScalar[ChatScalar.requestRender];
  const renderFn = renderEntry?.value;
  const prependList = sortByOrder(extLists[ChatList.requestPrepend]);
  const appendList = sortByOrder(extLists[ChatList.requestAppend]);

  // prepend/append routed through vendor's contentPrepend/contentAppend
  // slot so actions stay last. Mirrors HostResponseCard.
  const contentPrepend =
    prependList.length === 0 ? null : (
      <>
        {prependList.map((e) => (
          <PluginSlotBoundary
            key={e.item.id}
            slot={ChatList.requestPrepend}
            pluginId={e.pluginId}
          >
            {e.item.render({ data: props.data })}
          </PluginSlotBoundary>
        ))}
      </>
    );
  const contentAppend =
    appendList.length === 0 ? null : (
      <>
        {appendList.map((e) => (
          <PluginSlotBoundary
            key={e.item.id}
            slot={ChatList.requestAppend}
            pluginId={e.pluginId}
          >
            {e.item.render({ data: props.data })}
          </PluginSlotBoundary>
        ))}
      </>
    );

  const fallback = (): React.ReactElement => {
    const vendorData = asVendorRequestData(props.data);
    return vendorData ? (
      <VendorRequestCard
        data={vendorData}
        contentPrepend={contentPrepend}
        contentAppend={contentAppend}
      />
    ) : (
      <></>
    );
  };

  if (renderFn) {
    return (
      <PluginSlotBoundary
        slot={ChatScalar.requestRender}
        pluginId={renderEntry!.pluginId}
        fallback={fallback()}
      >
        {renderFn({ data: props.data, fallback })}
      </PluginSlotBoundary>
    );
  }
  return fallback();
}

export function HostResponseCard(props: {
  data: ChatResponseData;
  isLast?: boolean;
}) {
  const extScalar = useChatScalarSnapshot();
  const extLists = useChatListSnapshot();

  const renderEntry = extScalar[ChatScalar.responseRender];
  const renderFn = renderEntry?.value;
  const prependList = sortByOrder(extLists[ChatList.responsePrepend]);
  const appendList = sortByOrder(extLists[ChatList.responseAppend]);

  // prepend/append are routed through vendor's contentPrepend/contentAppend
  // slot so they land BETWEEN messages and Actions — actions always last.
  // Vendor change: see Response/Card.js DefaultResponseRender, which now
  // reads props.contentPrepend / props.contentAppend.
  const contentPrepend =
    prependList.length === 0 ? null : (
      <>
        {prependList.map((e) => (
          <PluginSlotBoundary
            key={e.item.id}
            slot={ChatList.responsePrepend}
            pluginId={e.pluginId}
          >
            {e.item.render({ data: props.data, isLast: props.isLast })}
          </PluginSlotBoundary>
        ))}
      </>
    );
  const contentAppend =
    appendList.length === 0 ? null : (
      <>
        {appendList.map((e) => (
          <PluginSlotBoundary
            key={e.item.id}
            slot={ChatList.responseAppend}
            pluginId={e.pluginId}
          >
            {e.item.render({ data: props.data, isLast: props.isLast })}
          </PluginSlotBoundary>
        ))}
      </>
    );

  const fallback = (): React.ReactElement => {
    const vendorData = asVendorResponseData(props.data);
    return vendorData ? (
      <VendorResponseCard
        data={vendorData}
        isLast={props.isLast}
        contentPrepend={contentPrepend}
        contentAppend={contentAppend}
      />
    ) : (
      <></>
    );
  };

  if (renderFn) {
    return (
      <PluginSlotBoundary
        slot={ChatScalar.responseRender}
        pluginId={renderEntry!.pluginId}
        fallback={fallback()}
      >
        {renderFn({
          data: props.data,
          isLast: props.isLast,
          fallback,
        })}
      </PluginSlotBoundary>
    );
  }
  return fallback();
}
