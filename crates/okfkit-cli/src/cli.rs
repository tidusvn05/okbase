//! Command-line arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Make markdown knowledge bundles (Open Knowledge Format) work well for AI agents.
///
/// Read commands index the bundle incrementally first; no setup is needed.
/// Every read command accepts --json (the same JSON as the MCP tools).
#[derive(Debug, Parser)]
#[command(
    name = "okfkit",
    version,
    propagate_version = true,
    max_term_width = 100
)]
pub struct Cli {
    /// Bundle directory [default: current directory].
    #[arg(
        long,
        short = 'b',
        global = true,
        env = "OKFKIT_BUNDLE",
        value_name = "DIR"
    )]
    pub bundle: Option<PathBuf>,

    /// Where to keep the index: auto (<bundle>/.okfkit if writable, else the user cache), cache, or a directory.
    #[arg(
        long,
        global = true,
        env = "OKFKIT_STATE_DIR",
        value_name = "auto|cache|DIR"
    )]
    pub state_dir: Option<String>,

    /// Print JSON instead of text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Only show documents matching this path glob (repeatable).
    #[arg(long, global = true, value_name = "GLOB")]
    pub allow: Vec<String>,

    /// Hide documents matching this path glob (repeatable).
    #[arg(long, global = true, value_name = "GLOB")]
    pub deny: Vec<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Index the bundle and show a summary: size, types, languages, level and recommended mode.
    Status,
    /// Recommend how to use okfkit for this bundle, from the simplest setup up (read-only).
    #[command(
        after_help = "Examples:\n  okfkit advise\n  okfkit advise --user-langs vi,ja --for claude\n  okfkit advise --for team --private --json"
    )]
    Advise {
        /// Languages people ask in (comma-separated ISO codes) [default: the bundle's languages].
        #[arg(long, value_delimiter = ',', value_name = "LANGS")]
        user_langs: Vec<String>,
        /// Who will use the bundle.
        #[arg(long = "for", value_enum, value_name = "WHO")]
        audience: Option<AudienceArg>,
        /// Documents must not be sent to a cloud LLM.
        #[arg(long)]
        private: bool,
    },
    /// Update the index (only changed files are re-read).
    Index {
        /// Delete the index and build it from scratch.
        #[arg(long)]
        rebuild: bool,
    },
    /// Regex search over all documents, case- and accent-insensitive.
    #[command(
        after_help = "Examples:\n  okfkit grep 'refund|đổi trả' --files-only\n  okfkit grep 'dmScope|dm_scope' --path 'gateway/**' --context 2"
    )]
    Grep {
        /// Regular expression; use alternation (a|b) for synonyms.
        pattern: String,
        /// Path glob or prefix, e.g. 'gateway/**'.
        #[arg(long)]
        path: Option<String>,
        /// Lines of context around each match (0-5).
        #[arg(long, short = 'C', default_value_t = 1)]
        context: usize,
        /// List matching documents ranked by match count.
        #[arg(long, short = 'l')]
        files_only: bool,
        /// Maximum output lines.
        #[arg(long, default_value_t = 40)]
        limit: usize,
        #[command(flatten)]
        filter: FilterArgs,
    },
    /// Read a document, one section, or a line range.
    Get {
        /// Document id (path without .md).
        id: String,
        /// Heading text of the section to return.
        #[arg(long, short = 's')]
        section: Option<String>,
        /// Line range, e.g. 10-40.
        #[arg(long)]
        lines: Option<String>,
        /// Token budget (estimated) [default: 4000].
        #[arg(long)]
        max_tokens: Option<usize>,
    },
    /// Show a directory's index.md (or a generated listing).
    List {
        /// Directory [default: the root].
        dir: Option<String>,
    },
    /// Filter documents by metadata; count, facet and sum.
    #[command(
        after_help = "Examples:\n  okfkit query --type Contract --active-on 2026-01-15 --facet region\n  okfkit query --field region=VN --sum contract_value --count-only"
    )]
    Query {
        #[command(flatten)]
        filter: FilterArgs,
        /// Sort field, '-' prefix for descending (e.g. -updated).
        #[arg(long)]
        sort: Option<String>,
        /// Maximum rows [default: 50].
        #[arg(long)]
        limit: Option<usize>,
        /// Only counts, facets and sums.
        #[arg(long)]
        count_only: bool,
        /// Count values of this field over the results (repeatable).
        #[arg(long, value_name = "FIELD")]
        facet: Vec<String>,
        /// Sum this numeric field over the results.
        #[arg(long, value_name = "FIELD")]
        sum: Option<String>,
    },
    /// Prompt-ready catalog: documents, tag vocabulary and facet counts.
    Catalog {
        /// Largest flat catalog in tokens before switching to the root index [default: 12000].
        #[arg(long)]
        max_tokens: Option<usize>,
    },
    /// Links from a document and backlinks to it.
    Links {
        /// Document id.
        id: String,
    },
    /// Check the bundle against the okfkit levels (L0-L3). Exits with 1 when there are errors.
    Lint {
        /// Target level.
        #[arg(long, value_enum, default_value_t = LevelArg::L2)]
        level: LevelArg,
        /// Output format (--json is the same as --format json).
        #[arg(long, value_enum, default_value_t = LintFormat::Text)]
        format: LintFormat,
        /// Apply safe fixes first (writes files): create missing index.md files, use canonical tags.
        #[arg(long)]
        fix_safe: bool,
        /// Skip a rule by code (repeatable).
        #[arg(long, value_name = "CODE")]
        disable: Vec<String>,
    },
    /// Turn a folder of plain markdown into an OKF bundle (fills missing frontmatter, creates index.md).
    #[command(
        after_help = "Examples:\n  okfkit adopt ./docs                 # show the plan, write nothing\n  okfkit adopt ./docs --out ./docs-okf\n  okfkit adopt ./docs --write --level L2"
    )]
    Adopt {
        /// The folder [default: --bundle or the current directory].
        dir: Option<PathBuf>,
        /// Write the result to this new directory (the source is not modified).
        #[arg(long, value_name = "DIR", conflicts_with = "write")]
        out: Option<PathBuf>,
        /// Edit the folder in place (needs a clean git working tree, or --force).
        #[arg(long)]
        write: bool,
        /// Edit in place even if the folder is not a clean git working tree.
        #[arg(long, requires = "write")]
        force: bool,
        /// Target level: L1 (titles, descriptions, index.md) or L2 (also lang, status, updated, tags).
        #[arg(long, value_enum, default_value_t = LevelArg::L1)]
        level: LevelArg,
        /// List every file change.
        #[arg(long, short = 'v')]
        verbose: bool,
    },
    /// Show tag usage against the vocabulary, or suggest a _meta/vocabulary.md.
    Vocab {
        /// Print a suggested _meta/vocabulary.md built from the tags in use.
        #[arg(long)]
        suggest: bool,
        /// With --suggest: write it to _meta/vocabulary.md (only if that file does not exist).
        #[arg(long, requires = "suggest")]
        write: bool,
    },
    /// Semantic search (needs embeddings: `okfkit embed enable`, okfkit-full build).
    Search {
        /// The question, in any language.
        query: String,
        /// Maximum hits.
        #[arg(long, short = 'k', default_value_t = 8)]
        limit: usize,
        #[command(flatten)]
        filter: FilterArgs,
    },
    /// The best sections for a question within a token budget, formatted for a prompt.
    Retrieve {
        /// The question.
        query: String,
        /// Token budget (estimated).
        #[arg(long, default_value_t = 3000)]
        budget: usize,
    },
    /// Embeddings for semantic search (opt-in; local models need the okfkit-full build).
    Embed {
        #[command(subcommand)]
        command: EmbedCmd,
    },
    /// The Japanese dictionary (downloaded on first use; install it ahead of time for offline use).
    Dict {
        #[command(subcommand)]
        command: DictCmd,
    },
    /// Read-only SQL over the bundle's spreadsheets (CSV, TSV, XLSX).
    Data {
        #[command(subcommand)]
        command: DataCmd,
    },
    /// MCP server.
    Mcp {
        #[command(subcommand)]
        command: McpCmd,
    },
    /// Set up agent CLIs (Claude Code, Codex) to use this bundle.
    Agent {
        #[command(subcommand)]
        command: AgentCmd,
    },
    /// List modules and their capabilities.
    Modules,
    /// Run an `okfkit-<name>` plugin from PATH.
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Who will use the bundle (`advise --for`).
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AudienceArg {
    /// Claude Code on this machine.
    Claude,
    /// OpenAI Codex CLI on this machine.
    Codex,
    /// A team sharing one MCP server.
    Team,
    /// An application embedding okfkit.
    Host,
}

