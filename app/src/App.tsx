import "@xyflow/react/dist/style.css";

import { Background, Controls, MiniMap, ReactFlow, type NodeMouseHandler } from "@xyflow/react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  CircleDot,
  Clock3,
  Code2,
  Database,
  FileText,
  GitBranch,
  Highlighter,
  Link2,
  Loader2,
  MessageSquare,
  Pin,
  PinOff,
  Radio,
  RefreshCw,
  Search,
  ShieldAlert,
  Sparkles,
  Zap,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { buildGraphElements, type GraphElements } from "@/lib/graphElements";
import { cn } from "@/lib/utils";
import {
  getVisualizerBootstrap,
  isTauriRuntime,
  loadVisualizationSnapshot,
  recordVisualAgentEvent,
  startRuntimeWatch,
  subscribeWatchEvents,
} from "@/lib/visualizerApi";
import type {
  ArchitectureNode,
  ChangeCluster,
  CodexSessionSummary,
  FocusSignal,
  NormalizedSessionEvent,
  SessionEventKind,
  SessionVisualizationSnapshot,
  SessionWatchEventPayload,
  VisualAgentEvent,
} from "@/types/visualization";

function App() {
  const [snapshot, setSnapshot] = useState<SessionVisualizationSnapshot | null>(null);
  const [runtimeHome, setRuntimeHome] = useState<string | undefined>();
  const [selectedEventId, setSelectedEventId] = useState<string | undefined>();
  const [selectedNodeId, setSelectedNodeId] = useState<string | undefined>();
  const [pinnedNodeIds, setPinnedNodeIds] = useState<Set<string>>(() => new Set());
  const [watchEvents, setWatchEvents] = useState<SessionWatchEventPayload[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | undefined>();

  const refresh = useCallback(
    async (nextRuntimeHome = runtimeHome) => {
      setLoading(true);
      setError(undefined);
      try {
        const nextSnapshot = await loadVisualizationSnapshot(nextRuntimeHome);
        setSnapshot(nextSnapshot);
        setRuntimeHome(nextSnapshot.runtimeHome);
        setSelectedEventId(
          (current) => current ?? nextSnapshot.events[nextSnapshot.events.length - 1]?.id
        );
      } catch (loadError) {
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      } finally {
        setLoading(false);
      }
    },
    [runtimeHome]
  );

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    async function boot() {
      try {
        const bootstrap = await getVisualizerBootstrap();
        if (disposed) {
          return;
        }
        const candidate = bootstrap.runtimeHomeCandidates[0];
        setRuntimeHome(candidate?.path);
        const nextSnapshot = await loadVisualizationSnapshot(candidate?.path);
        if (disposed) {
          return;
        }
        setSnapshot(nextSnapshot);
        setSelectedEventId(nextSnapshot.events[nextSnapshot.events.length - 1]?.id);
        await startRuntimeWatch(nextSnapshot.runtimeHome);
        unsubscribe = await subscribeWatchEvents((event) => {
          setWatchEvents((current) => [event, ...current].slice(0, 8));
          void loadVisualizationSnapshot(nextSnapshot.runtimeHome).then((liveSnapshot) => {
            if (!disposed) {
              setSnapshot(liveSnapshot);
            }
          });
        });
      } catch (bootError) {
        if (!disposed) {
          setError(bootError instanceof Error ? bootError.message : String(bootError));
          setLoading(false);
        }
        return;
      }

      if (!disposed) {
        setLoading(false);
      }
    }

    void boot();

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  const selectedEvent = useMemo(
    () => snapshot?.events.find((event) => event.id === selectedEventId),
    [selectedEventId, snapshot?.events]
  );
  const selectedCluster = useMemo(
    () =>
      snapshot?.changeClusters.find((cluster) =>
        selectedEvent ? cluster.evidenceEventIds.includes(selectedEvent.id) : false
      ) ?? snapshot?.changeClusters[0],
    [selectedEvent, snapshot?.changeClusters]
  );
  const selectedNode = useMemo(
    () => snapshot?.graph.nodes.find((node) => node.id === selectedNodeId),
    [selectedNodeId, snapshot?.graph.nodes]
  );

  const highlightedNodeIds = useMemo(() => {
    if (!snapshot) {
      return new Set<string>();
    }

    const nodeIds = new Set<string>();
    const focusByPath = new Map(snapshot.focusSignals.map((signal) => [signal.path, signal]));
    for (const path of selectedEvent?.pathMentions ?? []) {
      const absolute = absolutePath(path, snapshot.workspacePath);
      const signal = focusByPath.get(absolute);
      if (signal?.nodeId) {
        nodeIds.add(signal.nodeId);
      }
      const graphNode = snapshot.graph.nodes.find((node) => node.path === absolute);
      if (graphNode) {
        nodeIds.add(graphNode.id);
      }
    }
    for (const nodeId of selectedCluster?.nodeIds ?? []) {
      nodeIds.add(nodeId);
    }
    for (const nodeId of pinnedNodeIds) {
      nodeIds.add(nodeId);
    }

    return nodeIds;
  }, [pinnedNodeIds, selectedCluster?.nodeIds, selectedEvent?.pathMentions, snapshot]);

  const graphElements = useMemo(() => {
    if (!snapshot) {
      return { nodes: [], edges: [] } satisfies GraphElements;
    }

    return buildGraphElements(snapshot, highlightedNodeIds, pinnedNodeIds);
  }, [highlightedNodeIds, pinnedNodeIds, snapshot]);

  const onNodeClick = useCallback<NodeMouseHandler>((_event, node) => {
    setSelectedNodeId(node.id);
  }, []);

  const togglePinnedNode = useCallback(() => {
    if (!selectedNodeId) {
      return;
    }
    setPinnedNodeIds((current) => {
      const next = new Set(current);
      if (next.has(selectedNodeId)) {
        next.delete(selectedNodeId);
      } else {
        next.add(selectedNodeId);
      }
      return next;
    });
  }, [selectedNodeId]);

  const addCheckpoint = useCallback(async () => {
    if (!snapshot) {
      return;
    }
    const event = await recordVisualAgentEvent({
      phase: "checkpoint",
      kind: "decision_marker",
      label: "Reviewer checkpoint",
      visualStyle: "timeline_marker",
      visualTargetHints: selectedNode?.path ? [selectedNode.path] : [],
      summary: "Manual marker created from the observer UI.",
    });
    setSnapshot((current) =>
      current
        ? {
            ...current,
            visualAgentEvents: [event, ...current.visualAgentEvents],
          }
        : current
    );
  }, [selectedNode?.path, snapshot]);

  if (!snapshot && loading) {
    return (
      <main className="bg-background text-foreground flex min-h-screen items-center justify-center">
        <Loader2 className="text-muted-foreground size-6 animate-spin" />
      </main>
    );
  }

  if (!snapshot) {
    return (
      <main className="bg-background text-foreground min-h-screen p-6">
        <section className="border-destructive/30 bg-destructive/5 mx-auto max-w-2xl rounded-lg border p-4">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <AlertTriangle className="size-4" />
            Load failed
          </div>
          <p className="text-muted-foreground mt-2 text-sm">{error}</p>
        </section>
      </main>
    );
  }

  const activeSession = snapshot.sessions.find(
    (session) => session.id === snapshot.activeSessionId
  );
  const pinned = selectedNodeId ? pinnedNodeIds.has(selectedNodeId) : false;

  return (
    <VisualizerScreen
      activeSession={activeSession}
      addCheckpoint={addCheckpoint}
      error={error}
      graphElements={graphElements}
      loading={loading}
      onNodeClick={onNodeClick}
      onSelectEvent={setSelectedEventId}
      pinned={pinned}
      refresh={refresh}
      runtimeHome={runtimeHome}
      selectedCluster={selectedCluster}
      selectedEvent={selectedEvent}
      selectedEventId={selectedEventId}
      selectedNode={selectedNode}
      selectedNodeId={selectedNodeId}
      snapshot={snapshot}
      togglePinnedNode={togglePinnedNode}
      watchEvents={watchEvents}
    />
  );
}

function VisualizerScreen({
  activeSession,
  addCheckpoint,
  error,
  graphElements,
  loading,
  onNodeClick,
  onSelectEvent,
  pinned,
  refresh,
  runtimeHome,
  selectedCluster,
  selectedEvent,
  selectedEventId,
  selectedNode,
  selectedNodeId,
  snapshot,
  togglePinnedNode,
  watchEvents,
}: {
  activeSession?: CodexSessionSummary;
  addCheckpoint: () => Promise<void>;
  error?: string;
  graphElements: GraphElements;
  loading: boolean;
  onNodeClick: NodeMouseHandler;
  onSelectEvent: (eventId: string) => void;
  pinned: boolean;
  refresh: () => Promise<void>;
  runtimeHome?: string;
  selectedCluster?: ChangeCluster;
  selectedEvent?: NormalizedSessionEvent;
  selectedEventId?: string;
  selectedNode?: ArchitectureNode;
  selectedNodeId?: string;
  snapshot: SessionVisualizationSnapshot;
  togglePinnedNode: () => void;
  watchEvents: SessionWatchEventPayload[];
}) {
  return (
    <main className="bg-background text-foreground min-h-screen overflow-hidden">
      <header className="border-border bg-background/95 grid min-h-16 grid-cols-[minmax(260px,1.1fr)_minmax(300px,1.4fr)_minmax(260px,1fr)] items-center gap-4 border-b px-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Activity className="size-4 text-cyan-600" />
            <h1 className="truncate text-sm font-semibold">Codex Analysis Visualizer</h1>
            <span
              className={cn(
                "rounded-full px-2 py-0.5 text-[11px] font-medium",
                isTauriRuntime() ? "bg-emerald-50 text-emerald-700" : "bg-amber-50 text-amber-700"
              )}>
              {snapshot.sourceMode}
            </span>
          </div>
          <div className="text-muted-foreground mt-1 flex min-w-0 items-center gap-2 text-xs">
            <Code2 className="size-3" />
            <span className="truncate">
              {relativePath(snapshot.workspacePath, snapshot.workspacePath)}
            </span>
          </div>
        </div>

        <div className="grid min-w-0 grid-cols-3 gap-3 text-xs">
          <Metric label="Events" value={snapshot.events.length.toString()} tone="cyan" />
          <Metric label="Focus" value={snapshot.focusSignals.length.toString()} tone="emerald" />
          <Metric label="Clusters" value={snapshot.changeClusters.length.toString()} tone="amber" />
        </div>

        <div className="flex min-w-0 items-center justify-end gap-2">
          <div className="text-muted-foreground min-w-0 text-right text-xs">
            <div className="truncate">{runtimeHome ?? snapshot.runtimeHome}</div>
            <div>{formatTime(snapshot.generatedAtMs)}</div>
          </div>
          <Button
            aria-label="Refresh"
            title="Refresh"
            size="icon"
            variant="outline"
            onClick={() => void refresh()}>
            <RefreshCw className={cn("size-4", loading && "animate-spin")} />
          </Button>
          <Button
            aria-label="Add checkpoint"
            title="Add checkpoint"
            size="icon"
            variant="outline"
            onClick={() => void addCheckpoint()}>
            <Zap className="size-4" />
          </Button>
        </div>
      </header>

      <div className="grid h-[calc(100vh-4rem)] grid-cols-[330px_minmax(420px,1fr)_360px]">
        <aside className="border-border flex min-h-0 flex-col border-r">
          <PanelHeader icon={<Database className="size-4" />} title="Sessions" />
          <div className="border-border min-h-0 border-b p-3">
            {snapshot.sessions.slice(0, 4).map((session) => (
              <SessionRow
                key={session.id}
                active={session.id === snapshot.activeSessionId}
                session={session}
                workspacePath={snapshot.workspacePath}
              />
            ))}
          </div>

          <PanelHeader icon={<Clock3 className="size-4" />} title="Timeline" />
          <div className="min-h-0 flex-1 overflow-auto px-3 pb-3">
            {snapshot.events.map((event) => (
              <TimelineRow
                key={event.id}
                event={event}
                selected={event.id === selectedEventId}
                workspacePath={snapshot.workspacePath}
                onSelect={() => onSelectEvent(event.id)}
              />
            ))}
          </div>
        </aside>

        <section className="relative min-h-0">
          <div className="border-border absolute inset-x-0 top-0 z-10 flex h-12 items-center justify-between border-b bg-white/90 px-3 backdrop-blur">
            <div className="flex min-w-0 items-center gap-2">
              <Highlighter className="size-4 text-cyan-700" />
              <span className="text-sm font-semibold">Context Graph</span>
              <span className="text-muted-foreground text-xs">
                {graphElements.nodes.length} nodes / {graphElements.edges.length} edges
              </span>
            </div>
            <Button
              aria-label={pinned ? "Unpin selected node" : "Pin selected node"}
              title={pinned ? "Unpin selected node" : "Pin selected node"}
              size="icon-sm"
              variant="outline"
              disabled={!selectedNodeId}
              onClick={togglePinnedNode}>
              {pinned ? <PinOff className="size-4" /> : <Pin className="size-4" />}
            </Button>
          </div>
          <div className="h-full pt-12">
            <ReactFlow
              nodes={graphElements.nodes}
              edges={graphElements.edges}
              fitView
              minZoom={0.35}
              maxZoom={1.5}
              nodesDraggable={false}
              onNodeClick={onNodeClick}>
              <Background color="#e5e7eb" gap={18} />
              <MiniMap pannable zoomable nodeStrokeWidth={3} />
              <Controls showInteractive={false} />
            </ReactFlow>
          </div>
        </section>

        <aside className="border-border flex min-h-0 flex-col border-l">
          <PanelHeader icon={<Sparkles className="size-4" />} title="Bridge" />
          <div className="min-h-0 flex-1 overflow-auto p-3">
            <ActiveSummary
              activeSession={activeSession}
              selectedEvent={selectedEvent}
              selectedNode={selectedNode}
              selectedCluster={selectedCluster}
              workspacePath={snapshot.workspacePath}
            />
            <BridgeEvents
              events={snapshot.visualAgentEvents}
              workspacePath={snapshot.workspacePath}
            />
            <FocusList signals={snapshot.focusSignals} workspacePath={snapshot.workspacePath} />
            <WatchPanel
              watchTargets={snapshot.watchPlan?.watchTargets ?? []}
              watchEvents={watchEvents}
              workspacePath={snapshot.workspacePath}
            />
            <Diagnostics diagnostics={snapshot.diagnostics} error={error} />
          </div>
        </aside>
      </div>
    </main>
  );
}

function Metric({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "cyan" | "emerald" | "amber";
}) {
  const toneClass = {
    cyan: "border-cyan-200 bg-cyan-50 text-cyan-800",
    emerald: "border-emerald-200 bg-emerald-50 text-emerald-800",
    amber: "border-amber-200 bg-amber-50 text-amber-800",
  }[tone];

  return (
    <div className={cn("rounded-md border px-3 py-2", toneClass)}>
      <div className="text-[11px] font-medium">{label}</div>
      <div className="text-lg leading-5 font-semibold">{value}</div>
    </div>
  );
}

function PanelHeader({ icon, title }: { icon: ReactNode; title: string }) {
  return (
    <div className="border-border bg-muted/30 flex h-10 shrink-0 items-center gap-2 border-b px-3 text-sm font-semibold">
      {icon}
      {title}
    </div>
  );
}

function SessionRow({
  session,
  active,
  workspacePath,
}: {
  session: CodexSessionSummary;
  active: boolean;
  workspacePath: string;
}) {
  return (
    <div
      className={cn(
        "mb-2 rounded-md border p-2",
        active ? "border-cyan-300 bg-cyan-50/60" : "border-border bg-background"
      )}>
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0 truncate text-sm font-medium">{session.title || session.id}</div>
        <span
          className={cn(
            "rounded-full px-2 py-0.5 text-[10px] font-semibold",
            statusClass(session.status)
          )}>
          {session.status}
        </span>
      </div>
      <div className="text-muted-foreground mt-1 flex items-center gap-1 text-xs">
        <GitBranch className="size-3" />
        <span className="truncate">{session.gitBranch ?? session.source}</span>
      </div>
      <div className="text-muted-foreground mt-1 truncate text-xs">
        {relativePath(session.cwd, workspacePath)}
      </div>
    </div>
  );
}

function TimelineRow({
  event,
  selected,
  workspacePath,
  onSelect,
}: {
  event: NormalizedSessionEvent;
  selected: boolean;
  workspacePath: string;
  onSelect: () => void;
}) {
  const Icon = eventIcon(event.kind);

  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "group/timeline border-border hover:bg-muted/40 grid w-full grid-cols-[28px_1fr] gap-2 border-b py-3 text-left transition-colors",
        selected && "bg-cyan-50"
      )}>
      <span
        className={cn(
          "mt-0.5 flex size-6 items-center justify-center rounded-full",
          eventTone(event.kind)
        )}>
        <Icon className="size-3.5" />
      </span>
      <span className="min-w-0">
        <span className="flex items-center justify-between gap-2">
          <span className="truncate text-xs font-semibold">{event.title}</span>
          <span className="text-muted-foreground shrink-0 text-[10px]">
            {formatTime(event.timestampMs)}
          </span>
        </span>
        <span className="text-muted-foreground mt-1 line-clamp-2 text-xs">{event.summary}</span>
        {event.pathMentions.length > 0 && (
          <span className="mt-2 flex flex-wrap gap-1">
            {event.pathMentions.slice(0, 3).map((path) => (
              <span key={path} className="bg-muted rounded px-1.5 py-0.5 text-[10px]">
                {relativePath(absolutePath(path, workspacePath), workspacePath)}
              </span>
            ))}
          </span>
        )}
      </span>
    </button>
  );
}

