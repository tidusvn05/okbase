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
    Serve {
        /// Use stdio (the only transport in this version).
        #[arg(long, default_value_t = true)]
        stdio: bool,
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
