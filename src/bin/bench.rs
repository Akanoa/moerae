use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use moerae::embedding::model::EmbeddingModel;
use moerae::{Config, Moerae};

// --- BEAM data structures (deserialize only) ---

#[derive(Deserialize)]
struct Batch {
    batch_number: Option<i64>,
    turns: Vec<Vec<Turn>>,
    #[allow(dead_code)]
    time_anchor: Option<String>,
}

#[derive(Deserialize)]
struct Turn {
    role: String,
    content: String,
    #[allow(dead_code)]
    id: Option<i64>,
    #[allow(dead_code)]
    question_type: Option<String>,
    #[allow(dead_code)]
    time_anchor: Option<String>,
    #[allow(dead_code)]
    index: Option<String>,
}

// Probing questions: HashMap<String, Vec<serde_json::Value>>
// Each value has at least a "question" field; other fields vary by type.
// We preserve all original fields in output.

// --- Output structures ---

#[derive(Serialize)]
struct BenchOutput {
    scale: String,
    chat_index: usize,
    plan: Option<usize>,
    config: BenchConfig,
    stats: BenchStats,
    questions: HashMap<String, Vec<OutputQuestion>>,
}

#[derive(Serialize)]
struct BenchConfig {
    max_indexable_tokens: usize,
    segment_capacity: usize,
    segment_staleness: u64,
    max_segments: usize,
    max_cached_indexes: usize,
    search_limit: usize,
}

#[derive(Serialize)]
struct BenchStats {
    turns_ingested: usize,
    segments: usize,
    total_nodes: usize,
    ingest_secs: f64,
    search_secs: f64,
}

#[derive(Serialize)]
struct OutputQuestion {
    /// All original fields from BEAM's probing question, preserved as-is.
    #[serde(flatten)]
    original: serde_json::Value,
    retrieved_context: Vec<RetrievedItem>,
    search_ms: u64,
}

#[derive(Serialize)]
struct RetrievedItem {
    rank: usize,
    score: f32,
    node_id: i64,
    data: String,
    metadata: Option<String>,
}

// --- CLI ---

struct Args {
    dataset: PathBuf,
    scale: String,
    output: PathBuf,
    chat: Option<usize>,
    plan: Option<usize>,
    search_limit: usize,
    segment_capacity: usize,
    max_indexable_tokens: usize,
    max_segments: usize,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        process::exit(0);
    }

    let mut dataset = None;
    let mut scale = None;
    let mut output = PathBuf::from("bench/output");
    let mut chat = None;
    let mut plan = None;
    let mut search_limit = 50;
    let mut segment_capacity = 500;
    let mut max_indexable_tokens = 512;
    let mut max_segments = 5000;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--dataset" => {
                i += 1;
                dataset = Some(PathBuf::from(&args[i]));
            }
            "--scale" => {
                i += 1;
                scale = Some(args[i].clone());
            }
            "--output" => {
                i += 1;
                output = PathBuf::from(&args[i]);
            }
            "--chat" => {
                i += 1;
                chat = Some(args[i].parse::<usize>().unwrap_or_else(|_| {
                    eprintln!("error: --chat requires a number");
                    process::exit(1);
                }));
            }
            "--plan" => {
                i += 1;
                plan = Some(args[i].parse::<usize>().unwrap_or_else(|_| {
                    eprintln!("error: --plan requires a number");
                    process::exit(1);
                }));
            }
            "--search-limit" => {
                i += 1;
                search_limit = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --search-limit requires a number");
                    process::exit(1);
                });
            }
            "--segment-capacity" => {
                i += 1;
                segment_capacity = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --segment-capacity requires a number");
                    process::exit(1);
                });
            }
            "--max-indexable-tokens" => {
                i += 1;
                max_indexable_tokens = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --max-indexable-tokens requires a number");
                    process::exit(1);
                });
            }
            "--max-segments" => {
                i += 1;
                max_segments = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --max-segments requires a number");
                    process::exit(1);
                });
            }
            other => {
                eprintln!("error: unknown argument: {other}");
                print_usage();
                process::exit(1);
            }
        }
        i += 1;
    }

    let dataset = dataset.unwrap_or_else(|| {
        eprintln!("error: --dataset is required");
        process::exit(1);
    });
    let scale = scale.unwrap_or_else(|| {
        eprintln!("error: --scale is required");
        process::exit(1);
    });

    Args {
        dataset,
        scale,
        output,
        chat,
        plan,
        search_limit,
        segment_capacity,
        max_indexable_tokens,
        max_segments,
    }
}