function ActiveSummary({
  activeSession,
  selectedEvent,
  selectedNode,
  selectedCluster,
  workspacePath,
}: {
  activeSession?: CodexSessionSummary;
  selectedEvent?: NormalizedSessionEvent;
  selectedNode?: ArchitectureNode;
  selectedCluster?: ChangeCluster;
  workspacePath: string;
}) {
  return (
    <section className="border-border mb-3 rounded-md border p-3">
      <div className="flex items-center gap-2 text-sm font-semibold">
        <CircleDot className="size-4 text-emerald-600" />
        Active Thread
      </div>
      <div className="mt-2 min-w-0 text-sm font-medium">
        {activeSession?.title ?? "No active session"}
      </div>
      <div className="text-muted-foreground mt-1 truncate text-xs">{activeSession?.id}</div>

      <div className="border-border mt-3 border-t pt-3">
        <DetailLine label="Event" value={selectedEvent?.title} />
        <DetailLine
          label="Node"
          value={
            selectedNode
              ? relativePath(selectedNode.path ?? selectedNode.label, workspacePath)
              : undefined
          }
        />
        <DetailLine label="Cluster" value={selectedCluster?.title} />
        {selectedCluster?.summary && (
          <p className="text-muted-foreground mt-2 text-xs">{selectedCluster.summary}</p>
        )}
      </div>
    </section>
  );
}

