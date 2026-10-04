<script lang="ts">
  import { api, type Overview } from "./lib/api";
  import { router, go } from "./lib/router.svelte";
  import OverviewView from "./views/Overview.svelte";
  import GraphView from "./views/Graph.svelte";
  import SearchView from "./views/Search.svelte";
  import DocView from "./views/Doc.svelte";

  let overview = $state<Overview | null>(null);
  let error = $state<string | null>(null);
  let q = $state("");

  api.overview().then(
    (o) => {
      overview = o;
      document.title = `${o.name} · okbase view`;
    },
    (e) => (error = String(e.message ?? e)),
  );

  function search(e: SubmitEvent) {
    e.preventDefault();
    go(`#/search?q=${encodeURIComponent(q)}`);
  }

  const tabs = [
    { name: "overview", label: "Overview", hash: "#/" },
    { name: "graph", label: "Graph", hash: "#/graph" },
    { name: "search", label: "Search", hash: "#/search" },
  ];
</script>

<header>
  <a class="brand" href="#/">
    <span class="logo">ok</span>
    <span>{overview?.name ?? "okbase"}</span>
  </a>
  <nav>
    {#each tabs as t (t.name)}
      <a href={t.hash} class:active={router.route.name === t.name}>{t.label}</a>
    {/each}
  </nav>
  <form onsubmit={search} role="search">
    <input type="search" placeholder="Search documents…" bind:value={q} aria-label="Search" />
  </form>
</header>

<main>
  {#if error}
    <p class="error">Could not load the bundle: {error}</p>
  {:else if !overview}
    <p class="muted">Loading…</p>
  {:else if router.route.name === "overview"}
    <OverviewView {overview} />
  {:else if router.route.name === "graph"}
    <GraphView {overview} />
  {:else if router.route.name === "search"}
    {#key `${router.route.q}|${router.route.tag}`}
      <SearchView q={router.route.q} tag={router.route.tag} />
    {/key}
  {:else if router.route.name === "doc"}
    {#key router.route.id}
      <DocView id={router.route.id} anchor={router.route.anchor} {overview} />
    {/key}
  {/if}
</main>

<style>
  header {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .brand { display: flex; align-items: center; gap: 8px; color: var(--text); font-weight: 600; }
  .brand:hover { text-decoration: none; }
  .logo {
    display: grid; place-items: center; width: 28px; height: 28px; border-radius: 7px;
    background: var(--accent); color: #fff; font-size: 13px; font-weight: 700;
  }
  nav { display: flex; gap: 4px; }
  nav a { padding: 4px 10px; border-radius: 6px; color: var(--text-2); }
  nav a:hover { background: var(--surface-2); text-decoration: none; }
  nav a.active { background: var(--surface-2); color: var(--text); font-weight: 500; }
  form { margin-left: auto; flex: 1 1 200px; max-width: 360px; display: flex; }
  form input { width: 100%; }
  main { flex: 1; padding: 20px 16px; width: 100%; max-width: 1400px; margin: 0 auto; display: flex; flex-direction: column; }
</style>
