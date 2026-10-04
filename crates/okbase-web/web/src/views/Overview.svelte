<script lang="ts">
  import { api, docHref, type Diagnostic, type LintReport, type Overview } from "../lib/api";
  import { typePalette } from "../lib/colors";

  let { overview }: { overview: Overview } = $props();
  const s = $derived(overview.stats);
  const palette = $derived(typePalette(s.types));

  let lint = $state<LintReport | null>(null);
  let lintError = $state<string | null>(null);
  let onlyErrors = $state(false);
  api.lint().then((r) => (lint = r), (e) => (lintError = String(e.message ?? e)));

  const types = $derived(Object.entries(s.types).sort((a, b) => b[1] - a[1]));
  const langs = $derived(Object.entries(s.langs).sort((a, b) => b[1] - a[1]));
  const maxType = $derived(Math.max(1, ...types.map(([, n]) => n)));
  const maxLang = $derived(Math.max(1, ...langs.map(([, n]) => n)));

  const byRule = $derived.by(() => {
    const m = new Map<string, Diagnostic[]>();
    for (const d of lint?.diagnostics ?? []) {
      if (onlyErrors && d.severity !== "error") continue;
      m.set(d.rule, [...(m.get(d.rule) ?? []), d]);
    }
    return [...m].sort((a, b) => b[1].length - a[1].length);
  });

  const fmt = (n: number) => n.toLocaleString();
  const idOf = (path: string) => path.replace(/\.md$/, "");
</script>

<div class="tiles">
  <div class="card tile"><div class="label">Documents</div><div class="value">{fmt(s.docs)}</div></div>
  <div class="card tile"><div class="label">Tokens (est.)</div><div class="value">{fmt(s.tokens)}</div></div>
  <div class="card tile">
    <div class="label">Broken links</div>
    <div class="value" class:bad={s.broken_links > 0}>{fmt(s.broken_links)}</div>
  </div>
  <div class="card tile">
    <div class="label">OKF level</div>
    <div class="value">{lint ? (lint.level ?? "none") : "…"}</div>
    {#if lint}<div class="small muted">target {lint.target}</div>{/if}
  </div>
  <div class="card tile"><div class="label">Recommended mode</div><div class="value">{s.mode}</div></div>
</div>

<div class="grid">
  <section class="card">
    <h2>Documents by type</h2>
    {#if types.length === 0}
      <p class="muted">No document has a <code>type</code>.</p>
    {/if}
    <ul class="bars">
      {#each types as [t, n] (t)}
        <li title="{t}: {n}">
          <span class="name"><span class="dot" style="background:{palette.color(t)}"></span>{t}</span>
          <span class="track"><span class="bar" style="width:{(n / maxType) * 100}%"></span></span>
          <span class="num">{n}</span>
        </li>
      {/each}
    </ul>
  </section>
  <section class="card">
    <h2>Documents by language</h2>
    <ul class="bars">
      {#each langs as [l, n] (l)}
        <li title="{l}: {n}">
          <span class="name">{l}</span>
          <span class="track"><span class="bar" style="width:{(n / maxLang) * 100}%"></span></span>
          <span class="num">{n}</span>
        </li>
      {/each}
    </ul>
    <p class="small muted">Capabilities: {overview.capabilities.join(", ")}</p>
  </section>
</div>

<section class="card lint">
  <div class="lint-head">
    <h2>Lint</h2>
    {#if lint}
      <span class="chip">{lint.errors} errors</span>
      <span class="chip">{lint.warnings} warnings</span>
      <label class="small muted"><input type="checkbox" bind:checked={onlyErrors} /> Errors only</label>
    {/if}
  </div>
  {#if lintError}
    <p class="error">{lintError}</p>
  {:else if !lint}
    <p class="muted">Running lint…</p>
  {:else if byRule.length === 0}
    <p class="muted">No issues at level {lint.target}.</p>
  {:else}
    {#each byRule as [rule, items] (rule)}
      <details>
        <summary>
          <span class="sev" class:err={items[0].severity === "error"}>{items[0].severity === "error" ? "✕" : "!"}</span>
          <span class="mono">{rule}</span> <span class="muted">({items.length})</span>
          <span class="muted small">— {items[0].message}</span>
        </summary>
        <ul class="diag">
          {#each items as d, i (i)}
            <li>
              {#if d.path.endsWith(".md")}
                <a class="mono" href={docHref(idOf(d.path))}>{d.path}{d.line ? `:${d.line}` : ""}</a>
              {:else}
                <span class="mono">{d.path}</span>
              {/if}
              <span class="muted small">{d.message}</span>
            </li>
          {/each}
        </ul>
      </details>
    {/each}
  {/if}
</section>

<style>
  h2 { font-size: 16px; margin: 0 0 12px; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 12px; margin-bottom: 12px; }
  .tile .label { font-size: 13px; color: var(--text-2); }
  .tile .value { font-size: 28px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .tile .value.bad { color: var(--danger); }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 12px; margin-bottom: 12px; }
  .bars { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .bars li { display: grid; grid-template-columns: minmax(90px, 180px) 1fr 48px; align-items: center; gap: 8px; }
  .name { display: flex; gap: 6px; align-items: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
  .track { height: 10px; }
  .bar { display: block; height: 100%; background: var(--accent); border-radius: 0 4px 4px 0; min-width: 2px; }
  .num { text-align: right; font-variant-numeric: tabular-nums; color: var(--text-2); font-size: 14px; }
  .lint-head { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-bottom: 8px; }
  .lint-head h2 { margin: 0 8px 0 0; }
  details { border-top: 1px solid var(--border); padding: 6px 0; }
  summary { cursor: pointer; display: flex; gap: 6px; align-items: baseline; flex-wrap: wrap; }
  .sev { display: inline-grid; place-items: center; width: 18px; height: 18px; border-radius: 50%; font-size: 11px; background: var(--surface-2); color: var(--warn); font-weight: 700; }
  .sev.err { color: var(--danger); }
  .diag { margin: 6px 0 4px; padding-left: 24px; display: grid; gap: 2px; }
  .diag li { overflow-wrap: anywhere; }
</style>
