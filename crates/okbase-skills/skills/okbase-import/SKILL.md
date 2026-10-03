---
name: okbase-import
description: Work with source documents in the knowledge bundle at {{bundle}} (PDF, Word, PowerPoint, OpenDocument, RTF, EPUB, HTML, images): transcribe scanned pages, import them as markdown, and distill them into clean documents. Use when the bundle holds such files, when a page "needs OCR", or when asked to import or convert documents.
---

# Source documents

okbase reads PDF, Word, PowerPoint, OpenDocument, RTF, EPUB, HTML and text files **directly**: they are searchable with the `{{p}}_*` tools as they are, and nothing is written to the folder. Their ids keep the extension (`manuals/printer.pdf`); PDF pages are marked with a `page N` comment, so cite the file and the page.

1. **See what is there:** `okbase --bundle {{bundle}} import` lists every source, what it would become and the pages without text. It writes nothing.
2. **Pages without text** (scans, photos, images): okbase has no OCR and never sends files to an OCR service. Ask the user once whether those pages may be sent to your model provider. If yes, loop:
   - `okbase --bundle {{bundle}} import ocr-next` names a file and page and, for scans, gives an `image` of the page (in okbase's state directory);
   - open that image (or the PDF page when no image is given);
   - transcribe it as markdown in its original language: headings, lists, tables, every number, only what is on the page;
   - `okbase --bundle {{bundle}} import ocr-submit <file> --page <n> -` with the markdown on stdin.
3. **Multi-column pages** (listed by `import`) may have their lines interleaved: check them against the original before relying on them.
4. **Markdown copies** are only needed to edit or curate: ask the user, then `okbase --bundle {{bundle}} import --write` (into `sources/`). Re-running it updates files whose source changed and keeps any you edited (it reports them; `--force` only with the user's consent).
5. **Distill** a source into a clean document when the user wants one: `okbase --bundle {{bundle}} new --type <Type> "<Title>" --description "<one sentence>" --source <source path>`, then write the essentials with every number checked against the source. The source stays as the reference.
