import { layoutContextGraphWithHierarchy } from "./layoutContextGraphWithHierarchy";
import type {
  ContextGraphEdge,
  ContextGraphEdgeData,
  ContextGraphModel,
  ContextGraphNode,
} from "./types";

const MAX_CACHED_LAYOUTS = 20;
const layoutByTopologyKey = new Map<string, ContextGraphModel>();

export function layoutContextGraph(model: ContextGraphModel) {
  if (model.nodes.length === 0) {
    return model;
  }

  const topologyKey = contextGraphTopologyKey(model);
  const cachedLayout = layoutByTopologyKey.get(topologyKey);

  if (cachedLayout) {
    touchCachedLayout(topologyKey, cachedLayout);
    return applyCurrentGraphState(cachedLayout, model);
  }

  const layout = layoutContextGraphWithHierarchy(model);
  touchCachedLayout(topologyKey, layout);
  return layout;
}

function contextGraphTopologyKey(model: ContextGraphModel) {
  return JSON.stringify({
    nodes: model.nodes.map((node) => `${node.id}:${node.data.kind}`).sort(),
    containsEdges: model.containsEdges.map((edge) => `${edge.source}>${edge.target}`).sort(),
    impactEdges: model.impactEdges.map((edge) => `${edge.source}>${edge.target}`).sort(),
  });
}

function applyCurrentGraphState(
  cached: ContextGraphModel,
  current: ContextGraphModel
): ContextGraphModel {
  const currentNodeById = new Map(current.nodes.map((node) => [node.id, node]));
  const currentContainsEdgeById = new Map(current.containsEdges.map((edge) => [edge.id, edge]));
  const currentImpactEdgeById = new Map(current.impactEdges.map((edge) => [edge.id, edge]));

  return {
    ...current,
    nodes: cached.nodes.flatMap((node) => {
      const currentNode = currentNodeById.get(node.id);
      return currentNode ? [applyCurrentNodeState(node, currentNode)] : [];
    }),
    containsEdges: cached.containsEdges.flatMap((edge) => {
      const currentEdge = currentContainsEdgeById.get(edge.id);
      return currentEdge ? [applyCurrentEdgeState(edge, currentEdge)] : [];
    }),
    impactEdges: cached.impactEdges.flatMap((edge) => {
      const currentEdge = currentImpactEdgeById.get(edge.id);
      return currentEdge ? [applyCurrentEdgeState(edge, currentEdge)] : [];
    }),
  };
}

function applyCurrentNodeState(cached: ContextGraphNode, current: ContextGraphNode) {
  return {
    ...cached,
    className: current.className,
    data: current.data,
  };
}

function applyCurrentEdgeState(
  cached: ContextGraphEdge,
  current: ContextGraphEdge
): ContextGraphEdge {
  const currentData: ContextGraphEdgeData = current.data ??
    cached.data ?? { kind: "contains", isHighlighted: false };

  return {
    ...cached,
    className: current.className,
    data: {
      ...currentData,
      sourceOffset: cached.data?.sourceOffset,
      targetOffset: cached.data?.targetOffset,
    },
    zIndex: current.zIndex,
  };
}

function touchCachedLayout(key: string, layout: ContextGraphModel) {
  layoutByTopologyKey.delete(key);
  layoutByTopologyKey.set(key, layout);

  const oldestKey = layoutByTopologyKey.keys().next().value;
  if (layoutByTopologyKey.size > MAX_CACHED_LAYOUTS && oldestKey) {
    layoutByTopologyKey.delete(oldestKey);
  }
}
