use std::borrow::Cow;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::PathBuf;

use rustyline::completion::{Completer, Pair};
use rustyline::config::Builder as ConfigBuilder;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hint, Hinter};
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use moerae::types::Scope;
use moerae::Moerae;

const COMMANDS: &[(&str, &str)] = &[
    ("/put ", "Store content"),
    ("/put --persist ", "Store persistent content"),
    ("/search ", "Search"),
    ("/search --project ", "Search project scope"),
    ("/search --limit ", "Search with limit"),
    ("/get ", "Get node by ID"),
    ("/stats", "Show project statistics"),
    ("/convs", "List conversations"),
    ("/conversations", "List conversations"),
    ("/new", "Start new conversation"),
    ("/switch ", "Switch conversation"),
    ("/delete ", "Delete conversation by prefix"),
    ("/purge", "Delete all conversations except active"),
    ("/project ", "Switch to a project"),
    ("/projects", "List all projects"),
    ("/delete_project ", "Delete a project by name"),
    ("/verbose", "Toggle llama.cpp logs"),
    ("/help", "Show help"),
    ("/quit", "Exit"),
    ("/exit", "Exit"),
    ("/q", "Exit"),
];

struct MoeraeHelper;

impl Completer for MoeraeHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        if !line.starts_with('/') {
            return Ok((0, vec![]));
        }

        let input = &line[..pos];
        let matches: Vec<Pair> = COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(input))
            .map(|(cmd, desc)| Pair {
                display: format!("{cmd:<30} {desc}"),
                replacement: cmd.to_string(),
            })
            .collect();

        Ok((0, matches))
    }
}

impl Hinter for MoeraeHelper {
    type Hint = CommandHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        if !line.starts_with('/') || pos < line.len() {
            return None;
        }

        COMMANDS
            .iter()
            .find(|(cmd, _)| cmd.starts_with(line) && *cmd != line)
            .map(|(cmd, _)| CommandHint(cmd[line.len()..].to_string()))
    }
}

struct CommandHint(String);

impl Hint for CommandHint {
    fn display(&self) -> &str {
        &self.0
    }

    fn completion(&self) -> Option<&str> {
        Some(&self.0)
    }
}

impl Highlighter for MoeraeHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[90m{hint}\x1b[0m"))
    }
}

impl Validator for MoeraeHelper {}
impl Helper for MoeraeHelper {}

// --- Stderr redirection for log control ---

struct StderrGuard {
    saved_fd: OwnedFd,
    devnull_fd: OwnedFd,
    verbose: bool,
}

impl StderrGuard {
    fn new() -> Self {
        let saved_fd = unsafe { OwnedFd::from_raw_fd(libc::dup(libc::STDERR_FILENO)) };
        let devnull = std::fs::File::open("/dev/null").expect("cannot open /dev/null");
        let devnull_fd = OwnedFd::from(devnull);

        let mut guard = Self {
            saved_fd,
            devnull_fd,
            verbose: false,
        };
        guard.silence();
        guard
    }

    fn silence(&mut self) {
        unsafe {
            libc::dup2(self.devnull_fd.as_raw_fd(), libc::STDERR_FILENO);
        }
        self.verbose = false;
    }

    fn restore(&mut self) {
        unsafe {
            libc::dup2(self.saved_fd.as_raw_fd(), libc::STDERR_FILENO);
        }
        self.verbose = true;
    }

    fn toggle(&mut self) {
        if self.verbose {
            self.silence();
        } else {
            self.restore();
        }
    }

    fn is_verbose(&self) -> bool {
        self.verbose
    }
}

impl Drop for StderrGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

// --- Project discovery ---

fn moerae_base() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".moerae")
    } else {
        PathBuf::from(".moerae")
    }
}

fn history_path() -> PathBuf {
    moerae_base().join("repl_history.txt")
}