#[derive(Debug, Subcommand)]
pub enum EmbedCmd {
    /// List the supported local models with license, size and spike quality.
    Models,
    /// Turn embeddings on for this bundle (writes okfkit.toml).
    #[command(
        after_help = "Examples:\n  okfkit embed enable --accept-license                 # EmbeddingGemma 300M Q4 (Gemma terms)\n  okfkit embed enable --model bge-m3-int8              # MIT, no acceptance needed\n  okfkit embed enable --api-url https://api.openai.com/v1 --api-model text-embedding-3-small"
    )]
    Enable {
        /// Local model id.
        #[arg(
            long,
            default_value = "embeddinggemma-300m-q4",
            conflicts_with = "api_url"
        )]
        model: String,
        /// Accept the model's license (required for models with their own terms, such as Gemma).
        #[arg(long)]
        accept_license: bool,
        /// Use an OpenAI-compatible API instead of a local model.
        #[arg(long, value_name = "URL", requires = "api_model")]
        api_url: Option<String>,
        /// API model name.
        #[arg(long, value_name = "NAME")]
        api_model: Option<String>,
        /// Environment variable holding the API key.
        #[arg(long, value_name = "VAR", default_value = "OPENAI_API_KEY")]
        api_key_env: String,
    },
    /// Turn embeddings off (writes okfkit.toml; vectors stay cached).
    Disable,
    /// Show the model and how many chunks are embedded.
    Status,
    /// Embed the chunks that have no vector yet (first run downloads the model).
    Index,
}

