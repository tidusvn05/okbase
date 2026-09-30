---
name: okfkit-answer
description: Answer questions from the knowledge bundle at {{bundle}} (markdown documents with metadata) using the okfkit tools. Use it for any question whose answer should come from that bundle, including listing, counting or filtering documents.
---

# Answering from the knowledge bundle

The bundle at `{{bundle}}` is served by okfkit through the MCP tools `{{p}}_*`.

## Steps

1. **Orient.** If the bundle catalog is not already in your context, call `{{p}}_catalog` (or `{{p}}_list` with `dir: "."`).
2. **Choose the tool by question type.**
   - *List, count, "which documents", filter by type, tag, status, language, date or any field:* use `{{p}}_query`. Use `facets` for breakdowns, `count_only` for counts and `sum_field` for totals. Do not count or add up by reading documents.
<!-- if data.sql -->
   - *Numbers in spreadsheets (inventory, sales, prices):* use `data_tables`, then `data_query` with SQL aggregates (SUM, COUNT, GROUP BY). Never add numbers up yourself.
<!-- end -->
   - *Exact strings (config keys, codes, error text, names):* use `{{p}}_grep` with an alternation of likely spellings and synonyms (`'dmScope|dm_scope|direct message'`). Start with `files_only: true` to rank documents, then read the best ones.
<!-- if embed.search -->
   - *Questions about meaning:* use `{{p}}_search` first; it understands any language.
<!-- end -->
<!-- if !embed.search -->
   - *Questions about meaning:* translate the key terms into the documents' language(s), then use `{{p}}_grep` with alternation and `{{p}}_list` to browse the relevant directory.
<!-- end -->
3. **Read.** Use `{{p}}_get` on the best documents. For long documents pass `section` (a heading); a truncated result lists the headings.
4. **Verify.** Every key, value, limit, date or command you state must appear in text you have read. If the question asks several things, cover each one: details are often spread over a guide, a reference page and a FAQ. `{{p}}_links` shows related documents.
5. **Prefer current documents.** `status: stable` is in force, `deprecated` is superseded (see `supersedes`), `draft` is not approved yet. For "in force on a date" use `active_on`.
6. **Answer** in the user's language, concisely. Keep keys, commands and values verbatim, and cite the document ids you used (path without `.md`).

## Without the MCP tools

If the `{{p}}_*` tools are not available, run the okfkit CLI; add `--json` for structured output:

```sh
okfkit --bundle {{bundle}} catalog
okfkit --bundle {{bundle}} list [DIR]
okfkit --bundle {{bundle}} grep 'term|synonym' --files-only [--path 'dir/**'] [--context 2]
okfkit --bundle {{bundle}} get ID [--section HEADING] [--lines 10-40]
okfkit --bundle {{bundle}} query [--type T] [--tag T] [--status stable] [--field KEY=VALUE] [--active-on YYYY-MM-DD] [--facet F] [--sum F] [--count-only]
okfkit --bundle {{bundle}} links ID
```
