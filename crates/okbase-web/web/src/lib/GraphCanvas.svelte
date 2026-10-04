<script lang="ts">
  import Graph from "graphology";
  import Sigma from "sigma";
  import FA2Layout from "graphology-layout-forceatlas2/worker";
  import forceAtlas2 from "graphology-layout-forceatlas2";
  import type { GraphResult } from "./api";
  import { cssVar, type TypePalette } from "./colors";
  import { go } from "./router.svelte";
  import { docHref } from "./api";

  interface Props {
    data: GraphResult;
    palette: TypePalette;
    /** Highlight this node (the document being read). */
    focus?: string | null;
    /** Color documents with broken links in the danger color. */
    showBroken?: boolean;
    /** Show every label (small graphs) rather than only large nodes. */
    allLabels?: boolean;
  }
  let { data, palette, focus = null, showBroken = false, allLabels = false }: Props = $props();

  let container: HTMLDivElement;
  let hovered = $state<string | null>(null);
  let tip = $state<{ x: number; y: number; title: string; id: string; meta: string } | null>(null);

  $effect(() => {
    const graph = new Graph({ type: "directed", multi: false });
    const n = data.nodes.length;
    const danger = cssVar("--danger");
    data.nodes.forEach((node, i) => {
      const degree = node.in_degree + node.out_degree;
      const angle = (i / Math.max(n, 1)) * Math.PI * 2;
      graph.addNode(node.id, {
        label: node.title || node.id,
        x: Math.cos(angle) * 100 + Math.random(),
        y: Math.sin(angle) * 100 + Math.random(),
        size: n > 300 ? 2 + Math.sqrt(degree) * 1.4 : 4 + Math.sqrt(degree) * 2.4,
        color: showBroken && node.broken > 0 ? danger : palette.color(node.type),
        type_: node.type,
        broken: node.broken,
        degree,
      });
    });
    for (const e of data.edges) {
      if (graph.hasNode(e.src) && graph.hasNode(e.target)) {
        graph.addEdge(e.src, e.target, { size: Math.min(1 + Math.log2(e.count), 3) });
      }
    }

    // Layout before the renderer fits the camera: synchronous for small graphs, a worker for large ones.
    const settings = { ...forceAtlas2.inferSettings(graph), barnesHutOptimize: n > 300 };
    let layout: FA2Layout | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    if (n <= 300) {
      forceAtlas2.assign(graph, { iterations: 300, settings });
    }

    const edgeColor = cssVar("--border");
    const labelColor = cssVar("--text");
    const muted = cssVar("--grid");
    const renderer = new Sigma(graph, container, {
      renderEdgeLabels: false,
      defaultEdgeType: "arrow",
      defaultEdgeColor: edgeColor,
      labelColor: { color: labelColor },
      labelFont: "system-ui, sans-serif",
      labelSize: 12,
      labelRenderedSizeThreshold: allLabels ? 0 : 9,
      zIndex: true,
    });

    // Hover: keep the node and its neighbours, fade the rest.
    renderer.setSetting("nodeReducer", (node, attrs) => {
      const active = hovered ?? focus;
      if (!active || !graph.hasNode(active)) return attrs;
      if (node === active || graph.areNeighbors(node, active)) {
        return { ...attrs, zIndex: 1, forceLabel: node === active || !!hovered };
      }
      return { ...attrs, color: muted, label: null, zIndex: 0 };
    });
    renderer.setSetting("edgeReducer", (edge, attrs) => {
      const active = hovered ?? focus;
      if (!active || !graph.hasNode(active)) return attrs;
      return graph.hasExtremity(edge, active)
        ? { ...attrs, color: cssVar("--text-3"), size: (attrs.size ?? 1) + 0.5 }
        : { ...attrs, hidden: true };
    });
    renderer.on("enterNode", ({ node, event }) => {
      hovered = node;
      const a = graph.getNodeAttributes(node);
      const meta = [
        a.type_ ?? "no type",
        `${graph.outDegree(node)} out · ${graph.inDegree(node)} in`,
        a.broken ? `${a.broken} broken` : "",
      ].filter(Boolean).join(" · ");
      tip = { x: event.x, y: event.y, title: a.label, id: node, meta };
      container.style.cursor = "pointer";
      renderer.refresh({ skipIndexation: true });
    });
    renderer.on("leaveNode", () => {
      hovered = null;
      tip = null;
      container.style.cursor = "";
      renderer.refresh({ skipIndexation: true });
    });
    renderer.on("clickNode", ({ node }) => go(docHref(node)));

    if (n > 300) {
      layout = new FA2Layout(graph, { settings });
      layout.start();
      timer = setTimeout(() => layout?.stop(), Math.min(1500 + n, 8000));
    }

    return () => {
      clearTimeout(timer);
      layout?.kill();
      renderer.kill();
    };
  });
</script>

<div class="wrap">
  <div class="canvas" bind:this={container} role="img" aria-label="Link graph of {data.nodes.length} documents"></div>
  {#if tip}
    <div class="tip" style="left:{tip.x + 14}px; top:{tip.y + 14}px">
      <strong>{tip.title}</strong>
      <div class="mono small muted">{tip.id}</div>
      <div class="small">{tip.meta}</div>
    </div>
  {/if}
</div>

<style>
  .wrap { position: absolute; inset: 0; }
  .canvas { position: absolute; inset: 0; }
  .tip {
    position: absolute;
    pointer-events: none;
    max-width: 320px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 10px;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.12);
    z-index: 5;
  }
</style>
