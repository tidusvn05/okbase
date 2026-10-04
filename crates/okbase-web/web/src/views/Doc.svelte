<script lang="ts">
  import { api, docHref, type DocView, type GraphResult, type Overview } from "../lib/api";
  import { typePalette } from "../lib/colors";
  import GraphCanvas from "../lib/GraphCanvas.svelte";
  import { untrack } from "svelte";

  let { id, anchor, overview }: { id: string; anchor: string | null; overview: Overview } = $props();
  const palette = $derived(typePalette(overview.stats.types));

  let doc = $state<DocView | null>(null);
  let error = $state<string | null>(null);
  let near = $state<GraphResult | null>(null);
  let body = $state<HTMLElement>();

  $effect(() => {
    const current = id;
    api.doc(current).then(
      (d) => {
        doc = d;
        const a = untrack(() => anchor);
        if (a) requestAnimationFrame(() => scrollTo(a));
        else window.scrollTo(0, 0);
      },
      (e) => (error = String(e.message ?? e)),
    );
    api.graph({}).then((g) => {
      const keep = new Set([current]);
      for (const e of g.edges) {
        if (e.src === current) keep.add(e.target);
        if (e.target === current) keep.add(e.src);
      }
      near = {
        nodes: g.nodes.filter((n) => keep.has(n.id)),
        edges: g.edges.filter((e) => keep.has(e.src) && keep.has(e.target)),
        truncated: 0,
      };
    });
  });

  function scrollTo(a: string) {
    const el = body?.querySelector(`[id="${CSS.escape(a)}"]`) ?? document.getElementById(a);
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  // In-page anchors and broken links do not change the route.
  function onclick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    const href = a?.getAttribute("href");
    if (!href) return;
    if (href === "#/missing") {
      e.preventDefault();
    } else if (href.startsWith("#") && !href.startsWith("#/")) {
      e.preventDefault();
      scrollTo(decodeURIComponent(href.slice(1)));
    }
  }

  const fm = $derived(
    doc
      ? Object.entries(doc.frontmatter).filter(([k]) => !["title", "description"].includes(k))
      : [],
  );
  const outgoing = $derived.by(() => {
    const seen = new Set<string>();
    return (doc?.links.outgoing ?? []).filter((l) => {
      const key = l.exists && l.id ? l.id : `?${l.raw}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  });
  const backlinks = $derived([...new Set((doc?.links.backlinks ?? []).map((l) => l.id!).filter(Boolean))]);
  const show = (v: unknown) => (typeof v === "string" ? v : JSON.stringify(v));
</script>

{#if error}
  <p class="error">{error}</p>
  <p><a href="#/">Back to the overview</a></p>
{:else if !doc}
  <p class="muted">Loading…</p>
{:else}
  <div class="doc">
    <aside class="toc">
      {#if doc.toc.length > 1}
        <h3>On this page</h3>
        <ul>
          {#each doc.toc as h (h.anchor)}
            <li style="padding-left:{(h.level - 1) * 10}px">
              <a href="#{h.anchor}" onclick={(e) => { e.preventDefault(); scrollTo(h.anchor); }}>{h.text}</a>
            </li>
          {/each}
        </ul>
      {/if}
    </aside>

    <article>
      <div class="crumbs mono small muted">{doc.path}</div>
      <h1>{doc.title}</h1>
      {#if typeof doc.frontmatter.description === "string"}
        <p class="lead">{doc.frontmatter.description}</p>
      {/if}
      <div class="chips">
        {#if typeof doc.frontmatter.type === "string"}
          <span class="chip"><span class="dot" style="background:{palette.color(doc.frontmatter.type)}"></span> {doc.frontmatter.type}</span>
        {/if}
        {#if typeof doc.frontmatter.status === "string"}<span class="chip">{doc.frontmatter.status}</span>{/if}
        {#each (Array.isArray(doc.frontmatter.tags) ? doc.frontmatter.tags : []) as t (t)}
          <a class="chip" href="#/search?tag={encodeURIComponent(String(t))}">#{t}</a>
        {/each}
        <span class="chip">{doc.tokens.toLocaleString()} tokens</span>
      </div>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="prose" bind:this={body} {onclick}>
        {@html doc.html}
      </div>
    </article>

    <aside class="side">
      {#if near && near.nodes.length > 1}
        <section>
          <h3>Neighbourhood</h3>
          <div class="mini card"><GraphCanvas data={near} {palette} focus={id} allLabels /></div>
        </section>
      {/if}
      <section>
        <h3>Links ({outgoing.length})</h3>
        <ul class="links">
          {#each outgoing as l, i (i)}
            <li>
              {#if l.exists && l.id}
                <a href={docHref(l.id)}>{l.id}</a>
              {:else}
                <span class="broken" title="Line {l.line}: target not found">{l.raw}</span>
              {/if}
            </li>
          {:else}
            <li class="muted small">None</li>
          {/each}
        </ul>
      </section>
      <section>
        <h3>Backlinks ({backlinks.length})</h3>
        <ul class="links">
          {#each backlinks as b (b)}
            <li><a href={docHref(b)}>{b}</a></li>
          {:else}
            <li class="muted small">None</li>
          {/each}
        </ul>
      </section>
      {#if fm.length}
        <section>
          <h3>Frontmatter</h3>
          <dl>
            {#each fm as [k, v] (k)}
              <dt class="mono">{k}</dt>
              <dd>{show(v)}</dd>
            {/each}
          </dl>
        </section>
      {/if}
    </aside>
  </div>
{/if}

<style>
  .doc { display: grid; grid-template-columns: 200px minmax(0, 1fr) 300px; gap: 28px; align-items: start; }
  @media (max-width: 1100px) { .doc { grid-template-columns: minmax(0, 1fr) 280px; } .toc { display: none; } }
  @media (max-width: 760px) { .doc { grid-template-columns: minmax(0, 1fr); } }
  aside { position: sticky; top: 72px; max-height: calc(100vh - 90px); overflow: auto; }
  @media (max-width: 760px) { aside { position: static; max-height: none; } }
  h1 { margin: 4px 0 8px; font-size: 28px; }
  h3 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-2); margin: 0 0 8px; }
  .lead { color: var(--text-2); font-size: 17px; margin: 0 0 12px; }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 16px; padding-bottom: 16px; border-bottom: 1px solid var(--border); }
  .chip { display: inline-flex; align-items: center; gap: 5px; }
  a.chip:hover { text-decoration: none; border-color: var(--text-3); }
  .toc ul, .links { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; font-size: 14px; }
  .toc a { color: var(--text-2); }
  .side section { margin-bottom: 20px; }
  .links li { overflow-wrap: anywhere; }
  .broken { color: var(--danger); text-decoration: line-through dotted; }
  .mini { position: relative; height: 220px; padding: 0; overflow: hidden; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 10px; margin: 0; font-size: 13px; }
  dt { color: var(--text-2); }
  dd { margin: 0; overflow-wrap: anywhere; }
</style>