fn print_usage() {
    eprintln!(
        "moerae-bench: BEAM benchmark harness for Moerae

USAGE:
    moerae-bench --dataset <path> --scale <100K|500K|1M|10M> [OPTIONS]

REQUIRED:
    --dataset <path>              Path to BEAM chats/ directory
    --scale <100K|500K|1M|10M>    Conversation scale to benchmark

OPTIONS:
    --output <path>               Output directory (default: bench/output)
    --chat <N>                    Run single chat index only
    --plan <N>                    For 10M scale, run single plan index only
    --search-limit <N>            Top-k results per question (default: 50)
    --segment-capacity <N>        Nodes per segment (default: 500)
    --max-indexable-tokens <N>    Token threshold for metadata (default: 512)
    --max-segments <N>            Max closed segments before eviction (default: 5000)
    -h, --help                    Show this help"
    );
}

// --- Core logic ---

fn make_config(args: &Args) -> Config {
    Config {
        max_indexable_tokens: args.max_indexable_tokens,
        segment_capacity: args.segment_capacity,
        segment_staleness: 999_999_999,
        max_segments: args.max_segments,
        max_persist_segments: 100,
        max_cached_indexes: 100,
        base_score: 0.3,
        hit_boost: 0.1,
        promotion_threshold: 0.7,
        default_search_limit: args.search_limit,
    }
}

/// Extract user-assistant pairs from BEAM chat data.
fn extract_pairs(batches: &[Batch]) -> Vec<(Option<i64>, String, String)> {
    let mut pairs = Vec::new();

    for batch in batches {
        let batch_num = batch.batch_number;
        for turn in &batch.turns {
            // Each turn is a vec of messages; extract consecutive user-assistant pairs
            let mut j = 0;
            while j + 1 < turn.len() {
                if turn[j].role == "user" && turn[j + 1].role == "assistant" {
                    pairs.push((
                        batch_num,
                        turn[j].content.clone(),
                        turn[j + 1].content.clone(),
                    ));
                    j += 2;
                } else {
                    j += 1;
                }
            }
        }
    }

    pairs
}

/// Build the data string and optional metadata for a conversation turn.
fn format_turn(
    batch_num: Option<i64>,
    user: &str,
    assistant: &str,
    model: &EmbeddingModel,
    max_tokens: usize,
) -> (String, Option<String>) {
    let data = format!("User: {user}\nAssistant: {assistant}");

    // Check if data fits within max_indexable_tokens
    let token_count = model.token_count(&data).unwrap_or(max_tokens + 1);

    if token_count <= max_tokens {
        (data, None)
    } else {
        // Generate metadata from user message prefix
        let user_prefix: String = user.chars().take(200).collect();
        let metadata = match batch_num {
            Some(n) => format!("[batch {n}] {user_prefix}"),
            None => user_prefix,
        };
        (data, Some(metadata))
    }
}

