---
name: okfkit-adopt
description: Convert a folder of plain markdown (docs site, wiki, Obsidian vault) into an OKF knowledge bundle with okfkit adopt, then finish the parts heuristics cannot do. Use when asked to adopt, convert, import or standardize a markdown folder.
---

# Adopting a markdown folder

`okfkit adopt` adds the frontmatter and index files an OKF bundle needs, without changing existing values or page bodies.

## Steps

1. **Plan first.** Run `okfkit adopt DIR -v` (writes nothing). Check the detected site, the level before and after, and which pages it moves (content pages named `index.md` move to `overview.md`).
2. **Apply.** Prefer `okfkit adopt DIR --out NEW_DIR` (the source stays untouched). Use `--write` only in a clean git working tree, so the change is one reviewable diff. Add `--level L2` to also fill `lang`, `status`, `updated` and `tags`.
3. **Finish what heuristics cannot.**
   - Descriptions taken from the first sentence of a page (the plan lists them; lint reports them as `unreviewed-generated`): rewrite each as one specific sentence.
   - `type` guessed from the directory (often `Document`): set a more precise type when the page is clearly a guide, reference, FAQ, policy, etc.
   - Links to moved `index.md` pages: point them at the new `overview.md`.
4. **Check.** Run `okfkit --bundle NEW_DIR lint --level L1` (or `L2`) and fix the remaining errors; follow the okfkit-curate skill for anything beyond that.
5. **Set up agents.** `okfkit --bundle NEW_DIR agent install --claude` (or `--codex`) registers the MCP tools and skills.