struct ProjectInfo {
    name: String,
    #[allow(dead_code)]
    hash_dir: String,
    conversations: i64,
    nodes: i64,
}

fn list_projects_full() -> Vec<ProjectInfo> {
    let projects_dir = moerae_base().join("projects");
    let mut result = vec![];

    let entries = match std::fs::read_dir(&projects_dir) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let db_path = path.join("moerae.db");
        if !db_path.exists() {
            continue;
        }

        let hash_dir = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();

        if let Ok(conn) = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            // Try to read stored project name from config
            let name = conn
                .query_row("SELECT data FROM config WHERE id = 1", [], |r| {
                    r.get::<_, String>(0)
                })
                .ok()
                .and_then(|json| {
                    serde_json::from_str::<serde_json::Value>(&json)
                        .ok()?
                        .get("_project_id")?
                        .as_str()
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| hash_dir.clone());

            let conversations: i64 = conn
                .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
                .unwrap_or(0);
            let nodes: i64 = conn
                .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
                .unwrap_or(0);

            result.push(ProjectInfo {
                name,
                hash_dir,
                conversations,
                nodes,
            });
        }
    }
    result
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let initial_project = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .cloned()
        .unwrap_or_else(|| "repl".into());

    let mut log_guard = StderrGuard::new();

    let config = ConfigBuilder::new()
        .auto_add_history(true)
        .max_history_size(1000)
        .unwrap()
        .build();

    let mut rl: Editor<MoeraeHelper, DefaultHistory> = Editor::with_config(config).unwrap();
    rl.set_helper(Some(MoeraeHelper));

    let hist_path = history_path();
    if let Some(parent) = hist_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = rl.load_history(&hist_path);

    let mut project_id = initial_project;
    let mut m = init_project(&project_id, &mut log_guard);

    let mut conv = m.create_conversation().unwrap();

    println!("Conversation: {}", &conv.uuid[..8]);
    println!();
    print_help();

    loop {
        let prompt = format!("{}> ", project_id);
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                match parse_command(line) {
                    Cmd::Put {
                        data,
                        metadata,
                        persist,
                    } => match conv.put(&data, metadata.as_deref(), persist) {
                        Ok(()) => {
                            let tag = if persist { " [persist]" } else { "" };
                            println!("  stored{tag}");
                        }
                        Err(e) => println!("  error: {e}"),
                    },
                    Cmd::Search {
                        query,
                        scope,
                        limit,
                    } => match conv.search_debug(&query, scope, limit) {
                        Ok(results) => {
                            if results.items.is_empty() {
                                println!("  (no results)");
                            } else {
                                for (i, item) in results.items.iter().enumerate() {
                                    println!(
                                        "  [{}] score={:.4} seg={} node={}",
                                        i + 1,
                                        item.score,
                                        item.segment_id,
                                        item.node_id
                                    );
                                    println!("      {}", item.data);
                                    if let Some(ref meta) = item.metadata {
                                        println!("      meta: {meta}");
                                    }
                                }
                                if results.has_more {
                                    println!("  ... more results available");
                                }
                                if results.skipped_mismatched > 0 {
                                    println!(
                                        "  ({} segments skipped: model mismatch)",
                                        results.skipped_mismatched
                                    );
                                }
                            }
                        }
                        Err(e) => println!("  error: {e}"),
                    },
                    Cmd::Get { node_id } => match m.get(node_id) {
                        Ok(content) => {
                            println!("  data: {}", content.data);
                            if let Some(ref meta) = content.metadata {
                                println!("  meta: {meta}");
                            }
                        }
                        Err(e) => println!("  error: {e}"),
                    },
                    Cmd::Stats => match m.segment_stats() {
                        Ok(s) => println!(
                            "  project={} segments={} mismatched={} nodes={}",
                            project_id, s.total_segments, s.mismatched_segments, s.total_nodes
                        ),
                        Err(e) => println!("  error: {e}"),
                    },
                    Cmd::Conversations => match m.list_conversations() {
                        Ok(list) => {
                            println!("  project: {project_id}");
                            for c in &list {
                                let marker = if c.uuid == conv.uuid { " *" } else { "" };
                                println!(
                                    "  {}  {:<12}  segs={}  nodes={}{marker}",
                                    &c.uuid[..8],
                                    format_timestamp(c.created_at),
                                    c.segment_count,
                                    c.node_count
                                );
                            }
                        }
                        Err(e) => println!("  error: {e}"),
                    },
                    Cmd::Switch { prefix } => {
                        match resolve_conversation_prefix(&m, &prefix) {
                            PrefixMatch::Exact(full_uuid) => {
                                drop(conv);
                                match m.conversation(&full_uuid) {
                                    Ok(c) => {
                                        println!("  switched to {}", &c.uuid[..8]);
                                        conv = c;
                                    }
                                    Err(e) => {
                                        println!("  error: {e}");
                                        conv = m.create_conversation().unwrap();
                                        println!("  created new: {}", &conv.uuid[..8]);
                                    }
                                }
                            }
                            PrefixMatch::Ambiguous(matches) => {
                                println!("  ambiguous prefix, {} matches:", matches.len());
                                for c in &matches {
                                    println!(
                                        "    {}  created={}  segs={}  nodes={}",
                                        c.uuid,
                                        format_timestamp(c.created_at),
                                        c.segment_count,
                                        c.node_count
                                    );
                                }
                            }
                            PrefixMatch::None => {
                                println!("  no conversation matching '{prefix}'");
                            }
                        }
                    }
                    Cmd::NewConv => {
                        drop(conv);
                        conv = m.create_conversation().unwrap();
                        println!("  new conversation: {}", &conv.uuid[..8]);
                    }
                    Cmd::Delete { prefix } => {
                        match resolve_conversation_prefix(&m, &prefix) {
                            PrefixMatch::Exact(full_uuid) => {
                                if full_uuid == conv.uuid {
                                    println!(
                                        "  cannot delete active conversation (switch first)"
                                    );
                                } else {
                                    match m.delete_conversation(&full_uuid) {
                                        Ok(()) => println!("  deleted {}", &full_uuid[..8]),
                                        Err(e) => println!("  error: {e}"),
                                    }
                                }
                            }
                            PrefixMatch::Ambiguous(matches) => {
                                println!("  ambiguous prefix, {} matches:", matches.len());
                                for c in &matches {
                                    println!(
                                        "    {}  created={}  segs={}  nodes={}",
                                        c.uuid,
                                        format_timestamp(c.created_at),
                                        c.segment_count,
                                        c.node_count
                                    );
                                }
                            }
                            PrefixMatch::None => {
                                println!("  no conversation matching '{prefix}'");
                            }
                        }
                    }
                    Cmd::Purge => match m.list_conversations() {
                        Ok(list) => {
                            let mut deleted = 0;
                            for c in &list {
                                if c.uuid != conv.uuid {
                                    if let Ok(()) = m.delete_conversation(&c.uuid) {
                                        deleted += 1;
                                    }
                                }
                            }
                            println!("  purged {deleted} conversations");
                        }
                        Err(e) => println!("  error: {e}"),
                    },
                    Cmd::SwitchProject { name } => {
                        drop(conv);
                        drop(m);
                        project_id = name;
                        m = init_project(&project_id, &mut log_guard);
                    
                        conv = m.create_conversation().unwrap();
                        println!("  conversation: {}", &conv.uuid[..8]);
                    }
                    Cmd::ListProjects => {
                        let projects = list_projects_full();
                        if projects.is_empty() {
                            println!("  (no projects)");
                        } else {
                            for p in &projects {
                                let marker = if p.name == project_id { " *" } else { "" };
                                println!(
                                    "  {:<20}  convs={}  nodes={}{marker}",
                                    p.name, p.conversations, p.nodes
                                );
                            }
                        }
                    }
                    Cmd::DeleteProject { name } => {
                        if name == project_id {
                            println!("  cannot delete active project (switch first)");
                        } else {
                            let projects = list_projects_full();
                            match projects.iter().find(|p| p.name == name) {
                                Some(p) => {
                                    let dir = moerae_base().join("projects").join(&p.hash_dir);
                                    match std::fs::remove_dir_all(&dir) {
                                        Ok(()) => println!("  deleted project '{name}'"),
                                        Err(e) => println!("  error: {e}"),
                                    }
                                }
                                None => println!("  no project named '{name}'"),
                            }
                        }
                    }
                    Cmd::Verbose => {
                        log_guard.toggle();
                        let state = if log_guard.is_verbose() {
                            "on"
                        } else {
                            "off"
                        };
                        println!("  verbose: {state}");
                    }
                    Cmd::Help => print_help(),
                    Cmd::Quit => break,
                    Cmd::Unknown(s) => println!("  unknown command: {s}. Type /help"),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                println!("error: {e}");
                break;
            }
        }
    }

    let _ = rl.save_history(&hist_path);
    println!("bye");
}

