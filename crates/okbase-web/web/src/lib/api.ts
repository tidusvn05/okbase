// Typed client for the okbase view API (same JSON as the CLI's --json).

export type Facets = Record<string, [string, number][]>;

export interface Stats {
  docs: number;
  reserved: number;
  tokens: number;
  chunks: number;
  types: Record<string, number>;
  langs: Record<string, number>;
  broken_links: number;
  mode: string;
}

export interface Overview {
  name: string;
  stats: Stats;
  capabilities: string[];
}

export interface Filter {
  type?: string[];
  tags_all?: string[];
  tags_any?: string[];
  status?: string[];
  lang?: string[];
  path?: string;
  text?: string;
}

export interface QueryRow {
  id: string;
  title: string;
  type: string | null;
  status: string | null;
  lang: string | null;
  updated: string | null;
  tags: string[];
}

export interface QueryResult {
  total: number;
  docs: QueryRow[];
  more: number;
  facets: Facets;
}

export interface GraphNode {
  id: string;
  title: string;
  type: string | null;
  status: string | null;
  tags: string[];
  out_degree: number;
  in_degree: number;
  broken: number;
}

export interface GraphResult {
  nodes: GraphNode[];
  edges: { src: string; target: string; count: number }[];
  truncated: number;
}

export interface GrepResult {
  docs: {
    id: string;
    title: string;
    matches: number;
    lines: { line: number; text: string; hit: boolean }[];
  }[];
  total_lines: number;
  total_docs: number;
  truncated: boolean;
  literal: boolean;
  hint: string | null;
}

export interface LinkRow {
  id: string | null;
  raw: string;
  text: string;
  kind: string;
  line: number;
  exists: boolean;
}

export interface DocView {
  id: string;
  path: string;
  title: string;
  frontmatter: Record<string, unknown>;
  content: string;
  tokens: number;
  html: string;
  toc: { level: number; text: string; anchor: string }[];
  links: { id: string; outgoing: LinkRow[]; backlinks: LinkRow[] };
}

export interface Diagnostic {
  rule: string;
  level: string;
  severity: "error" | "warning" | string;
  path: string;
  line: number | null;
  field?: string;
  message: string;
}

export interface LintReport {
  target: string;
  level: string | null;
  documents: number;
  errors: number;
  warnings: number;
  diagnostics: Diagnostic[];
}

async function call<T>(path: string, body?: unknown): Promise<T> {
  const resp = await fetch(path, body === undefined
    ? {}
    : { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  const data = await resp.json().catch(() => ({ error: resp.statusText }));
  if (!resp.ok) throw new Error(data.error ?? resp.statusText);
  return data as T;
}

const encodeId = (id: string) => id.split("/").map(encodeURIComponent).join("/");

export const api = {
  overview: () => call<Overview>("/api/overview"),
  query: (req: Filter & { limit?: number; facets?: string[]; count_only?: boolean; sort?: string }) =>
    call<QueryResult>("/api/query", req),
  graph: (req: { filter?: Filter; limit?: number }) => call<GraphResult>("/api/graph", req),
  grep: (req: { pattern: string; context?: number; limit?: number; filter?: Filter }) =>
    call<GrepResult>("/api/grep", req),
  doc: (id: string) => call<DocView>(`/api/doc/${encodeId(id)}`),
  lint: () => call<LintReport>("/api/lint"),
};

export const docHref = (id: string) => `#/doc/${encodeId(id)}`;