fn run_chat(
    args: &Args,
    chat_dir: &Path,
    chat_index: usize,
    plan_index: Option<usize>,
) -> Result<(), String> {
    let tag = match plan_index {
        Some(p) => format!("[{}/{}:plan-{}]", args.scale, chat_index, p),
        None => format!("[{}/{}]", args.scale, chat_index),
    };

    // Load chat.json
    let chat_path = chat_dir.join("chat.json");
    if !chat_path.exists() {
        // Try chat_truncated.json as fallback
        let trunc_path = chat_dir.join("chat_truncated.json");
        if !trunc_path.exists() {
            return Err(format!("{tag} No chat.json or chat_truncated.json found"));
        }
    }

    let chat_file = if chat_path.exists() {
        &chat_path
    } else {
        &chat_dir.join("chat_truncated.json")
    };

    let chat_data = fs::read_to_string(chat_file)
        .map_err(|e| format!("{tag} Failed to read chat: {e}"))?;
    let batches: Vec<Batch> =
        serde_json::from_str(&chat_data).map_err(|e| format!("{tag} Failed to parse chat: {e}"))?;

    // Load probing questions
    let pq_path = chat_dir.join("probing_questions/probing_questions.json");
    if !pq_path.exists() {
        return Err(format!("{tag} No probing_questions.json found"));
    }
    let pq_data =
        fs::read_to_string(&pq_path).map_err(|e| format!("{tag} Failed to read questions: {e}"))?;
    let probing: HashMap<String, Vec<serde_json::Value>> =
        serde_json::from_str(&pq_data).map_err(|e| format!("{tag} Failed to parse questions: {e}"))?;

    // Extract pairs
    let pairs = extract_pairs(&batches);
    let num_batches = batches.len();
    println!("{tag} Loaded {} turns ({} batches)", pairs.len(), num_batches);

    if pairs.is_empty() {
        return Err(format!("{tag} No user-assistant pairs found"));
    }

    // Init Moerae with benchmark config
    let project_id = match plan_index {
        Some(p) => format!("beam-{}-{}-plan-{}", args.scale, chat_index, p),
        None => format!("beam-{}-{}", args.scale, chat_index),
    };

    let config = make_config(args);
    let m = Moerae::builder(&project_id)
        .config(config)
        .build()
        .map_err(|e| format!("{tag} Init failed: {e}"))?;

    let mut conv = m
        .create_conversation()
        .map_err(|e| format!("{tag} Create conversation failed: {e}"))?;

    // Ingest all turns
    let ingest_start = Instant::now();
    let mut ingested = 0;
    let total_turns = pairs.len();

    for (batch_num, user, assistant) in &pairs {
        let (data, metadata) =
            format_turn(*batch_num, user, assistant, m.model(), args.max_indexable_tokens);

        match conv.put(&data, metadata.as_deref(), false) {
            Ok(()) => ingested += 1,
            Err(e) => {
                println!("{tag} Put error (skipping): {e}");
            }
        }

        if ingested % 100 == 0 && ingested > 0 {
            print!("\r{tag} Ingesting [{ingested}/{total_turns}]");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }

    let ingest_secs = ingest_start.elapsed().as_secs_f64();
    println!("\r{tag} Ingested [{ingested}/{total_turns}] ({ingest_secs:.1}s)");

    // Count total questions
    let total_questions: usize = probing.values().map(|qs| qs.len()).sum();

    println!("{tag} Searching {total_questions} questions ({} types)", probing.len());

    // Search for each probing question
    let search_start = Instant::now();
    let mut output_questions: HashMap<String, Vec<OutputQuestion>> = HashMap::new();
    let mut searched = 0;

    for (qtype, questions) in &probing {
        let mut out_items = Vec::new();

        for pq in questions {
            let question_text = pq
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if question_text.is_empty() {
                continue;
            }

            let q_start = Instant::now();

            let retrieved =
                match conv.search_debug(question_text, None, Some(args.search_limit)) {
                    Ok(results) => results
                        .items
                        .into_iter()
                        .enumerate()
                        .map(|(i, r)| RetrievedItem {
                            rank: i + 1,
                            score: r.score,
                            node_id: r.node_id,
                            data: r.data,
                            metadata: r.metadata,
                        })
                        .collect(),
                    Err(e) => {
                        let preview: String = question_text.chars().take(60).collect();
                        println!("{tag} Search error for '{preview}': {e}");
                        Vec::new()
                    }
                };

            let search_ms = q_start.elapsed().as_millis() as u64;

            out_items.push(OutputQuestion {
                original: pq.clone(),
                retrieved_context: retrieved,
                search_ms,
            });

            searched += 1;
        }

        output_questions.insert(qtype.clone(), out_items);
    }

    let search_secs = search_start.elapsed().as_secs_f64();
    println!("{tag} Searched [{searched}/{total_questions}] ({search_secs:.1}s)");

    // Get stats
    let stats = m
        .segment_stats()
        .map_err(|e| format!("{tag} Stats failed: {e}"))?;

    // Build output
    let output = BenchOutput {
        scale: args.scale.clone(),
        chat_index,
        plan: plan_index,
        config: BenchConfig {
            max_indexable_tokens: args.max_indexable_tokens,
            segment_capacity: args.segment_capacity,
            segment_staleness: 999_999_999,
            max_segments: args.max_segments,
            max_cached_indexes: 100,
            search_limit: args.search_limit,
        },
        stats: BenchStats {
            turns_ingested: ingested,
            segments: stats.total_segments,
            total_nodes: stats.total_nodes,
            ingest_secs,
            search_secs,
        },
        questions: output_questions,
    };

    // Write output JSON
    fs::create_dir_all(&args.output)
        .map_err(|e| format!("{tag} Failed to create output dir: {e}"))?;

    let filename = match plan_index {
        Some(p) => format!("{}_{}_plan{}.json", args.scale, chat_index, p),
        None => format!("{}_{}.json", args.scale, chat_index),
    };
    let out_path = args.output.join(&filename);

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("{tag} Serialization failed: {e}"))?;
    fs::write(&out_path, json).map_err(|e| format!("{tag} Write failed: {e}"))?;

    println!("{tag} Done -> {}", out_path.display());
    Ok(())
}