fn init_project(project_id: &str, log_guard: &mut StderrGuard) -> Moerae {
    println!("Loading project '{project_id}'...");
    match Moerae::init(project_id) {
        Ok(m) => {
            let stats = m.segment_stats().unwrap();
            println!(
                "  ready. {} segments, {} nodes.",
                stats.total_segments, stats.total_nodes
            );
            m
        }
        Err(e) => {
            log_guard.restore();
            eprintln!("Failed to init: {e}");
            eprintln!(
                "Make sure the model is at: ~/.moerae/models/embeddinggemma-300m-qat-q8_0.gguf"
            );
            std::process::exit(1);
        }
    }
}

enum Cmd {
    Put {
        data: String,
        metadata: Option<String>,
        persist: bool,
    },
    Search {
        query: String,
        scope: Option<Scope>,
        limit: Option<usize>,
    },
    Get {
        node_id: i64,
    },
    Stats,
    Conversations,
    Switch {
        prefix: String,
    },
    NewConv,
    Delete {
        prefix: String,
    },
    Purge,
    SwitchProject {
        name: String,
    },
    ListProjects,
    DeleteProject { name: String },
    Verbose,
    Help,
    Quit,
    Unknown(String),
}

fn parse_command(line: &str) -> Cmd {
    if let Some(rest) = line.strip_prefix("/put ") {
        let persist = rest.starts_with("--persist ");
        let data = if persist { &rest[10..] } else { rest };

        let (data, metadata) = if let Some((d, m)) = data.split_once("|||") {
            (d.trim().to_string(), Some(m.trim().to_string()))
        } else {
            (data.to_string(), None)
        };

        Cmd::Put {
            data,
            metadata,
            persist,
        }
    } else if let Some(rest) = line.strip_prefix("/search ") {
        let (scope, rest) = if let Some(r) = rest.strip_prefix("--project ") {
            (Some(Scope::Project), r)
        } else {
            (None, rest)
        };
        let (limit, query) = if let Some(r) = rest.strip_prefix("--limit ") {
            if let Some((n, q)) = r.split_once(' ') {
                (n.parse().ok(), q.to_string())
            } else {
                (None, r.to_string())
            }
        } else {
            (None, rest.to_string())
        };
        Cmd::Search {
            query,
            scope,
            limit,
        }
    } else if let Some(rest) = line.strip_prefix("/get ") {
        match rest.trim().parse::<i64>() {
            Ok(id) => Cmd::Get { node_id: id },
            Err(_) => Cmd::Unknown("invalid node_id".into()),
        }
    } else if line == "/stats" {
        Cmd::Stats
    } else if line == "/conversations" || line == "/convs" {
        Cmd::Conversations
    } else if let Some(rest) = line.strip_prefix("/switch ") {
        Cmd::Switch {
            prefix: rest.trim().to_string(),
        }
    } else if line == "/new" {
        Cmd::NewConv
    } else if let Some(rest) = line.strip_prefix("/delete ") {
        Cmd::Delete {
            prefix: rest.trim().to_string(),
        }
    } else if line == "/purge" {
        Cmd::Purge
    } else if let Some(rest) = line.strip_prefix("/project ") {
        Cmd::SwitchProject {
            name: rest.trim().to_string(),
        }
    } else if line == "/projects" {
        Cmd::ListProjects
    } else if let Some(rest) = line.strip_prefix("/delete_project ") {
        Cmd::DeleteProject {
            name: rest.trim().to_string(),
        }
    } else if line == "/verbose" || line == "/v" {
        Cmd::Verbose
    } else if line == "/help" || line == "/h" {
        Cmd::Help
    } else if line == "/quit" || line == "/exit" || line == "/q" {
        Cmd::Quit
    } else {
        Cmd::Search {
            query: line.to_string(),
            scope: None,
            limit: None,
        }
    }
}