#[derive(Debug, Subcommand)]
pub enum DictCmd {
    /// Show whether the dictionary is installed and where.
    Status,
    /// Download (checksum-verified) and build the dictionary now.
    Install,
}

#[derive(Debug, Subcommand)]
pub enum DataCmd {
    /// List the dataset tables with columns and row counts.
    Tables,
    /// Run one read-only SELECT.
    #[command(
        after_help = "Example:\n  okfkit data sql \"SELECT region, SUM(revenue) FROM sales_2026 GROUP BY region\""
    )]
    Sql {
        /// The query.
        query: String,
        /// Maximum rows returned.
        #[arg(long, default_value_t = 100)]
        max_rows: usize,
        /// Time limit in seconds.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },
}

#[derive(Debug, Subcommand)]
pub enum McpCmd {
    /// Serve the bundle's tools over MCP.
    #[command(
        after_help = "Examples:\n  okfkit mcp serve                          # stdio (for claude/codex)\n  okfkit mcp serve --http                   # http://127.0.0.1:7331/mcp\n  OKFKIT_MCP_TOKEN=... okfkit mcp serve --http 0.0.0.0:7331 --allow-host kb.example.com"
    )]
    Serve {
        /// Use stdio (the default).
        #[arg(long, conflicts_with = "http")]
        stdio: bool,
        /// Serve streamable HTTP at this address instead of stdio.
        #[arg(long, value_name = "ADDR", num_args = 0..=1, default_missing_value = "127.0.0.1:7331")]
        http: Option<std::net::SocketAddr>,
        /// Accept this Host header (repeatable; loopback hosts are always accepted).
        #[arg(long, value_name = "HOST", requires = "http")]
        allow_host: Vec<String>,
        /// Environment variable holding the bearer token clients must send (required off loopback).
        #[arg(
            long,
            value_name = "VAR",
            default_value = "OKFKIT_MCP_TOKEN",
            requires = "http"
        )]
        token_env: String,
        /// Tool name prefix.
        #[arg(long, default_value = "kb")]
        prefix: String,
        /// Hide a tool by name, without prefix (repeatable).
        #[arg(long, value_name = "TOOL")]
        disable: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AgentCmd {
    /// Register the MCP server and install the okfkit skills.
    #[command(
        after_help = "Examples:\n  okfkit -b ./kb agent install --claude          # this project (.mcp.json, .claude/skills)\n  okfkit -b ./kb agent install --claude --user   # all projects\n  okfkit -b ./kb agent install --codex --print   # show the changes only"
    )]
    Install {
        /// Claude Code.
        #[arg(long)]
        claude: bool,
        /// OpenAI Codex CLI.
        #[arg(long)]
        codex: bool,
        /// Install for the user instead of the project.
        #[arg(long)]
        user: bool,
        /// Project directory [default: current directory].
        #[arg(long, value_name = "DIR")]
        project: Option<PathBuf>,
        /// Print the changes instead of making them.
        #[arg(long)]
        print: bool,
        /// MCP server name.
        #[arg(long, default_value = "okfkit")]
        name: String,
        /// Tool name prefix used in the skill.
        #[arg(long, default_value = "kb")]
        prefix: String,
    },
}

