use std::io::{self, BufRead};
use std::process;

use moerae::types::Scope;
use moerae::Moerae;

fn main() {
    // Silence llama.cpp logs on stderr
    let saved_stderr = unsafe { libc::dup(libc::STDERR_FILENO) };
    let devnull = std::fs::File::open("/dev/null").unwrap();
    unsafe {
        libc::dup2(std::os::fd::AsRawFd::as_raw_fd(&devnull), libc::STDERR_FILENO);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_usage();
        process::exit(0);
    }

    let cmd = args[0].as_str();

    match cmd {
        "put" => cmd_put(&args[1..]),
        "search" => cmd_search(&args[1..]),
        "get" => cmd_get(&args[1..]),
        "forget" => cmd_forget(&args[1..]),
        "stats" => cmd_stats(&args[1..]),
        "convs" => cmd_convs(&args[1..]),
        "projects" => cmd_projects(),
        "completions" => cmd_completions(&args[1..]),
        _ => {
            eprintln_restore(saved_stderr, &format!("unknown command: {cmd}"));
            print_usage();
            process::exit(1);
        }
    }
}

fn init(args: &[String]) -> (Moerae, String, Vec<String>) {
    let mut project = "default".to_string();
    let mut rest = vec![];
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--project" => {
                i += 1;
                if i < args.len() {
                    project = args[i].clone();
                }
            }
            _ => rest.push(args[i].clone()),
        }
        i += 1;
    }

    let m = match Moerae::init(&project) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    (m, project, rest)
}

