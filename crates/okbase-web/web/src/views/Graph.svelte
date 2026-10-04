<script lang="ts">
  import { api, type GraphResult, type Overview } from "../lib/api";
  import { typePalette } from "../lib/colors";
  import GraphCanvas from "../lib/GraphCanvas.svelte";

  let { overview }: { overview: Overview } = $props();

  const palette = $derived(typePalette(overview.stats.types));
  let types = $state<string[]>([]);
  let tag = $state("");
  let path = $state("");
  let showBroken = $state(false);
  let hideIsolated = $state(false);
  let data = $state<GraphResult | null>(null);
  let error = $state<string | null>(null);
  let tags = $state<[string, number][]>([]);

  api.query({ facets: ["tags"], count_only: true }).then((r) => (tags = r.facets.tags ?? []));

  $effect(() => {
    const filter = {
      type: types.length ? types : undefined,
      tags_any: tag ? [tag] : undefined,
      path: path.trim() || undefined,
    };
    error = null;
    api.graph({ filter }).then(
      (g) => (data = g),
      (e) => (error = String(e.message ?? e)),
    );
  });

  const shown = $derived.by(() => {
    if (!data || !hideIsolated) return data;
    const linked = new Set(data.edges.flatMap((e) => [e.src, e.target]));
    return { ...data, nodes: data.nodes.filter((n) => linked.has(n.id)) };
  });

  function toggle(t: string) {
    types = types.includes(t) ? types.filter((x) => x !== t) : [...types, t];
  }
</script>

<div class="layout">
  <div class="filters">
    <div class="types" role="group" aria-label="Filter by type">
      {#each palette.legend as l (l.type)}
        {#if l.type !== "Other"}
          <button class:off={types.length > 0 && !types.includes(l.type)} onclick={() => toggle(l.type)}>
            <span class="dot" style="background:{l.color}"></span>{l.type}
            <span class="muted small">{overview.stats.types[l.type]}</span>
          </button>
        {:else}
          <span class="chip"><span class="dot" style="background:{l.color}"></span> Other types</span>
        {/if}
      {/each}
    </div>
    <select bind:value={tag} aria-label="Filter by tag">
      <option value="">All tags</option>
      {#each tags as [t, c] (t)}
        <option value={t}>{t} ({c})</option>
      {/each}
    </select>
    <input type="text" placeholder="Path prefix, e.g. policies/" bind:value={path} aria-label="Path prefix" />
    <label><input type="checkbox" bind:checked={showBroken} /> Mark broken links</label>
    <label><input type="checkbox" bind:checked={hideIsolated} /> Hide unlinked</label>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {:else if !shown}
    <p class="muted">Loading graph…</p>
  {:else}
    <p class="muted small summary">
      {shown.nodes.length} documents · {shown.edges.length} links
      {#if shown.truncated}· {shown.truncated} less-linked documents left out{/if}
      {#if shown.edges.length === 0}· no links between these documents{/if}
      · hover to see neighbours, click to open
    </p>
    <div class="canvas card">
      <GraphCanvas data={shown} {palette} {showBroken} allLabels={shown.nodes.length <= 60} />
    </div>
  {/if}
</div>

<style>
  .layout { display: flex; flex-direction: column; gap: 12px; flex: 1; min-height: 0; }
  .filters { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .types { display: flex; flex-wrap: wrap; gap: 6px; }
  .types button { display: inline-flex; align-items: center; gap: 6px; }
  .types button.off { opacity: 0.45; }
  .types .chip { display: inline-flex; align-items: center; gap: 6px; font-size: 15px; padding: 5px 10px; border-radius: 6px; color: var(--text-2); }
  label { display: inline-flex; align-items: center; gap: 4px; font-size: 14px; color: var(--text-2); }
  select { background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 5px 8px; max-width: 200px; }
  .summary { margin: 0; }
  .canvas { position: relative; flex: 1; min-height: 520px; padding: 0; overflow: hidden; }
</style>