fn discover_chats(base: &Path, scale: &str) -> Result<Vec<(usize, PathBuf, Option<usize>)>, String> {
    let scale_dir = base.join(scale);
    if !scale_dir.exists() {
        return Err(format!("Scale directory not found: {}", scale_dir.display()));
    }

    let mut entries: Vec<(usize, PathBuf, Option<usize>)> = Vec::new();

    let mut dirs: Vec<_> = fs::read_dir(&scale_dir)
        .map_err(|e| format!("Failed to read {}: {e}", scale_dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    dirs.sort_by_key(|e| e.file_name());

    for entry in dirs {
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let chat_idx: usize = match dir_name.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let chat_dir = entry.path();

        // Check for 10M plan structure
        let mut has_plans = false;
        if let Ok(sub_entries) = fs::read_dir(&chat_dir) {
            for sub in sub_entries.flatten() {
                let sub_name = sub.file_name().to_string_lossy().to_string();
                if sub_name.starts_with("plan-") {
                    has_plans = true;
                    if let Some(plan_num) = sub_name.strip_prefix("plan-") {
                        if let Ok(p) = plan_num.parse::<usize>() {
                            entries.push((chat_idx, sub.path(), Some(p)));
                        }
                    }
                }
            }
        }

        if !has_plans {
            entries.push((chat_idx, chat_dir, None));
        }
    }

    entries.sort_by_key(|(idx, _, plan)| (*idx, *plan));
    Ok(entries)
}

fn silence_stderr() {
    unsafe {
        let devnull = std::fs::File::open("/dev/null").unwrap();
        libc::dup2(
            std::os::fd::AsRawFd::as_raw_fd(&devnull),
            libc::STDERR_FILENO,
        );
    }
}

fn main() {
    // Silence stderr permanently (llama.cpp logs during model load + every embed call)
    silence_stderr();

    let args = parse_args();

    let chats = match discover_chats(&args.dataset, &args.scale) {
        Ok(c) => c,
        Err(e) => {
            println!("error: {e}");
            process::exit(1);
        }
    };

    if chats.is_empty() {
        println!("error: no chats found for scale {}", args.scale);
        process::exit(1);
    }

    let chats: Vec<_> = chats
        .into_iter()
        .filter(|(idx, _, plan)| {
            if let Some(target_chat) = args.chat {
                if *idx != target_chat {
                    return false;
                }
            }
            if let Some(target_plan) = args.plan {
                if *plan != Some(target_plan) {
                    return false;
                }
            }
            true
        })
        .collect();

    println!(
        "moerae-bench: {} scale, {} chat(s) to process",
        args.scale,
        chats.len()
    );

    let mut errors = 0;
    for (chat_idx, chat_dir, plan_idx) in &chats {
        if let Err(e) = run_chat(&args, chat_dir, *chat_idx, *plan_idx) {
            println!("error: {e}");
            errors += 1;
        }
    }

    if errors > 0 {
        println!("Completed with {errors} error(s)");
        process::exit(1);
    }

    println!("All done.");
}
