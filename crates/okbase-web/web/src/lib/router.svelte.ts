// Hash router: #/ (overview), #/graph, #/search?q=…, #/doc/<id>.

export type Route =
  | { name: "overview" }
  | { name: "graph" }
  | { name: "search"; q: string; tag: string | null }
  | { name: "doc"; id: string; anchor: string | null };

function parse(hash: string): Route {
  const h = hash.replace(/^#\/?/, "");
  if (h.startsWith("doc/")) {
    const [path, anchor] = h.slice(4).split("#", 2);
    return { name: "doc", id: decodeURIComponent(path), anchor: anchor ?? null };
  }
  if (h === "graph") return { name: "graph" };
  if (h === "search" || h.startsWith("search?")) {
    const params = new URLSearchParams(h.split("?")[1] ?? "");
    return { name: "search", q: params.get("q") ?? "", tag: params.get("tag") };
  }
  return { name: "overview" };
}

export const router = $state({ route: parse(location.hash) });

window.addEventListener("hashchange", () => {
  // `#/missing` marks a broken link; in-page anchors (`#x`) are handled by the doc view.
  if (location.hash === "#/missing" || (location.hash && !location.hash.startsWith("#/"))) return;
  router.route = parse(location.hash);
});

export const go = (hash: string) => {
  location.hash = hash;
};
