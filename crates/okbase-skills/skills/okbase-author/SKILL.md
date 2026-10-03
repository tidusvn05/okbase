---
name: okbase-author
description: Write a new document for the knowledge bundle at {{bundle}} (a policy, guide, FAQ, reference…) with the right type, title, description, tags and links, or start the bundle in an empty folder. Use when asked to add, write or document something in the knowledge base.
---

# Writing a new document

1. **Empty folder?** Ask the user what the knowledge base is about and which languages people use, then run `okbase --bundle {{bundle}} init --title "<topic>" --langs <vi,en,…>`.
2. **Check what exists first.** Search with `{{p}}_grep` / `{{p}}_search` for the topic: update an existing document instead of creating a duplicate (duplicates are the main cause of wrong answers). Pick the type from `_meta/types/` and tags from `_meta/vocabulary.md`.
3. **Create it:** `okbase --bundle {{bundle}} new --type <Type> "<Title>" --description "<one specific sentence: what this document answers>" --tags <tag,…>`. It writes the frontmatter (with `status: draft`) and lists the document in its folder's index.md.
4. **Write the body** in the bundle's language: short sections with `##` headings, concrete values (numbers, dates, names), one topic per document. Link related documents with relative links. Fill every field the command said is required.
5. **New tag?** Add it to `_meta/vocabulary.md` with synonyms in every language people ask in, rather than inventing one-off tags.
6. **Check:** `okbase --bundle {{bundle}} lint --level L2` must show no errors for the new file. Leave `status: draft` until the user approves; then set `stable` (and `supersedes` if it replaces an older document).
