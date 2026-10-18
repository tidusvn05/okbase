//! Built-in, offline starter bundles. Assets are embedded, never fetched or evaluated as code.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Error, scaffold};

const ASSETS: &[(&str, &str)] = &[
    (
        "software-project",
        include_str!("../templates/software-project.yaml"),
    ),
    (
        "personal-assistant",
        include_str!("../templates/personal-assistant.yaml"),
    ),
    (
        "project-assistant",
        include_str!("../templates/project-assistant.yaml"),
    ),
    ("infra-ops", include_str!("../templates/infra-ops.yaml")),
    (
        "product-management",
        include_str!("../templates/product-management.yaml"),
    ),
    (
        "engineering-lead",
        include_str!("../templates/engineering-lead.yaml"),
    ),
    ("executive", include_str!("../templates/executive.yaml")),
    (
        "team-handbook",
        include_str!("../templates/team-handbook.yaml"),
    ),
    (
        "customer-support",
        include_str!("../templates/customer-support.yaml"),
    ),
    ("research", include_str!("../templates/research.yaml")),
];

/// Discoverable template metadata; no bundle is opened to list these.
#[derive(Debug, Serialize)]
pub struct Summary {
    /// Stable CLI identifier.
    pub id: String,
    /// Default bundle title.
    pub title: String,
    /// What the template helps with.
    pub description: String,
    /// Intended audiences, not access-control roles.
    pub roles: Vec<String>,
    /// Document types included in the bundle.
    pub types: Vec<String>,
}

/// An embedded starter bundle. Its manifest is internal, not a user template format.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    id: String,
    title: String,
    description: String,
    roles: Vec<String>,
    schemas: BTreeMap<String, BTreeMap<String, Value>>,
    vocabulary: BTreeMap<String, Vec<String>>,
    documents: Vec<Document>,
    /// Five grounded questions, also used by the offline acceptance tests.
    pub questions: Vec<Question>,
    workflows: Vec<Workflow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    title: String,
    description: String,
    tags: Vec<String>,
    fields: BTreeMap<String, Value>,
    body: String,
}

/// A trial question with its evidence in a particular source document.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// Natural-language question to ask the agent.
    pub question: String,
    /// Bundle-relative document ID containing the answer.
    pub source: String,
    /// Literal supporting passage; acceptance checks read the actual indexed source.
    pub evidence: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Workflow {
    id: String,
    title: String,
    description: String,
    steps: Vec<String>,
    sources: Vec<String>,
}

/// Lists the shipped templates in display order, without opening a bundle or writing state.
pub fn catalog() -> Result<Vec<Summary>, Error> {
    ASSETS
        .iter()
        .map(|(id, _)| Ok(load(id)?.summary()))
        .collect()
}

/// Loads one embedded template; a name is never interpreted as a path or URL.
pub fn load(id: &str) -> Result<Template, Error> {
    let (_, asset) = ASSETS.iter().find(|(name, _)| *name == id).ok_or_else(|| {
        Error::Invalid(format!(
            "invalid argument: unknown template {id:?}; run `okbase templates`"
        ))
    })?;
    let template: Template = serde_saphyr::from_str(asset)
        .map_err(|e| Error::Invalid(format!("invalid embedded template {id}: {e}")))?;
    if template.id != id {
        return Err(Error::Invalid(format!(
            "invalid embedded template identifier {id}"
        )));
    }
    Ok(template)
}

fn yaml(value: impl Serialize) -> String {
    // JSON flow scalars, sequences and maps are valid YAML, with unambiguous string quoting.
    serde_json::to_string(&value).expect("template fields serialize as JSON")
}

fn document(d: &Document, today: &str, example: bool) -> String {
    let mut tags = d.tags.clone();
    if example {
        tags.push("example".into());
    }
    let mut out = format!(
        "---\ntype: {}\ntitle: {}\ndescription: {}\ntags: {}\nlang: en\nstatus: {}\nupdated: {}\nexample: {example}\n",
        yaml(&d.kind),
        yaml(&d.title),
        yaml(&d.description),
        yaml(tags),
        if example { "draft" } else { "stable" },
        yaml(today),
    );
    for (key, value) in &d.fields {
        let _ = writeln!(out, "{}: {}", yaml(key), yaml(value));
    }
    let _ = write!(out, "---\n# {}\n\n", d.title);
    if example {
        out.push_str("> Fictional example: replace this record with your own information before relying on it.\n\n");
    }
    let _ = writeln!(out, "{}", d.body.trim());
    out
}