function BridgeEvents({
  events,
  workspacePath,
}: {
  events: VisualAgentEvent[];
  workspacePath: string;
}) {
  return (
    <section className="border-border mb-3 rounded-md border">
      <div className="border-border flex items-center gap-2 border-b px-3 py-2 text-sm font-semibold">
        <Link2 className="size-4" />
        Visual Events
      </div>
      <div className="divide-border divide-y">
        {events.slice(0, 6).map((event) => (
          <div key={event.id} className="p-3">
            <div className="flex items-center justify-between gap-2">
              <span className="truncate text-xs font-semibold">{event.label}</span>
              <span
                className={cn(
                  "rounded-full px-2 py-0.5 text-[10px] font-medium",
                  phaseClass(event.phase)
                )}>
                {event.phase}
              </span>
            </div>
            {event.summary && (
              <div className="text-muted-foreground mt-1 text-xs">{event.summary}</div>
            )}
            {event.visualTargetHints.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1">
                {event.visualTargetHints.slice(0, 3).map((hint) => (
                  <span key={hint} className="bg-muted rounded px-1.5 py-0.5 text-[10px]">
                    {relativePath(absolutePath(hint, workspacePath), workspacePath)}
                  </span>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

function FocusList({ signals, workspacePath }: { signals: FocusSignal[]; workspacePath: string }) {
  return (
    <section className="border-border mb-3 rounded-md border">
      <div className="border-border flex items-center gap-2 border-b px-3 py-2 text-sm font-semibold">
        <Search className="size-4" />
        Focus Signals
      </div>
      <div className="divide-border divide-y">
        {signals.slice(0, 7).map((signal) => (
          <div key={signal.id} className="grid grid-cols-[1fr_44px] gap-2 p-3">
            <div className="min-w-0">
              <div className="truncate text-xs font-medium">
                {signal.path ? relativePath(signal.path, workspacePath) : signal.symbol}
              </div>
              <div className="text-muted-foreground mt-1 truncate text-[11px]">{signal.source}</div>
            </div>
            <div className="text-right text-xs font-semibold">{Math.round(signal.score * 100)}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

function WatchPanel({
  watchTargets,
  watchEvents,
  workspacePath,
}: {
  watchTargets: { path: string; exists: boolean; recursive: boolean; reason: string }[];
  watchEvents: SessionWatchEventPayload[];
  workspacePath: string;
}) {
  return (
    <section className="border-border mb-3 rounded-md border">
      <div className="border-border flex items-center gap-2 border-b px-3 py-2 text-sm font-semibold">
        <Radio className="size-4" />
        Runtime Watch
      </div>
      <div className="p-3">
        <div className="grid grid-cols-2 gap-2">
          {watchTargets.slice(0, 4).map((target) => (
            <div
              key={`${target.path}:${target.reason}`}
              className="border-border rounded-md border p-2">
              <div className="flex items-center justify-between gap-1">
                <span className="truncate text-[11px] font-medium">
                  {relativePath(target.path, workspacePath)}
                </span>
                {target.exists ? (
                  <CheckCircle2 className="size-3 text-emerald-600" />
                ) : (
                  <ShieldAlert className="size-3 text-amber-600" />
                )}
              </div>
              <div className="text-muted-foreground mt-1 text-[10px]">
                {target.recursive ? "recursive" : "file"}
              </div>
            </div>
          ))}
        </div>
        {watchEvents.length > 0 && (
          <div className="mt-3 space-y-2">
            {watchEvents.slice(0, 3).map((event) => (
              <div
                key={`${event.timestampMs}:${event.changedPaths.join("|")}`}
                className="bg-muted/50 rounded-md p-2">
                <div className="truncate text-[11px] font-medium">
                  {relativePath(event.changedPaths[0] ?? event.runtimeHome, workspacePath)}
                </div>
                <div className="text-muted-foreground mt-1 text-[10px]">
                  {formatTime(event.timestampMs)}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function Diagnostics({ diagnostics, error }: { diagnostics: string[]; error?: string }) {
  if (diagnostics.length === 0 && !error) {
    return null;
  }

  return (
    <section className="border-border rounded-md border">
      <div className="border-border flex items-center gap-2 border-b px-3 py-2 text-sm font-semibold">
        <AlertTriangle className="size-4" />
        Diagnostics
      </div>
      <div className="space-y-2 p-3">
        {error && <p className="text-destructive text-xs">{error}</p>}
        {diagnostics.map((diagnostic) => (
          <p key={diagnostic} className="text-muted-foreground text-xs">
            {diagnostic}
          </p>
        ))}
      </div>
    </section>
  );
}

function DetailLine({ label, value }: { label: string; value?: string }) {
  return (
    <div className="grid grid-cols-[72px_1fr] gap-2 py-1 text-xs">
      <span className="text-muted-foreground">{label}</span>
      <span className="min-w-0 truncate">{value ?? "-"}</span>
    </div>
  );
}

function absolutePath(path: string, workspacePath: string) {
  if (path.startsWith("/") || path.startsWith("~")) {
    return path;
  }
  return `${workspacePath}/${path}`.replace(/\/+/g, "/");
}

function relativePath(path: string, workspacePath: string) {
  if (path === workspacePath) {
    return ".";
  }
  if (path.startsWith(`${workspacePath}/`)) {
    return path.slice(workspacePath.length + 1);
  }
  return path.replace(/^\/Users\/[^/]+\//, "~/");
}

function formatTime(timestampMs: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(timestampMs));
}

function statusClass(status: string) {
  switch (status) {
    case "active":
      return "bg-emerald-50 text-emerald-700";
    case "recent":
      return "bg-cyan-50 text-cyan-700";
    default:
      return "bg-slate-100 text-slate-600";
  }
}

function phaseClass(phase: string) {
  switch (phase) {
    case "before_edit":
      return "bg-amber-50 text-amber-700";
    case "after_edit":
      return "bg-emerald-50 text-emerald-700";
    default:
      return "bg-cyan-50 text-cyan-700";
  }
}

function eventTone(kind: SessionEventKind) {
  switch (kind) {
    case "user_message":
    case "assistant_message":
      return "bg-sky-50 text-sky-700";
    case "command":
    case "tool_call":
      return "bg-violet-50 text-violet-700";
    case "patch":
      return "bg-emerald-50 text-emerald-700";
    case "error":
      return "bg-red-50 text-red-700";
    case "watch":
      return "bg-amber-50 text-amber-700";
    default:
      return "bg-slate-100 text-slate-600";
  }
}

function eventIcon(kind: SessionEventKind) {
  switch (kind) {
    case "user_message":
    case "assistant_message":
      return MessageSquare;
    case "command":
      return Code2;
    case "tool_call":
      return Search;
    case "tool_output":
      return FileText;
    case "patch":
      return CheckCircle2;
    case "error":
      return AlertTriangle;
    case "watch":
      return Radio;
    default:
      return Clock3;
  }
}

export default App;
