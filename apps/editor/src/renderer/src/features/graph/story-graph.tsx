import { ContextMenuPrimitive } from "@/components/ui/context-menu";
import type { EdgeKind, GraphEdge } from "@shared/types";
import {
  Background,
  BackgroundVariant,
  type Connection,
  Controls,
  type Edge,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  MiniMap,
  type Node,
  type NodeProps,
  NodeToolbar,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import { CircleAlert, Flag, Plus, Wand2 } from "lucide-react";
import { type KeyboardEvent, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useResolvedAppearance } from "@/hooks/use-theme";
import { cn } from "@/lib/utils";
import { appStore, useApp } from "@/store";

import { findChoiceOption, ghostTargets, positionsFor, withSelection } from "./graph-model";
import { edgeId, mergeEdges, NODE_HEIGHT, NODE_WIDTH } from "./layout";
import { edgeOps, SceneDialogs, useSceneActions } from "./scene-actions";

type LabelData = {
  label: string;
  path: string;
  line: number;
  start: boolean;
  problems: number;
};
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
      {data.start && <span className="absolute inset-x-3 -top-px h-0.5 rounded-full bg-primary" />}
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

type ConnectPrompt = { from: string; to: string };

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
  jump: {
    stroke: "color-mix(in oklab, var(--foreground) 50%, transparent)",
    width: 1.5,
  },
  choice: { stroke: "var(--primary)", width: 1.5 },
  call: {
    stroke: "color-mix(in oklab, var(--foreground) 50%, transparent)",
    dash: "5 4",
    width: 1.5,
  },
  fallthrough: {
    stroke: "color-mix(in oklab, var(--foreground) 28%, transparent)",
    dash: "2 4",
    width: 1.25,
  },
};

const LEGEND: { kind: EdgeKind; label: string }[] = [
  { kind: "choice", label: "menu choice" },
  { kind: "jump", label: "jump" },
  { kind: "call", label: "call" },
  { kind: "fallthrough", label: "falls through" },
];

const truncate = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
const contextItemClass =
  "relative flex h-7 cursor-default items-center gap-2 rounded-md px-2 text-xs outline-none select-none data-[highlighted]:bg-accent [&_svg]:size-3.5 [&_svg]:text-muted-foreground";