enum PrefixMatch {
    Exact(String),
    Ambiguous(Vec<moerae::ConversationInfo>),
    None,
}

fn resolve_conversation_prefix(m: &Moerae, prefix: &str) -> PrefixMatch {
    let list = match m.list_conversations() {
        Ok(l) => l,
        Err(_) => return PrefixMatch::None,
    };

    let matches: Vec<moerae::ConversationInfo> = list
        .into_iter()
        .filter(|c| c.uuid.starts_with(prefix))
        .collect();

    match matches.len() {
        0 => PrefixMatch::None,
        1 => PrefixMatch::Exact(matches.into_iter().next().unwrap().uuid),
        _ => PrefixMatch::Ambiguous(matches),
    }
}

fn format_timestamp(ts: i64) -> String {
    let mins = (ts / 60) % 60;
    let hours = (ts / 3600) % 24;
    let days = ts / 86400;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let ago = now - ts;

    if ago < 60 {
        "just now".to_string()
    } else if ago < 3600 {
        format!("{}m ago", ago / 60)
    } else if ago < 86400 {
        format!("{}h {}m ago", ago / 3600, (ago % 3600) / 60)
    } else if ago < 86400 * 7 {
        format!("{}d ago", ago / 86400)
    } else {
        let (y, m, d) = days_to_ymd(days);
        format!("{y:04}-{m:02}-{d:02} {hours:02}:{mins:02}")
    }
}