fn guide(title: &str, description: &str, tag: &str, body: &str, today: &str) -> String {
    document(
        &Document {
            path: String::new(),
            kind: "Guide".into(),
            title: title.into(),
            description: description.into(),
            tags: vec![tag.into()],
            fields: BTreeMap::new(),
            body: body.into(),
        },
        today,
        false,
    )
}

impl Template {
    /// Metadata for humans and agents choosing a starting point.
    pub fn summary(&self) -> Summary {
        Summary {
            id: self.id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            roles: self.roles.clone(),
            types: self.schemas.keys().cloned().collect(),
        }
    }

    /// Plans a complete L2 bundle; does not create directories or write files.
    pub fn files(
        &self,
        root: &Path,
        options: &scaffold::InitOptions,
    ) -> Result<Vec<(String, String)>, Error> {
        let mut options = options.clone();
        if options.title.is_empty() {
            options.title.clone_from(&self.title);
        }
        if options.description.is_empty() {
            options.description.clone_from(&self.description);
        }
        options.types.extend(self.schemas.keys().cloned());
        options.types.sort();
        options.types.dedup();
        let mut files: BTreeMap<String, String> =
            scaffold::init_files(root, &options)?.into_iter().collect();
        for (kind, fields) in &self.schemas {
            scaffold::validate_type(kind)?;
            files.insert(format!("_meta/types/{kind}.md"), format!("---\ntype: Type Schema\ntitle: {}\ndescription: {}\nfields: {}\n---\n\nExtend these fields for your own {kind} records; keep OKF status separate from workflow state.\n", yaml(kind), yaml(format!("Fields used to organize {kind} records in this knowledge base.")), yaml(fields)));
        }
        let mut vocabulary = self.vocabulary.clone();
        for term in ["getting-started", "workflow", "example"] {
            vocabulary.entry(term.into()).or_default();
        }
        for d in &self.documents {
            if !self.schemas.contains_key(&d.kind) {
                return Err(Error::Invalid(format!(
                    "template {} has no schema for {}",
                    self.id, d.kind
                )));
            }
            for tag in &d.tags {
                vocabulary.entry(tag.clone()).or_default();
            }
            files.insert(d.path.clone(), document(d, &options.today, true));
        }
        let terms: BTreeMap<_, _> = vocabulary
            .into_iter()
            .map(|(tag, synonyms)| (tag, json!({"synonyms": synonyms, "facet": "topic"})))
            .collect();
        files.insert("_meta/vocabulary.md".into(), format!("---\ntype: Vocabulary\ntitle: Tag vocabulary\ndescription: Canonical topics and synonyms for this starter knowledge base.\nterms: {}\n---\n\nAdd a canonical term before using a new tag. Edit synonyms to match the languages people ask in.\n", yaml(terms)));

        let mut start = format!(
            "This bundle was created from the `{}` template. All domain records are fictional English examples (`example: true`, `status: draft`). Changing --langs configures languages for new documents and advice; it does not translate these examples.\n\n## Try it\n\nRun commands from this bundle's directory, or add `-b <path>` to each command.\n\n```sh\nokbase status\nokbase catalog\nokbase lint --level L2\nokbase onboard\n```\n\nAsk your agent these questions; require a citation to the linked source, rather than guessing.\n",
            self.id
        );
        for q in &self.questions {
            let _ = write!(
                start,
                "\n### {}\n\nExpected evidence: {}\n\nSource: [{}](../{}.md).\n",
                q.question, q.evidence, q.source, q.source
            );
        }
        start.push_str("\n## Workflows\n\n");
        for w in &self.workflows {
            let path = format!("workflows/{}.md", w.id);
            let mut body = String::from(
                "Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.\n\n",
            );
            for (i, step) in w.steps.iter().enumerate() {
                let _ = writeln!(body, "{}. {step}", i + 1);
            }
            body.push_str("\n## Source records\n\n");
            for source in &w.sources {
                let _ = writeln!(body, "- [{source}](../{source}.md)");
            }
            body.push_str("\n## Agent prompt\n\nRead this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.\n\nExternal actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.\n");
            files.insert(
                path,
                guide(&w.title, &w.description, "workflow", &body, &options.today),
            );
            let _ = writeln!(
                start,
                "- [{}](../workflows/{}.md): {}",
                w.title, w.id, w.description
            );
        }
        start.push_str("\n## Make it yours\n\n1. Replace the domain examples with real records. Verify their facts and sources, remove `example: true`, and use `status: stable` only after approval. Remove examples you do not need and update their folder listings and links.\n2. Inspect `_meta/types/` and `_meta/vocabulary.md`. Create a record with `okbase new --type <Type> \"<Title>\" --description \"<what it answers>\" --tags <existing-tag>`. Fill every required field reported by the command, then write its body and links.\n3. Add new fields or types in `_meta/types/<Type>.md`. Declare a new canonical tag in `_meta/vocabulary.md` before using it. Keep `status` as draft/stable/deprecated; use `task_status` or `incident_status` for work in progress.\n4. Re-run `okbase lint --level L2` and the relevant trial questions after edits. Reports must name their reporting period, evidence, and unresolved unknowns. Re-running init never updates an existing bundle; upgrading okbase does not rewrite these files.\n\n## Several bundles and bot hosts\n\nConnect another bundle with its own server name, for example `okbase -b ../operations agent install --name okbase-ops`. Query each bundle through its own tool prefix; cross-bundle references do not become local Markdown links.\n\nFor a bot, use MCP or the CLI's `--json` output, or the Rust library with a host-provided Scope. Put the scoped catalog in the bot's context. The host decides who may read which documents and provides authorized file-writing tools. MCP tools currently read knowledge; they do not write tasks, deploy services or run schedules. Credentials belong in the host's secret configuration, not knowledge records.\n");
        files.insert(
            "guides/getting-started.md".into(),
            guide(
                "Getting started",
                "Try this knowledge base, follow its workflows and replace the fictional examples.",
                "getting-started",
                &start,
                &options.today,
            ),
        );

        // Listing files are generated from the final document set, never copied between bundles.
        let mut listings: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for (path, content) in &files {
            if path.starts_with("_meta/")
                || !path.ends_with(".md")
                || path == "index.md"
                || path == "log.md"
            {
                continue;
            }
            let concept = okbase_core::Concept::parse(Path::new(path), content)?;
            let (dir, filename) = path.rsplit_once('/').unwrap_or(("", path));
            listings.entry(dir.into()).or_default().push((
                concept
                    .frontmatter
                    .get_str("title")
                    .unwrap_or(filename)
                    .into(),
                filename.into(),
            ));
        }
        let mut directories: BTreeSet<String> = listings.keys().cloned().collect();
        for dir in directories.clone() {
            let mut dir = dir.as_str();
            while let Some((parent, _)) = dir.rsplit_once('/') {
                directories.insert(parent.into());
                dir = parent;
            }
        }
        directories.insert(String::new());
        for dir in &directories {
            let mut index = if dir.is_empty() {
                format!(
                    "---\nokf_version: \"0.2\"\n---\n# {}\n\n{}\n\nThis bundle includes fictional examples. Start with [Getting started](guides/getting-started.md).\n\n",
                    options.title, options.description
                )
            } else {
                format!("# {dir}\n\n")
            };
            let prefix = if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            };
            for child in &directories {
                if let Some(name) = child
                    .strip_prefix(&prefix)
                    .filter(|s| !s.is_empty() && !s.contains('/'))
                {
                    let _ = writeln!(index, "* [{name}]({name}/index.md)");
                }
            }
            for (title, filename) in listings.get(dir).into_iter().flatten() {
                let _ = writeln!(index, "* [{title}]({filename})");
            }
            files.insert(format!("{prefix}index.md"), index);
        }
        files.insert("log.md".into(), format!("# Update log\n\n## {}\n* **Created** from template `{}` with okbase {}. Domain records are fictional examples; these files are now managed by the user.\n", options.today, self.id, env!("CARGO_PKG_VERSION")));
        let files: Vec<_> = files.into_iter().collect();
        scaffold::check_new(root, &files, &[])?;
        Ok(files)
    }
}