function Graph() {
  const graph = useApp((s) => s.analysis.graph);
  const diagnostics = useApp((s) => s.analysis.diagnostics);
  const start = useApp((s) => s.project?.manifest.start);
  const graphLayout = useApp((s) => s.graphLayout);
  const setNodePositions = useApp((s) => s.setNodePositions);
  const resetGraphLayout = useApp((s) => s.resetGraphLayout);
  const openScene = useApp((s) => s.openScene);
  const actions = useSceneActions();
  const [contextLabel, setContextLabel] = useState<string | null>(null);
  const [ghostLabel, setGhostLabel] = useState<string | null>(null);
  const [connectPrompt, setConnectPrompt] = useState<ConnectPrompt | null>(null);
  const [choiceText, setChoiceText] = useState("");
  const [contextPoint, setContextPoint] = useState({ x: 0, y: 0 });
  const { fitView, screenToFlowPosition } = useReactFlow();
  const appearance = useResolvedAppearance();

  const flow = useMemo(() => {
    const positions = positionsFor(graph, graphLayout.graph);
    const labelLines = new Map(graph.nodes.map((n) => [n.label, n]));
    const problems = (label: string) => {
      const n = labelLines.get(label);
      if (!n) return 0;
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
    const ghostNodes: GhostNodeType[] = ghostTargets(graph, graph.missing).flatMap((label): GhostNodeType[] => {
      const edge = graph.missing.find((item) => item.to === label);
      if (!edge) return [];
      const slot = ghostsPerSource.get(edge.from) ?? 0;
      ghostsPerSource.set(edge.from, slot + 1);
      const from = positions.get(edge.from) ?? { x: 0, y: 0 };
      return [
        {
          id: `missing:${label}`,
          type: "ghost",
          position: {
            x: from.x + slot * (NODE_WIDTH + 24),
            y: from.y + GHOST_OFFSET_Y,
          },
          data: { label },
          draggable: false,
          selectable: false,
          connectable: false,
        },
      ];
    });
    const ghostEdges: Edge[] = mergeEdges(graph.missing).map((e, i) => ({
      id: `missing-${edgeId(e, i)}`,
      source: e.from,
      target: `missing:${e.to}`,
      style: {
        stroke: "var(--destructive)",
        strokeWidth: 1.5,
        strokeDasharray: "5 4",
      },
      markerEnd: {
        type: MarkerType.ArrowClosed,
        width: 14,
        height: 14,
        color: "var(--destructive)",
      },
      selectable: false,
    }));
    const flowNodes: GraphNodeType[] = [...labelNodes, ...ghostNodes];
    const flowEdges: Edge[] = mergeEdges(graph.edges).map((merged, i) => {
      const e =
        graph.edges.find((edge) => edge.from === merged.from && edge.to === merged.to && edge.kind === merged.kind) ??
        merged;
      const style = EDGE_STYLE[e.kind];
      return {
        id: edgeId(merged, i),
        source: e.from,
        target: e.to,
        selectable: true,
        label: e.kind === "choice" && e.text ? truncate(e.text, 26) : e.kind === "call" ? "call" : undefined,
        labelBgPadding: [6, 3] as [number, number],
        labelBgBorderRadius: 4,
        style: {
          stroke: style.stroke,
          strokeWidth: style.width,
          strokeDasharray: style.dash,
        },
        markerEnd: {
          type: MarkerType.ArrowClosed,
          width: 14,
          height: 14,
          color: style.stroke,
        },
        animated: false,
        data: { edge: e },
      };
    });
    return { nodes: flowNodes, edges: [...flowEdges, ...ghostEdges] };
  }, [graph, diagnostics, start, graphLayout]);

  const [rfNodes, setNodes, onNodesChange] = useNodesState<GraphNodeType>(flow.nodes);
  const [rfEdges, setEdges, onEdgesChange] = useEdgesState(flow.edges);
  useEffect(() => setNodes((prev) => withSelection(flow.nodes, prev)), [flow.nodes, setNodes]);
  useEffect(() => setEdges(flow.edges), [flow.edges, setEdges]);

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
    const labels = dragged.filter((node) => node.type === "label");
    if (labels.length === 0) return;
    setNodePositions(Object.fromEntries(labels.map((node) => [node.id, node.position])));
  };

  const onConnect = (connection: Connection) => {
    if (!connection.source || !connection.target || connection.source === connection.target) return;
    setConnectPrompt({ from: connection.source, to: connection.target });
    setChoiceText(connection.target);
  };

  const connectScenes = async (choice: string | null) => {
    if (!connectPrompt) return;
    const { from, to } = connectPrompt;
    const state = appStore().getState();
    const focus = await state.edit([{ op: "connect", from, to, choice }]);
    setConnectPrompt(null);
    if (focus) toast.success(choice ? `Choice added from ${from} to ${to}` : `${from} now goes to ${to}`);
    else {
      const error = appStore().getState().editError;
      if (error) toast.error(error);
    }
  };

  const applyEdgeAction = async (edge: GraphEdge, action: "delete" | "makeExplicit" | "endHere") => {
    const state = appStore().getState();
    if (await state.edit(edgeOps(edge, action))) return;
    const error = appStore().getState().editError;
    if (error) toast.error(error);
  };

  const renameChoice = async (edge: GraphEdge, text: string) => {
    if (edge.kind !== "choice" || !edge.text || text === edge.text) return;
    const node = graph.nodes.find((item) => item.label === edge.from);
    const source = node && appStore().getState().buffers[node.path]?.source;
    if (!node || source === undefined) return;
    try {
      const scene = await window.kairos.edit.blocks(source, edge.from);
      const option = scene && findChoiceOption(scene.blocks, edge.text, edge.to);
      if (!option) {
        toast.error("Could not rename the choice", {
          description: `No choice called "${edge.text}" in ${edge.from} leads to ${edge.to}. Edit it in the scene instead.`,
        });
        return;
      }
      const state = appStore().getState();
      if (!(await state.edit([{ op: "setChoiceText", path: node.path, line: option.line, text }]))) {
        const error = appStore().getState().editError;
        if (error) toast.error(error);
      }
    } catch (error) {
      toast.error("Could not rename the choice", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const addAt = async (x: number, y: number, after?: string, from?: string) => {
    const label = await actions.add(after, from);
    if (label) setNodePositions({ [label]: screenToFlowPosition({ x, y }) });
  };

  const selected = rfNodes.find((node) => node.selected && node.type === "label");
  const selectedLabels = rfNodes.filter((node) => node.selected && node.type === "label").map((node) => node.id);
  const selectedEdge = rfEdges.find((edge) => edge.selected);
  const selectedGraphEdge = (selectedEdge?.data as { edge?: GraphEdge } | undefined)?.edge;
  const sourceNode = selectedEdge && flow.nodes.find((node) => node.id === selectedEdge.source);
  const targetNode = selectedEdge && flow.nodes.find((node) => node.id === selectedEdge.target);
  const openNode = (label: string | null) => {
    if (label?.startsWith("missing:")) void actions.createGhost(label.slice("missing:".length));
    else if (label) openScene(label);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const target = event.target;
    if (
      target instanceof HTMLElement &&
      (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
    )
      return;
    const mod = event.metaKey || event.ctrlKey;
    if (mod && event.key.toLowerCase() === "z") {
      event.preventDefault();
      if (event.shiftKey) appStore().getState().redo();
      else appStore().getState().undo();
    } else if (mod && event.key.toLowerCase() === "d") {
      event.preventDefault();
      if (selectedLabels[0]) void actions.duplicate(selectedLabels[0]);
    } else if (event.key === "Delete" || event.key === "Backspace") {
      if (selectedGraphEdge) {
        event.preventDefault();
        void applyEdgeAction(selectedGraphEdge, "delete");
      } else if (selectedLabels.length) {
        event.preventDefault();
        void actions.remove(selectedLabels);
      }
    } else if (event.key === "F2") {
      if (selectedLabels[0]) {
        event.preventDefault();
        void actions.rename(selectedLabels[0]);
      }
    } else if (event.key === "Enter" && selected) {
      event.preventDefault();
      openNode(selected.id);
    }
  };

  return (
    <ContextMenuPrimitive.Root>
      <ContextMenuPrimitive.Trigger asChild>
        <div
          role="application"
          aria-label="Story graph"
          className="relative flex min-h-0 flex-1 outline-none"
          onKeyDown={onKeyDown}
        >
          <ReactFlow
            nodes={rfNodes}
            onNodesChange={onNodesChange}
            edges={rfEdges}
            onEdgesChange={onEdgesChange}
            nodeTypes={nodeTypes}
            onConnect={(connection) => void onConnect(connection)}
            onConnectEnd={(_, connectionState) => {
              const event = _ as MouseEvent | TouchEvent;
              const point = "changedTouches" in event ? event.changedTouches[0] : event;
              const target = event.target;
              if (!point) return;
              if (
                connectionState.isValid === false &&
                connectionState.fromNode?.id &&
                target instanceof Element &&
                target.closest(".react-flow__pane")
              ) {
                const from = connectionState.fromNode.id;
                void addAt(point.clientX, point.clientY, from, from);
              }
            }}
            onNodeClick={(_, node) => setGhostLabel(node.type === "ghost" ? node.data.label : null)}
            onNodeDoubleClick={(_, node) => openNode(node.id)}
            onNodeContextMenu={(_, node) => {
              setContextLabel(node.id);
              setGhostLabel(null);
            }}
            onPaneContextMenu={(event) => {
              setContextLabel(null);
              setGhostLabel(null);
              setContextPoint({ x: event.clientX, y: event.clientY });
            }}
            onNodeDragStop={(_, __, nodes) => saveDragged(nodes)}
            selectionOnDrag
            multiSelectionKeyCode="Shift"
            panOnDrag={[1]}
            panOnScroll
            nodesConnectable
            nodesDraggable
            deleteKeyCode={null}
            minZoom={0.25}
            maxZoom={2}
            proOptions={{ hideAttribution: true }}
            colorMode={appearance}
            fitView
          >
            <NodeToolbar
              nodeId={selected?.id}
              isVisible={Boolean(selected)}
              className="flex items-center gap-1 rounded-md border bg-popover p-1 shadow-lg"
            >
              <Button variant="ghost" size="sm" onClick={() => selected && openNode(selected.id)}>
                Open
              </Button>
              <Button variant="ghost" size="sm" onClick={() => selected && void actions.rename(selected.id)}>
                Rename
              </Button>
              <Button variant="ghost" size="sm" onClick={() => selected && void actions.duplicate(selected.id)}>
                Duplicate
              </Button>
              <Button variant="ghost" size="sm" onClick={() => void actions.remove(selectedLabels)}>
                Delete
              </Button>
            </NodeToolbar>
            <NodeToolbar
              nodeId={ghostLabel ? `missing:${ghostLabel}` : undefined}
              isVisible={Boolean(ghostLabel)}
              className="rounded-md border bg-popover p-1 shadow-lg"
            >
              <Button size="sm" onClick={() => ghostLabel && void actions.createGhost(ghostLabel)}>
                Create this scene
              </Button>
            </NodeToolbar>
            {selectedEdge && selectedGraphEdge && sourceNode && targetNode && (
              <EdgeLabelRenderer>
                <div
                  className="pointer-events-auto absolute z-20 flex items-center gap-1 rounded-md border bg-popover p-1 shadow-lg"
                  style={{
                    transform: `translate(-50%, -50%) translate(${(sourceNode.position.x + targetNode.position.x + NODE_WIDTH) / 2}px, ${(sourceNode.position.y + targetNode.position.y + NODE_HEIGHT) / 2}px)`,
                  }}
                  onClick={(event) => event.stopPropagation()}
                  onMouseDown={(event) => event.stopPropagation()}
                >
                  {selectedGraphEdge.kind === "choice" && (
                    <input
                      key={selectedEdge.id}
                      aria-label="Choice text"
                      defaultValue={selectedGraphEdge.text ?? ""}
                      className="h-7 w-36 rounded border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      onBlur={(event) => void renameChoice(selectedGraphEdge, event.currentTarget.value.trim())}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") event.currentTarget.blur();
                      }}
                    />
                  )}
                  {selectedGraphEdge.kind === "fallthrough" ? (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => void applyEdgeAction(selectedGraphEdge, "makeExplicit")}
                      >
                        Make explicit
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => void applyEdgeAction(selectedGraphEdge, "endHere")}
                      >
                        End scene here
                      </Button>
                    </>
                  ) : (
                    <Button variant="ghost" size="sm" onClick={() => void applyEdgeAction(selectedGraphEdge, "delete")}>
                      Delete arrow
                    </Button>
                  )}
                </div>
              </EdgeLabelRenderer>
            )}
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
              <span className="border-l pl-3">Drag from a node’s bottom handle to connect scenes</span>
              <Button variant="secondary" size="sm" onClick={() => void actions.add()}>
                <Plus />
                Add scene
              </Button>
              <Button variant="secondary" size="sm" onClick={tidyUp}>
                <Wand2 />
                Tidy up
              </Button>
            </div>
            {graph.nodes.length === 0 && (
              <div className="pointer-events-none absolute inset-0 grid place-items-center text-xs text-muted-foreground">
                No labels yet. Write <code className="mx-1 font-mono text-foreground">label start:</code> in a script to
                begin.
              </div>
            )}
          </ReactFlow>
          <ContextMenuPrimitive.Portal>
            <ContextMenuPrimitive.Content className="z-50 min-w-44 overflow-hidden rounded-lg border bg-popover p-1 text-popover-foreground shadow-lg">
              {contextLabel?.startsWith("missing:") ? (
                <ContextMenuPrimitive.Item
                  className={contextItemClass}
                  onSelect={() => void actions.createGhost(contextLabel.slice("missing:".length))}
                >
                  Create this scene
                </ContextMenuPrimitive.Item>
              ) : contextLabel ? (
                <>
                  <ContextMenuPrimitive.Item className={contextItemClass} onSelect={() => openNode(contextLabel)}>
                    Open
                  </ContextMenuPrimitive.Item>
                  <ContextMenuPrimitive.Item
                    className={contextItemClass}
                    onSelect={() => void actions.rename(contextLabel)}
                  >
                    Rename
                  </ContextMenuPrimitive.Item>
                  <ContextMenuPrimitive.Item
                    className={contextItemClass}
                    onSelect={() => void actions.duplicate(contextLabel)}
                  >
                    Duplicate
                  </ContextMenuPrimitive.Item>
                  <ContextMenuPrimitive.Item
                    className={contextItemClass}
                    onSelect={() => void actions.setStart(contextLabel)}
                  >
                    Set as start
                  </ContextMenuPrimitive.Item>
                  <ContextMenuPrimitive.Item
                    className={contextItemClass}
                    onSelect={() => void actions.remove([contextLabel])}
                  >
                    Delete
                  </ContextMenuPrimitive.Item>
                </>
              ) : (
                <>
                  <ContextMenuPrimitive.Item
                    className={contextItemClass}
                    onSelect={() => void addAt(contextPoint.x, contextPoint.y)}
                  >
                    New scene here
                  </ContextMenuPrimitive.Item>
                  <ContextMenuPrimitive.Item className={contextItemClass} onSelect={tidyUp}>
                    Tidy up
                  </ContextMenuPrimitive.Item>
                </>
              )}
            </ContextMenuPrimitive.Content>
          </ContextMenuPrimitive.Portal>
        </div>
      </ContextMenuPrimitive.Trigger>
      <SceneDialogs {...actions.dialogs} />
      <Dialog
        open={connectPrompt !== null}
        onOpenChange={(open) => !open && setConnectPrompt(null)}
        title="Connect scenes"
      >
        <div className="space-y-3">
          <p className="text-xs text-muted-foreground">
            Choose how {connectPrompt?.from} leads to {connectPrompt?.to}.
          </p>
          <label className="block space-y-1 text-xs">
            <span>Choice text</span>
            <input
              aria-label="Choice text"
              value={choiceText}
              onChange={(event) => setChoiceText(event.target.value)}
              className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          </label>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={() => void connectScenes(null)}>
              Go to
            </Button>
            <Button type="button" onClick={() => void connectScenes(choiceText.trim())} disabled={!choiceText.trim()}>
              Add as a choice
            </Button>
          </div>
        </div>
      </Dialog>
    </ContextMenuPrimitive.Root>
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
