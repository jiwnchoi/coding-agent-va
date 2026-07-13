import type {
  AgentSessionNodeDescriptionRequest,
  AgentSessionSummary,
  DescriptionSettings,
  DescriptionGraphNode,
} from "@/shared/lib/generated/bindings";

import type { ContextGraphModel, ContextGraphNode } from "./types";

export function buildNodeDescriptionRequest({
  contextGraph,
  descriptionSettings,
  node,
  session,
}: {
  contextGraph: ContextGraphModel;
  descriptionSettings: DescriptionSettings;
  node: ContextGraphNode;
  session: AgentSessionSummary;
}): AgentSessionNodeDescriptionRequest | null {
  if (!session.cwd || node.data.kind !== "file") {
    return null;
  }

  const nodeById = new Map(contextGraph.nodes.map((candidate) => [candidate.id, candidate]));
  const relatedNodeIds = new Set<string>();
  const relations = contextGraph.impactEdges
    .filter((edge) => edge.source === node.id || edge.target === node.id)
    .flatMap((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      if (!source || !target || source.data.kind !== "file" || target.data.kind !== "file") {
        return [];
      }

      relatedNodeIds.add(source.id === node.id ? target.id : source.id);
      return [
        {
          importSpecifier: edge.data?.importSpecifier ?? "unknown",
          sourcePath: source.data.displayPath,
          targetPath: target.data.displayPath,
        },
      ];
    });
  const relatedNodes = [...relatedNodeIds]
    .map((nodeId) => nodeById.get(nodeId))
    .filter((candidate): candidate is ContextGraphNode => candidate !== undefined)
    .map(toDescriptionNode);

  const providerSettings = descriptionSettings[session.provider];

  return {
    provider: session.provider,
    providerSessionId: session.providerSessionId,
    transcriptPath: session.transcriptPath,
    runtimeHome: session.runtimeHome,
    model: providerSettings.model,
    reasoning: providerSettings.reasoning,
    cwd: session.cwd,
    clickedNode: toDescriptionNode(node),
    relatedNodes,
    relations,
  };
}

function toDescriptionNode(node: ContextGraphNode): DescriptionGraphNode {
  return {
    activities: node.data.activities,
    label: node.data.label,
    path: node.data.displayPath,
  };
}
