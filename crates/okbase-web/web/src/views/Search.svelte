<script lang="ts">
  import { api, docHref, type Facets, type Filter, type GrepResult, type QueryResult } from "../lib/api";
  import { go } from "../lib/router.svelte";
  import { untrack } from "svelte";

  let { q, tag }: { q: string; tag: string | null } = $props();

  const FACETS = [
    ["type", "Type"],
    ["tags", "Tags"],
    ["status", "Status"],
    ["lang", "Language"],
  ] as const;

  // The view is re-created when the route changes, so the props only seed the form.
  const initialTag = untrack(() => tag);
  let text = $state(untrack(() => q));
  let picked = $state<Record<string, string[]>>({
    type: [],
    tags: initialTag ? [initialTag] : [],
    status: [],
    lang: [],
  });
  let facets = $state<Facets>({});
  let list = $state<QueryResult | null>(null);
  let hits = $state<GrepResult | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  const filter = $derived<Filter>({
    type: picked.type.length ? picked.type : undefined,
    tags_all: picked.tags.length ? picked.tags : undefined,
    status: picked.status.length ? picked.status : undefined,
    lang: picked.lang.length ? picked.lang : undefined,
  });

  $effect(() => {
    const f = filter;
    const pattern = q.trim();
    loading = true;
    error = null;
    const counts = api.query({ ...f, facets: FACETS.map(([k]) => k), limit: pattern ? 0 : 200 });
    counts.then((r) => {
      facets = r.facets;
      list = r;
    });
    const search = pattern
      ? api.grep({ pattern, context: 1, limit: 200, filter: f }).then((r) => (hits = r))
      : Promise.resolve((hits = null));
    Promise.all([counts, search])
      .catch((e) => (error = String(e.message ?? e)))
      .finally(() => (loading = false));
  });

  function toggle(key: string, value: string) {
    const cur = picked[key];
    picked = { ...picked, [key]: cur.includes(value) ? cur.filter((v) => v !== value) : [...cur, value] };
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    go(`#/search?q=${encodeURIComponent(text)}`);
  }
</script>

<div class="search">
  <aside>
    {#each FACETS as [key, label] (key)}
      {#if (facets[key] ?? []).length || picked[key].length}
        <section>
          <h3>{label}</h3>
          <ul>
            {#each (facets[key] ?? []).slice(0, 30) as [value, count] (value)}
              <li>
                <label>
                  <input type="checkbox" checked={picked[key].includes(value)} onchange={() => toggle(key, value)} />
                  <span class="v">{value}</span>
                  <span class="muted small">{count}</span>
                </label>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/each}
  </aside>

  <div class="results">
    <form onsubmit={submit}>
      <input type="search" bind:value={text} placeholder="Regex or words; accents and case are ignored" aria-label="Search text" />
      <button type="submit">Search</button>
    </form>

    {#if error}
      <p class="error">{error}</p>
    {:else if hits}
      <p class="muted small">
        {hits.total_lines} matching lines in {hits.total_docs} documents{hits.truncated ? " (showing the first)" : ""}
        {#if hits.literal}· searched as plain text{/if}
        {#if loading}· updating…{/if}
      </p>
      {#if hits.hint}<p class="muted small">{hits.hint}</p>{/if}
      {#each hits.docs as d (d.id)}
        <article class="card hit">
          <a href={docHref(d.id)}><strong>{d.title}</strong></a>
          <span class="mono small muted">{d.id}</span>
          <span class="chip">{d.matches}</span>
          <pre>{#each d.lines as l, i (i)}<span class:hit={l.hit}><span class="ln">{l.line}</span>{l.text}</span>
{/each}</pre>
        </article>
      {/each}
    {:else if list}
      <p class="muted small">{list.total} documents{list.more ? `, showing ${list.docs.length}` : ""}{loading ? " · updating…" : ""}</p>
      <table>
        <thead><tr><th>Title</th><th>Type</th><th>Status</th><th>Updated</th></tr></thead>
        <tbody>
          {#each list.docs as d (d.id)}
            <tr>
              <td><a href={docHref(d.id)}>{d.title || d.id}</a><div class="mono small muted">{d.id}</div></td>
              <td>{d.type ?? ""}</td>
              <td>{d.status ?? ""}</td>
              <td class="small">{d.updated ?? ""}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="muted">Loading…</p>
    {/if}
  </div>
</div>

<style>
  .search { display: grid; grid-template-columns: 240px minmax(0, 1fr); gap: 24px; }
  @media (max-width: 760px) { .search { grid-template-columns: minmax(0, 1fr); } }
  h3 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-2); margin: 0 0 6px; }
  aside section { margin-bottom: 16px; }
  aside ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; max-height: 280px; overflow: auto; }
  label { display: flex; align-items: center; gap: 6px; font-size: 14px; cursor: pointer; }
  .v { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  form { display: flex; gap: 8px; margin-bottom: 8px; }
  form input { flex: 1; }
  .hit { margin-bottom: 10px; display: flex; flex-wrap: wrap; gap: 8px; align-items: baseline; }
  .hit pre { flex-basis: 100%; margin: 4px 0 0; white-space: pre-wrap; overflow-wrap: anywhere; font-size: 13px; color: var(--text-2); }
  .hit pre .hit { color: var(--text); background: color-mix(in srgb, var(--accent) 14%, transparent); display: inline; margin: 0; }
  .ln { display: inline-block; min-width: 3em; color: var(--text-3); user-select: none; }
  table { width: 100%; border-collapse: collapse; font-size: 14px; }
  th { text-align: left; color: var(--text-2); font-weight: 500; border-bottom: 1px solid var(--border); padding: 6px 8px; }
  td { border-bottom: 1px solid var(--grid); padding: 6px 8px; vertical-align: top; }
</style>