fn days_to_ymd(mut days: i64) -> (i64, i64, i64) {
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn print_help() {
    println!("Commands:");
    println!("  /put <text>                    Store content");
    println!("  /put --persist <text>          Store persistent content");
    println!("  /put <text> ||| <metadata>     Store with metadata summary");
    println!("  /search <query>                Search (or just type the query)");
    println!("  /search --project <query>      Search promoted segments across project");
    println!("  /search --limit N <query>      Search with result limit");
    println!("  /get <node_id>                 Fetch full content by node ID");
    println!("  /stats                         Show project statistics");
    println!("  /convs                         List conversations");
    println!("  /new                           Start new conversation");
    println!("  /switch <uuid-prefix>          Switch to existing conversation");
    println!("  /delete <uuid-prefix>          Delete a conversation");
    println!("  /purge                         Delete all conversations except active");
    println!("  /project <name>                Switch to a different project");
    println!("  /projects                      List all projects");
    println!("  /delete_project <name>         Delete a project");
    println!("  /verbose                       Toggle llama.cpp logs (off by default)");
    println!("  /help                          Show this help");
    println!("  /quit                          Exit (Ctrl-D also works)");
    println!();
    println!("  Typing without a / prefix searches directly.");
    println!("  Up/Down arrows recall history. Tab completes commands.");
    println!();
}