/// Metadata filters shared by `grep` and `query`.
#[derive(Debug, Args, Default)]
pub struct FilterArgs {
    /// Document type (repeatable: any of).
    #[arg(long, value_name = "TYPE")]
    pub r#type: Vec<String>,
    /// Required tag (repeatable: all of); synonyms and accents are resolved.
    #[arg(long, value_name = "TAG")]
    pub tag: Vec<String>,
    /// Tag (repeatable: any of).
    #[arg(long, value_name = "TAG")]
    pub any_tag: Vec<String>,
    /// Status (repeatable: any of).
    #[arg(long)]
    pub status: Vec<String>,
    /// Excluded status (repeatable).
    #[arg(long)]
    pub status_not: Vec<String>,
    /// Language (repeatable: any of).
    #[arg(long)]
    pub lang: Vec<String>,
    /// Other frontmatter field, KEY=VALUE[,VALUE] (repeatable).
    #[arg(long, value_name = "KEY=VALUE")]
    pub field: Vec<String>,
    /// Id prefix or glob.
    #[arg(long, value_name = "GLOB")]
    pub under: Option<String>,
    /// Substring of id, title or description.
    #[arg(long)]
    pub text: Option<String>,
    /// Updated on or after (YYYY-MM-DD).
    #[arg(long, value_name = "DATE")]
    pub updated_from: Option<String>,
    /// Updated on or before (YYYY-MM-DD).
    #[arg(long, value_name = "DATE")]
    pub updated_to: Option<String>,
    /// In force on this date (YYYY-MM-DD).
    #[arg(long, value_name = "DATE")]
    pub active_on: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LevelArg {
    #[value(name = "L0", alias = "l0")]
    L0,
    #[value(name = "L1", alias = "l1")]
    L1,
    #[value(name = "L2", alias = "l2")]
    L2,
    #[value(name = "L3", alias = "l3")]
    L3,
}

impl From<LevelArg> for okfkit::Level {
    fn from(l: LevelArg) -> Self {
        match l {
            LevelArg::L0 => okfkit::Level::L0,
            LevelArg::L1 => okfkit::Level::L1,
            LevelArg::L2 => okfkit::Level::L2,
            LevelArg::L3 => okfkit::Level::L3,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LintFormat {
    Text,
    Json,
    Sarif,
}
