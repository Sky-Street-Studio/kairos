import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MarkerType,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useNodesState,
  useReactFlow,
  type Connection,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { CircleAlert, Flag, Wand2 } from "lucide-react";
import { useEffect, useMemo } from "react";
import { toast } from "sonner";

import { useResolvedAppearance } from "@/hooks/use-theme";
import { cn } from "@/lib/utils";
import { appStore, useApp } from "@/store";
import type { EdgeKind } from "@shared/types";

import { ghostTargets, positionsFor, withSelection } from "./graph-model";
import { NODE_HEIGHT, NODE_WIDTH, edgeId, mergeEdges } from "./layout";

type LabelData = { label: string; path: string; line: number; start: boolean; problems: number };
type LabelNodeType = Node<LabelData, "label">;

function LabelNode({ data, selected }: NodeProps<LabelNodeType>) {
  return (
    <div
      style={{ width: NODE_WIDTH, height: NODE_HEIGHT }}
      className={cn(
        "group relative flex flex-col justify-center rounded-lg border bg-card px-3 shadow-[0_6px_18px_-10px_rgb(0_0_0/0.6)] transition-[box-shadow,border-color] duration-150",
        selected ? "border-primary shadow-[0_0_0_3px_var(--selection)]" : "hover:border-foreground/25",
      )}
      data-testid="graph-node"
    >
      {data.start && <span className="bg-accent-gradient absolute inset-x-3 -top-px h-0.5 rounded-full" />}
      <Handle type="target" position={Position.Top} className="!h-2 !w-2 !border-2 opacity-0 group-hover:opacity-100" />
      <div className="flex items-center gap-1.5">
        {data.start && <Flag className="size-3 shrink-0 text-primary" aria-label="start label" />}
        <span className="truncate font-mono text-[12.5px] font-medium">{data.label}</span>
        {data.problems > 0 && (
          <span className="ml-auto flex items-center gap-0.5 text-2xs text-destructive">
            <CircleAlert className="size-3" />
            {data.problems}
          </span>
        )}
      </div>
      <span className="truncate text-2xs text-muted-foreground tabular">
        {data.path.replace(/^script\//, "")}:{data.line}
      </span>
      <Handle
        type="source"
        position={Position.Bottom}
        className="!h-2.5 !w-2.5 !border-2 opacity-40 transition-opacity group-hover:opacity-100"
      />
    </div>
  );
}

type GhostData = { label: string };
type GhostNodeType = Node<GhostData, "ghost">;
type GraphNodeType = LabelNodeType | GhostNodeType;

const GHOST_OFFSET_Y = 160;

function GhostNode({ data }: NodeProps<GhostNodeType>) {
  return (
    <div
      style={{ width: NODE_WIDTH, height: NODE_HEIGHT }}
      className="relative flex items-center justify-center rounded-lg border border-dashed border-destructive/70 bg-card/60 px-3 text-center"
      data-testid="graph-ghost"
    >
      <Handle type="target" position={Position.Top} className="!h-2 !w-2 !border-2 opacity-0" />
      <span className="truncate text-2xs text-destructive">Missing scene: {data.label}</span>
    </div>
  );
}

const nodeTypes = { label: LabelNode, ghost: GhostNode };

const EDGE_STYLE: Record<EdgeKind, { stroke: string; dash?: string; width: number }> = {
  jump: { stroke: "color-mix(in oklab, var(--foreground) 50%, transparent)", width: 1.5 },
  choice: { stroke: "var(--primary)", width: 1.5 },
  call: { stroke: "color-mix(in oklab, var(--foreground) 50%, transparent)", dash: "5 4", width: 1.5 },
  fallthrough: { stroke: "color-mix(in oklab, var(--foreground) 28%, transparent)", dash: "2 4", width: 1.25 },
};

const LEGEND: { kind: EdgeKind; label: string }[] = [
  { kind: "choice", label: "menu choice" },
  { kind: "jump", label: "jump" },
  { kind: "call", label: "call" },
  { kind: "fallthrough", label: "falls through" },
];

const truncate = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

function Graph() {
  const graph = useApp((s) => s.analysis.graph);
  const diagnostics = useApp((s) => s.analysis.diagnostics);
  const start = useApp((s) => s.project?.manifest.start);
  const openFile = useApp((s) => s.openFile);
  const edit = useApp((s) => s.edit);
  const graphLayout = useApp((s) => s.graphLayout);
  const setNodePositions = useApp((s) => s.setNodePositions);
  const resetGraphLayout = useApp((s) => s.resetGraphLayout);
  const { fitView } = useReactFlow();
  const appearance = useResolvedAppearance();

  const flow = useMemo(() => {
    const positions = positionsFor(graph, graphLayout.graph);
    const labelLines = new Map(graph.nodes.map((n) => [n.label, n]));
    const problems = (label: string) => {
      const n = labelLines.get(label)!;
      const later = graph.nodes
        .filter((m) => m.path === n.path && m.line > n.line)
        .reduce((min, m) => Math.min(min, m.line), Infinity);
      return diagnostics.filter((d) => d.path === n.path && d.line >= n.line && d.line < later).length;
    };
    const labelNodes: LabelNodeType[] = graph.nodes.map((n) => ({
      id: n.label,
      type: "label",
      position: positions.get(n.label) ?? { x: 0, y: 0 },
      data: { ...n, start: n.label === start, problems: problems(n.label) },
    }));
    const ghostsPerSource = new Map<string, number>();
    const ghostNodes: GhostNodeType[] = ghostTargets(graph, graph.missing).map((label) => {
      const source = graph.missing.find((e) => e.to === label)!.from;
      const slot = ghostsPerSource.get(source) ?? 0;
      ghostsPerSource.set(source, slot + 1);
      const from = positions.get(source) ?? { x: 0, y: 0 };
      return {
        id: `missing:${label}`,
        type: "ghost",
        position: { x: from.x + slot * (NODE_WIDTH + 24), y: from.y + GHOST_OFFSET_Y },
        data: { label },
        draggable: false,
        selectable: false,
        connectable: false,
      };
    });
    const ghostEdges: Edge[] = mergeEdges(graph.missing).map((e, i) => ({
      id: `missing-${edgeId(e, i)}`,
      source: e.from,
      target: `missing:${e.to}`,
      style: { stroke: "var(--destructive)", strokeWidth: 1.5, strokeDasharray: "5 4" },
      markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14, color: "var(--destructive)" },
      selectable: false,
    }));
    const flowNodes: GraphNodeType[] = [...labelNodes, ...ghostNodes];
    const flowEdges: Edge[] = mergeEdges(graph.edges).map((e, i) => {
      const style = EDGE_STYLE[e.kind];
      return {
        id: edgeId(e, i),
        source: e.from,
        target: e.to,
        label: e.kind === "choice" && e.text ? truncate(e.text, 26) : e.kind === "call" ? "call" : undefined,
        labelBgPadding: [6, 3] as [number, number],
        labelBgBorderRadius: 4,
        style: { stroke: style.stroke, strokeWidth: style.width, strokeDasharray: style.dash },
        markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14, color: style.stroke },
        animated: false,
        data: { kind: e.kind },
      };
    });
    return { nodes: flowNodes, edges: [...flowEdges, ...ghostEdges] };
  }, [graph, diagnostics, start, graphLayout]);

  const [rfNodes, setNodes, onNodesChange] = useNodesState<GraphNodeType>(flow.nodes);
  useEffect(() => setNodes((prev) => withSelection(flow.nodes, prev)), [flow.nodes, setNodes]);

  const shape = graph.nodes.map((n) => n.label).join("|");
  useEffect(() => {
    if (!shape) return;
    const id = requestAnimationFrame(() => void fitView({ padding: 0.18, duration: 240, maxZoom: 1.1 }));
    return () => cancelAnimationFrame(id);
  }, [shape, fitView]);

  const tidyUp = () => {
    resetGraphLayout();
    requestAnimationFrame(() => void fitView({ padding: 0.18, duration: 240, maxZoom: 1.1 }));
  };

  const saveDragged = (dragged: Node[]) => {
    const labels = dragged.filter((n) => n.type === "label");
    if (labels.length === 0) return;
    setNodePositions(Object.fromEntries(labels.map((n) => [n.id, n.position])));
  };

  const onConnect = async (c: Connection) => {
    if (!c.source || !c.target || c.source === c.target) return;
    if (await edit([{ op: "connect", from: c.source, to: c.target, choice: null }])) {
      toast.success(`${c.source} now goes to ${c.target}`);
    } else {
      const error = appStore().getState().editError;
      if (error) toast.error("Could not connect the scenes", { description: error });
    }
  };

  if (graph.nodes.length === 0) {
    return (
      <div className="grid flex-1 place-items-center text-xs text-muted-foreground">
        No labels yet. Write <code className="font-mono text-foreground">label start:</code> in a script to begin.
      </div>
    );
  }

  return (
    <ReactFlow
      nodes={rfNodes}
      onNodesChange={onNodesChange}
      edges={flow.edges}
      nodeTypes={nodeTypes}
      onConnect={(c) => void onConnect(c)}
      onNodeClick={(_, n) => n.type === "label" && openFile(n.data.path, n.data.line)}
      onNodeDragStop={(_, __, nodes) => saveDragged(nodes)}
      selectionOnDrag
      multiSelectionKeyCode="Shift"
      panOnDrag={[1]}
      panOnScroll
      nodesConnectable
      nodesDraggable
      minZoom={0.25}
      maxZoom={2}
      proOptions={{ hideAttribution: true }}
      colorMode={appearance}
      fitView
    >
      <Background
        variant={BackgroundVariant.Dots}
        gap={20}
        size={1}
        color="color-mix(in oklab, var(--foreground) 14%, transparent)"
      />
      <Controls showInteractive={false} position="bottom-left" />
      <MiniMap
        position="bottom-right"
        pannable
        zoomable
        nodeColor="var(--muted)"
        nodeStrokeColor="var(--border)"
        style={{ width: 132, height: 88 }}
        className="!rounded-md !border"
      />
      <div className="absolute top-3 left-3 z-10 flex items-center gap-3 rounded-md border bg-popover/90 px-2.5 py-1.5 text-2xs text-muted-foreground">
        {LEGEND.map(({ kind, label }) => (
          <span key={kind} className="flex items-center gap-1.5">
            <svg width="18" height="6" aria-hidden>
              <line
                x1="0"
                y1="3"
                x2="18"
                y2="3"
                stroke={EDGE_STYLE[kind].stroke}
                strokeWidth="1.5"
                strokeDasharray={EDGE_STYLE[kind].dash}
              />
            </svg>
            {label}
          </span>
        ))}
        <span className="border-l pl-3">Scroll or hold Space and drag to move around</span>
        <span className="border-l pl-3">Drag from a node’s bottom handle to add a jump</span>
        <button
          type="button"
          onClick={tidyUp}
          className="ml-1 flex items-center gap-1 rounded border px-1.5 py-0.5 text-foreground hover:bg-accent"
        >
          <Wand2 className="size-3" />
          Tidy up
        </button>
      </div>
    </ReactFlow>
  );
}

export default function StoryGraphView() {
  return (
    <div className="relative flex min-h-0 flex-1" data-testid="story-graph">
      <ReactFlowProvider>
        <Graph />
      </ReactFlowProvider>
    </div>
  );
}