fn cmd_put(args: &[String]) {
    let (m, _, rest) = init(args);

    let mut conv_id = None;
    let mut persist = false;
    let mut metadata: Option<String> = None;
    let mut data_parts = vec![];
    let mut i = 0;

    while i < rest.len() {
        match rest[i].as_str() {
            "-c" | "--conversation" => {
                i += 1;
                if i < rest.len() {
                    conv_id = Some(rest[i].clone());
                }
            }
            "--persist" => persist = true,
            "-m" | "--metadata" => {
                i += 1;
                if i < rest.len() {
                    metadata = Some(rest[i].clone());
                }
            }
            "--stdin" => {
                let stdin = io::stdin();
                let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
                data_parts.push(lines.join("\n"));
            }
            _ => data_parts.push(rest[i].clone()),
        }
        i += 1;
    }

    let data = data_parts.join(" ");
    if data.trim().is_empty() {
        eprintln!("error: no data provided");
        process::exit(1);
    }

    let mut conv = match conv_id {
        Some(ref id) => match m.conversation(id) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        None => m.create_conversation().unwrap(),
    };

    match conv.put(&data, metadata.as_deref(), persist) {
        Ok(()) => {
            if conv_id.is_none() {
                println!("{}", conv.uuid);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_search(args: &[String]) {
    let (m, _, rest) = init(args);

    let mut conv_id = None;
    let mut scope = None;
    let mut limit = None;
    let mut json_output = false;
    let mut query_parts = vec![];
    let mut i = 0;

    while i < rest.len() {
        match rest[i].as_str() {
            "-c" | "--conversation" => {
                i += 1;
                if i < rest.len() {
                    conv_id = Some(rest[i].clone());
                }
            }
            "--project-scope" => scope = Some(Scope::Project),
            "-n" | "--limit" => {
                i += 1;
                if i < rest.len() {
                    limit = rest[i].parse().ok();
                }
            }
            "--json" => json_output = true,
            _ => query_parts.push(rest[i].clone()),
        }
        i += 1;
    }

    let query = query_parts.join(" ");
    if query.trim().is_empty() {
        eprintln!("error: no query provided");
        process::exit(1);
    }

    let conv = match conv_id {
        Some(ref id) => match m.conversation(id) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        None => m.create_conversation().unwrap(),
    };

    match conv.search_debug(&query, scope, limit) {
        Ok(results) => {
            if json_output {
                print_json(&results);
            } else {
                for item in &results.items {
                    println!(
                        "{:.4}\t{}\t{}",
                        item.score, item.node_id, item.data
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_get(args: &[String]) {
    let (m, _, rest) = init(args);

    if rest.is_empty() {
        eprintln!("error: node_id required");
        process::exit(1);
    }

    let node_id: i64 = match rest[0].parse() {
        Ok(id) => id,
        Err(_) => {
            eprintln!("error: invalid node_id '{}'", rest[0]);
            process::exit(1);
        }
    };

    match m.get(node_id) {
        Ok(content) => {
            println!("{}", content.data);
            if let Some(meta) = content.metadata {
                eprintln!("metadata: {meta}");
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_forget(args: &[String]) {
    let (m, _, rest) = init(args);

    let mut conv_id: Option<String> = None;
    let mut node_ids: Vec<i64> = vec![];
    let mut query_parts: Vec<String> = vec![];
    let mut scope = None;
    let mut limit: Option<usize> = Some(50);
    let mut min_score: f32 = 0.90;
    let mut apply = false;
    let mut json_output = false;
    let mut i = 0;

    while i < rest.len() {
        match rest[i].as_str() {
            "-c" | "--conversation" => {
                i += 1;
                if i < rest.len() {
                    conv_id = Some(rest[i].clone());
                }
            }
            "--node" => {
                i += 1;
                if i < rest.len() {
                    match rest[i].parse::<i64>() {
                        Ok(id) => node_ids.push(id),
                        Err(_) => {
                            eprintln!("error: invalid node id '{}'", rest[i]);
                            process::exit(1);
                        }
                    }
                }
            }
            "--query" => {
                i += 1;
                if i < rest.len() {
                    query_parts.push(rest[i].clone());
                }
            }
            "--project-scope" => scope = Some(Scope::Project),
            "-n" | "--limit" => {
                i += 1;
                if i < rest.len() {
                    limit = rest[i].parse().ok();
                }
            }
            "--min-score" => {
                i += 1;
                if i < rest.len() {
                    match rest[i].parse::<f32>() {
                        Ok(s) => min_score = s,
                        Err(_) => {
                            eprintln!("error: invalid --min-score '{}'", rest[i]);
                            process::exit(1);
                        }
                    }
                }
            }
            "--yes" => apply = true,
            "--json" => json_output = true,
            _ => query_parts.push(rest[i].clone()),
        }
        i += 1;
    }

    let query = query_parts.join(" ");
    let has_query = !query.trim().is_empty();

    if node_ids.is_empty() && !has_query {
        eprintln!("error: nothing to forget; use --node <id> or --query <text>");
        process::exit(1);
    }
    if !node_ids.is_empty() && has_query {
        eprintln!("error: use --node or --query, not both");
        process::exit(1);
    }
    // A query without a scope would silently match nothing: searching without -c
    // creates a fresh empty conversation. Silence is unacceptable for a destructive
    // command, so refuse rather than report "no nodes matched".
    if has_query && conv_id.is_none() && scope.is_none() {
        eprintln!("error: --query requires -c <uuid> or --project-scope");
        process::exit(1);
    }

    let plan = if has_query {
        m.plan_forget_matching(conv_id.as_deref(), &query, scope, limit, min_score)
    } else {
        m.plan_forget_nodes(&node_ids)
    };

    let plan = match plan {
        Ok(p) => p,
        Err(moerae::ForgetError::NoMatch) => {
            // Not an error: forget must stay idempotent for scripts.
            println!("no nodes matched");
            return;
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    if json_output {
        print_forget_json(&plan);
    } else {
        for t in &plan.targets {
            let score = match t.score {
                Some(s) => format!("{s:.4}"),
                None => "-".to_string(),
            };
            println!("{}\t{}\t{}\t{}", score, t.segment_id, t.node_id, t.data);
        }
        for imp in &plan.impacts {
            let after = imp.node_count_before - imp.nodes_removed;
            if after <= 0 {
                println!(
                    "segment {}: {} -> 0 nodes (segment dropped)",
                    imp.segment_id, imp.node_count_before
                );
            } else {
                println!(
                    "segment {}: {} -> {} nodes ({} removed)",
                    imp.segment_id, imp.node_count_before, after, imp.nodes_removed
                );
            }
        }
        if plan.has_more {
            println!("note: more candidates exist above the limit; raise -n to see them");
        }
        if plan.skipped_mismatched > 0 {
            println!(
                "note: {} segment(s) skipped (embedding model changed); \
                 target them with --node or rebuild first",
                plan.skipped_mismatched
            );
        }
    }

    if !apply {
        if !json_output {
            println!(
                "{} node(s) in {} segment(s) would be forgotten. Re-run with --yes to apply.",
                plan.targets.len(),
                plan.impacts.len()
            );
        }
        return;
    }

    match m.apply_forget(&plan) {
        Ok(outcome) => {
            if !json_output {
                println!(
                    "forgot {} node(s); {} segment(s) rewritten, {} dropped",
                    outcome.nodes_removed, outcome.segments_rewritten, outcome.segments_dropped
                );
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_stats(args: &[String]) {
    let (m, project, _) = init(args);

    match m.segment_stats() {
        Ok(s) => {
            println!("project: {project}");
            println!("segments: {}", s.total_segments);
            println!("mismatched: {}", s.mismatched_segments);
            println!("nodes: {}", s.total_nodes);
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_convs(args: &[String]) {
    let (m, project, _) = init(args);

    match m.list_conversations() {
        Ok(list) => {
            println!("project: {project}");
            for c in &list {
                println!(
                    "{}\tsegs={}\tnodes={}",
                    c.uuid, c.segment_count, c.node_count
                );
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_projects() {
    let base = if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home).join(".moerae").join("projects")
    } else {
        std::path::PathBuf::from(".moerae").join("projects")
    };

    let entries = match std::fs::read_dir(&base) {
        Ok(e) => e,
        Err(_) => {
            println!("(no projects)");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let db_path = path.join("moerae.db");
        if !db_path.exists() {
            continue;
        }

        if let Ok(conn) = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
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
                .unwrap_or_else(|| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .to_string()
                });

            let nodes: i64 = conn
                .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
                .unwrap_or(0);
            let convs: i64 = conn
                .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
                .unwrap_or(0);

            println!("{name}\tconvs={convs}\tnodes={nodes}");
        }
    }
}

fn cmd_completions(args: &[String]) {
    let shell = args.first().map(|s| s.as_str()).unwrap_or_else(|| {
        eprintln!("usage: moerae completions <bash|zsh|fish>");
        process::exit(1);
    });

    match shell {
        "bash" => print!("{}", BASH_COMPLETIONS),
        "zsh" => print!("{}", ZSH_COMPLETIONS),
        "fish" => print!("{}", FISH_COMPLETIONS),
        _ => {
            eprintln!("unknown shell: {shell}. Use bash, zsh, or fish.");
            process::exit(1);
        }
    }
}

const BASH_COMPLETIONS: &str = r#"_moerae() {
    local cur prev cmds opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmds="put search get forget stats convs projects completions"

    if [[ ${COMP_CWORD} -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "${cmds}" -- "${cur}") )
        return 0
    fi

    case "${prev}" in
        -p|--project)
            local projects=$(moerae projects 2>/dev/null | cut -f1)
            COMPREPLY=( $(compgen -W "${projects}" -- "${cur}") )
            return 0
            ;;
        completions)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- "${cur}") )
            return 0
            ;;
    esac

    case "${COMP_WORDS[1]}" in
        put)
            opts="-p --project -c --conversation --persist -m --metadata --stdin"
            ;;
        search)
            opts="-p --project -c --conversation --project-scope -n --limit --json"
            ;;
        get)
            opts="-p --project"
            ;;
        forget)
            opts="-p --project -c --conversation --node --query --min-score -n --limit --project-scope --yes --json"
            ;;
        stats|convs)
            opts="-p --project"
            ;;
    esac

    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
    return 0
}
complete -F _moerae moerae
"#;

const ZSH_COMPLETIONS: &str = r#"#compdef moerae

_moerae() {
    local -a commands
    commands=(
        'put:Store content'
        'search:Search for content'
        'get:Fetch content by node ID'
        'forget:Remove outdated content'
        'stats:Show project statistics'
        'convs:List conversations'
        'projects:List all projects'
        'completions:Generate shell completions'
    )

    _arguments -C \
        '1:command:->command' \
        '*::arg:->args'

    case $state in
        command)
            _describe 'command' commands
            ;;
        args)
            case ${words[1]} in
                put)
                    _arguments \
                        '(-p --project)'{-p,--project}'[Project name]:project:->projects' \
                        '(-c --conversation)'{-c,--conversation}'[Conversation UUID]:uuid:' \
                        '--persist[Store as persistent]' \
                        '(-m --metadata)'{-m,--metadata}'[Metadata summary]:metadata:' \
                        '--stdin[Read from stdin]' \
                        '*:data:'
                    ;;
                search)
                    _arguments \
                        '(-p --project)'{-p,--project}'[Project name]:project:->projects' \
                        '(-c --conversation)'{-c,--conversation}'[Conversation UUID]:uuid:' \
                        '--project-scope[Search project scope]' \
                        '(-n --limit)'{-n,--limit}'[Max results]:limit:' \
                        '--json[JSON output]' \
                        '*:query:'
                    ;;
                get)
                    _arguments \
                        '(-p --project)'{-p,--project}'[Project name]:project:->projects' \
                        ':node_id:'
                    ;;
                forget)
                    _arguments \
                        '(-p --project)'{-p,--project}'[Project name]:project:->projects' \
                        '(-c --conversation)'{-c,--conversation}'[Conversation UUID]:uuid:' \
                        '*--node[Forget a specific node]:node_id:' \
                        '--query[Forget nodes matching this text]:query:' \
                        '--min-score[Similarity floor (default 0.90)]:score:' \
                        '(-n --limit)'{-n,--limit}'[Max candidates]:limit:' \
                        '--project-scope[Match project scope]' \
                        '--yes[Apply instead of previewing]' \
                        '--json[JSON output]'
                    ;;
                stats|convs)
                    _arguments \
                        '(-p --project)'{-p,--project}'[Project name]:project:->projects'
                    ;;
                completions)
                    _arguments ':shell:(bash zsh fish)'
                    ;;
            esac

            case $state in
                projects)
                    local -a projects
                    projects=(${(f)"$(moerae projects 2>/dev/null | cut -f1)"})
                    _describe 'project' projects
                    ;;
            esac
            ;;
    esac
}

_moerae "$@"
"#;

const FISH_COMPLETIONS: &str = r#"# Commands
complete -c moerae -n "__fish_use_subcommand" -a put -d "Store content"
complete -c moerae -n "__fish_use_subcommand" -a search -d "Search for content"
complete -c moerae -n "__fish_use_subcommand" -a get -d "Fetch content by node ID"
complete -c moerae -n "__fish_use_subcommand" -a forget -d "Remove outdated content"
complete -c moerae -n "__fish_use_subcommand" -a stats -d "Show project statistics"
complete -c moerae -n "__fish_use_subcommand" -a convs -d "List conversations"
complete -c moerae -n "__fish_use_subcommand" -a projects -d "List all projects"
complete -c moerae -n "__fish_use_subcommand" -a completions -d "Generate shell completions"

# Global options
complete -c moerae -s p -l project -d "Project name" -xa "(moerae projects 2>/dev/null | string split \t -f1)"

# put options
complete -c moerae -n "__fish_seen_subcommand_from put" -s c -l conversation -d "Conversation UUID"
complete -c moerae -n "__fish_seen_subcommand_from put" -l persist -d "Store as persistent"
complete -c moerae -n "__fish_seen_subcommand_from put" -s m -l metadata -d "Metadata summary"
complete -c moerae -n "__fish_seen_subcommand_from put" -l stdin -d "Read from stdin"

# search options
complete -c moerae -n "__fish_seen_subcommand_from search" -s c -l conversation -d "Conversation UUID"
complete -c moerae -n "__fish_seen_subcommand_from search" -l project-scope -d "Search project scope"
complete -c moerae -n "__fish_seen_subcommand_from search" -s n -l limit -d "Max results"
complete -c moerae -n "__fish_seen_subcommand_from search" -l json -d "JSON output"

# forget options
complete -c moerae -n "__fish_seen_subcommand_from forget" -s c -l conversation -d "Conversation UUID"
complete -c moerae -n "__fish_seen_subcommand_from forget" -l node -d "Forget a specific node"
complete -c moerae -n "__fish_seen_subcommand_from forget" -l query -d "Forget nodes matching this text"
complete -c moerae -n "__fish_seen_subcommand_from forget" -l min-score -d "Similarity floor (default 0.90)"
complete -c moerae -n "__fish_seen_subcommand_from forget" -s n -l limit -d "Max candidates"
complete -c moerae -n "__fish_seen_subcommand_from forget" -l project-scope -d "Match project scope"
complete -c moerae -n "__fish_seen_subcommand_from forget" -l yes -d "Apply instead of previewing"
complete -c moerae -n "__fish_seen_subcommand_from forget" -l json -d "JSON output"

# completions options
complete -c moerae -n "__fish_seen_subcommand_from completions" -a "bash zsh fish"
"#;

fn print_json(results: &moerae::SearchResultsDebug) {
    print!("[");
    for (i, item) in results.items.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        let meta = match &item.metadata {
            Some(m) => format!("\"{}\"", m.replace('\\', "\\\\").replace('"', "\\\"")),
            None => "null".to_string(),
        };
        let data = item.data.replace('\\', "\\\\").replace('"', "\\\"");
        print!(
            "{{\"node_id\":{},\"score\":{:.4},\"data\":\"{}\",\"metadata\":{}}}",
            item.node_id, item.score, data, meta
        );
    }
    println!("]");
}

fn print_forget_json(plan: &moerae::ForgetPlan) {
    print!("{{\"targets\":[");
    for (i, t) in plan.targets.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        let data = t.data.replace('\\', "\\\\").replace('"', "\\\"");
        let score = match t.score {
            Some(s) => format!("{s:.4}"),
            None => "null".to_string(),
        };
        print!(
            "{{\"node_id\":{},\"segment_id\":{},\"score\":{},\"data\":\"{}\"}}",
            t.node_id, t.segment_id, score, data
        );
    }
    print!("],\"segments\":[");
    for (i, imp) in plan.impacts.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        print!(
            "{{\"segment_id\":{},\"node_count_before\":{},\"nodes_removed\":{}}}",
            imp.segment_id, imp.node_count_before, imp.nodes_removed
        );
    }
    println!(
        "],\"has_more\":{},\"skipped_mismatched\":{}}}",
        plan.has_more, plan.skipped_mismatched
    );
}

fn eprintln_restore(saved_fd: i32, msg: &str) {
    unsafe {
        libc::dup2(saved_fd, libc::STDERR_FILENO);
    }
    eprintln!("{msg}");
}

fn print_usage() {
    println!("moerae - AI memory system

Usage: moerae <command> [options]

Commands:
  put <text>              Store content
  search <query>          Search for content
  get <node_id>           Fetch full content by node ID
  forget                  Remove outdated content (dry-run unless --yes)
  stats                   Show project statistics
  convs                   List conversations
  projects                List all projects
  completions <shell>     Generate shell completions (bash, zsh, fish)

Global options:
  -p, --project <name>    Project name (default: 'default')

Put options:
  -c, --conversation <id> Use existing conversation
  --persist               Store as persistent (never evicted)
  -m, --metadata <text>   Metadata summary for large content
  --stdin                 Read data from stdin

Search options:
  -c, --conversation <id> Search within a conversation
  --project-scope         Search promoted segments across project
  -n, --limit <N>         Max results (default: 10)
  --json                  Output as JSON

Forget options:
  -c, --conversation <id> Conversation to forget from (required with --query)
  --node <id>             Forget a specific node (repeatable)
  --query <text>          Forget nodes semantically matching this text
  --min-score <f>         Similarity floor for --query (default: 0.90)
  -n, --limit <N>         Max candidates to consider (default: 50)
  --project-scope         Match against promoted segments across the project
  --yes                   Apply the plan (without it, forget only previews)
  --json                  Output the plan as JSON

Examples:
  moerae put -p myproject \"The capital of France is Paris\"
  moerae search -p myproject \"capital of France\"
  moerae put -p myproject --persist \"important config value\"
  moerae search -p myproject --project-scope \"config\"
  cat document.txt | moerae put -p myproject --stdin -m \"document summary\"
  moerae get -p myproject 42
  moerae forget -p myproject --node 42
  moerae forget -p myproject -c <uuid> --query \"old rate limit\"
  moerae forget -p myproject -c <uuid> --query \"old rate limit\" --yes
  moerae stats -p myproject
  moerae projects
  moerae completions bash >> ~/.bashrc
  moerae completions zsh > ~/.zfunc/_moerae
  moerae completions fish > ~/.config/fish/completions/moerae.fish");
}
