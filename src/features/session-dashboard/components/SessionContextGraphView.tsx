import {
  Handle,
  Position,
  ReactFlow,
  type NodeProps,
  useEdgesState,
  useNodesState,
} from "@xyflow/react";
import { FileCode2, FolderTree, SearchCode } from "lucide-react";
import { useEffect } from "react";

import type {
  ContextGraphNode,
  ContextGraphNodeData,
} from "@/features/session-dashboard/context-graph/types";
import { useSessionContextGraph } from "@/features/session-dashboard/context-graph/useSessionContextGraph";
import {
  type ActivitySectionKey,
  type AgentSessionFileActivity,
  type AgentSessionSummary,
  type SelectedActivityFile,
} from "@/lib/session-watch";
import { cn } from "@/lib/utils";

const nodeTypes = {
  contextGraphNode: ContextGraphNodeComponent,
};

export function SessionContextGraphView({
  fileActivity,
  isFileActivityLoading,
  selectedActivityFile,
  selectedSession,
  onSelectFile,
}: {
  fileActivity: AgentSessionFileActivity;
  isFileActivityLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  selectedSession: AgentSessionSummary | null;
  onSelectFile: (selection: SelectedActivityFile) => void;
}) {
  const selectedFilePath = selectedActivityFile?.filePath ?? "";
  const { contextGraph, errorMessage, isLoading } = useSessionContextGraph({
    fileActivity,
    includeEntireWorkspace: false,
    pinnedFilePaths: [],
    selectedFilePath,
    selectedSession,
  });
  const [nodes, setNodes, onNodesChange] = useNodesState<ContextGraphNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState(
    contextGraph ? [...contextGraph.containsEdges, ...contextGraph.impactEdges] : []
  );

  useEffect(() => {
    setNodes(contextGraph?.nodes ?? []);
    setEdges(contextGraph ? [...contextGraph.containsEdges, ...contextGraph.impactEdges] : []);
  }, [contextGraph, setEdges, setNodes]);

  function handleNodeClick(node: ContextGraphNode) {
    if (node.data.kind !== "file") {
      return;
    }

    const activityKey = resolvePrimaryActivity(node.data.activities);
    onSelectFile({ activityKey, filePath: node.data.displayPath });
  }

  const isGraphLoading = isFileActivityLoading || isLoading;

  return (
    <section>
      <div className="context-graph-shell">
        {selectedSession?.cwd ? (
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            nodesDraggable={false}
            nodesConnectable={false}
            elementsSelectable
            fitView
            fitViewOptions={{ padding: 0.22 }}
            minZoom={0.2}
            maxZoom={1.8}
            proOptions={{ hideAttribution: true }}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onNodeClick={(_event, node) => handleNodeClick(node as ContextGraphNode)}
          />
        ) : (
          <GraphEmptyState
            title="No workspace path"
            description="This session does not expose a workspace yet, so the file tree cannot be indexed."
          />
        )}

        {isGraphLoading ? (
          <div className="context-graph-status">Indexing and laying out...</div>
        ) : null}
        {errorMessage ? <div className="context-graph-error">{errorMessage}</div> : null}
        {selectedSession?.cwd && nodes.length === 0 && !isGraphLoading ? (
          <GraphEmptyState
            title="No graph nodes"
            description="No session files matched the indexed workspace graph yet."
          />
        ) : null}
      </div>
    </section>
  );
}

function ContextGraphNodeComponent({ data }: NodeProps<ContextGraphNode>) {
  const Icon =
    data.kind === "repo" ? SearchCode : data.kind === "directory" ? FolderTree : FileCode2;

  return (
    <div
      className={cn(
        "context-graph-node",
        data.kind === "repo" && "context-graph-node-repo",
        data.kind === "directory" && "context-graph-node-directory",
        data.kind === "file" && "context-graph-node-file",
        primaryActivityClass(data.activities),
        data.isSelected && "context-graph-node-selected"
      )}>
      <Handle className="context-graph-handle" type="target" position={Position.Left} />
      <Handle className="context-graph-handle" type="source" position={Position.Right} />
      <div className="flex items-start gap-2">
        <Icon className="mt-0.5 size-4 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{data.label}</p>
          <p className="text-muted-foreground truncate text-[11px]">{data.displayPath}</p>
        </div>
      </div>
    </div>
  );
}

function GraphEmptyState({ title, description }: { title: string; description: string }) {
  return (
    <div className="context-graph-empty">
      <p className="text-sm font-medium">{title}</p>
      <p className="text-muted-foreground mt-1 max-w-md text-sm">{description}</p>
    </div>
  );
}

function resolvePrimaryActivity(
  activities: ContextGraphNodeData["activities"]
): ActivitySectionKey {
  return activities[0] ?? "impacted";
}

function primaryActivityClass(activities: ContextGraphNodeData["activities"]) {
  if (activities.length === 0) {
    return "";
  }

  switch (resolvePrimaryActivity(activities)) {
    case "edited":
      return "context-graph-node-activity-edited";
    case "deleted":
      return "context-graph-node-activity-deleted";
    case "impacted":
      return "context-graph-node-activity-impacted";
    case "read":
      return "";
  }
}
