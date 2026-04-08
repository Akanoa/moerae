<div style="text-align: center;">
    <img src="docs/moerae-logo.svg" alt="Moerae">
</div>

<p align="center">A queryable hierarchical AI memory system for AI agents.<br>Two verbs: <code>put</code> and <code>search</code>. No files, no manual indexing, no context window management.<br>Put content in, search it later. Lifecycle handles the rest.</p>

Moerae embeds content locally using [embeddinggemma-300m](https://huggingface.co/ggml-org/embeddinggemma-300m-qat-q8_0-GGUF), stores it in SQLite with [usearch](https://github.com/unum-cloud/usearch) HNSW indexes, and manages segment lifecycle automatically: segments close at capacity, get evicted when stale, and promote to project scope when useful.

## Installation

### Linux / macOS (curl | sh)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Akanoa/moerae/releases/latest/download/moerae-installer.sh | sh
```

### Debian / Ubuntu

```sh
# Download the .deb for your architecture (amd64 or arm64)
curl -LO https://github.com/Akanoa/moerae/releases/latest/download/moerae_0.1.0_amd64.deb
sudo dpkg -i moerae_0.1.0_amd64.deb
```

### Fedora / RHEL

```sh
# Download the .rpm for your architecture (x86_64 or aarch64)
curl -LO https://github.com/Akanoa/moerae/releases/latest/download/moerae-0.1.0.x86_64.rpm
sudo rpm -i moerae-0.1.0.x86_64.rpm
```

### From source

```sh
cargo install --git https://github.com/Akanoa/moerae.git
```

## Quick Start

The embedding model is downloaded automatically on first use (~300MB).

### CLI

```sh
# Store content (returns conversation UUID)
moerae put -p myproject "The capital of France is Paris"

# Store into existing conversation
moerae put -p myproject -c <uuid> "Rust is a systems programming language"

# Search (tab-separated: score, node_id, data)
moerae search -p myproject -c <uuid> "capital of France"
# 0.9793  1  The capital of France is Paris

# Store from stdin
cat document.txt | moerae put -p myproject --stdin -m "document summary"

# Persistent storage (never evicted, promoted to project scope)
moerae put -p myproject --persist "important config"

# Search across project scope
moerae search -p myproject --project-scope "config"

# JSON output
moerae search -p myproject --json "query"

# Get full content by node ID
moerae get -p myproject 42

# Project management
moerae stats -p myproject
moerae convs -p myproject
moerae projects

# Shell completions
moerae completions bash >> ~/.bashrc
moerae completions zsh > ~/.zfunc/_moerae
moerae completions fish > ~/.config/fish/completions/moerae.fish
```

### REPL

```sh
cargo run --bin moerae-repl
```

Interactive shell with Tab completion, Up/Down history, and inline hints. Logs silenced by default (`/verbose` to toggle).

```
myproject> /put The capital of France is Paris
  stored
myproject> /put Rust is a systems programming language
  stored
myproject> capital of France
  [1] score=0.9793 seg=1 node=1
      The capital of France is Paris
```

REPL commands:

| Command | Description |
|---------|-------------|
| `/put <text>` | Store content |
| `/put --persist <text>` | Store persistent content |
| `/put <text> \|\|\| <metadata>` | Store with metadata summary |
| `/search <query>` | Search (or just type the query) |
| `/search --project <query>` | Search project scope |
| `/search --limit N <query>` | Limit results |
| `/get <node_id>` | Fetch full content |
| `/stats` | Project statistics |
| `/convs` | List conversations |
| `/new` | New conversation |
| `/switch <prefix>` | Switch conversation (prefix match) |
| `/delete <prefix>` | Delete conversation |
| `/purge` | Delete all conversations except active |
| `/project <name>` | Switch project |
| `/projects` | List all projects |
| `/delete_project <name>` | Delete a project |
| `/verbose` | Toggle llama.cpp logs |
| `/help` | Help |
| `/quit` | Exit |

### Library usage

```rust
use moerae::Moerae;

let m = Moerae::init("my-project")?;
let mut conv = m.create_conversation()?;

conv.put("The capital of France is Paris", None, false)?;
conv.put("Rust is a systems programming language", None, false)?;

let results = conv.search("capital of France", None, None)?;
for item in &results.items {
    println!("{}", item.data);
}
```

## API

### Project handle

```rust
// Init with defaults
let m = Moerae::init("project-id")?;

// Or use the builder for custom config/model
let m = Moerae::builder("project-id")
    .config(config)
    .model_path("/path/to/custom.gguf")
    .build()?;
```

### Conversations

```rust
let mut conv = m.create_conversation()?;       // new conversation (UUID generated)
let mut conv = m.conversation("uuid-here")?;   // resume existing
m.delete_conversation("uuid-here")?;           // delete + cascade
let list = m.list_conversations()?;            // list all
```

### Put

```rust
// Store content (synchronous, ~5-15ms)
conv.put("some fact", None, false)?;

// Store with persistence (never evicted, promoted to project scope)
conv.put("important config", None, true)?;

// Store large content with metadata summary
conv.put(large_text, Some("summary of the text"), false)?;
```

Content exceeding `max_indexable_tokens` (default: 128) requires metadata. Only the metadata is embedded; the full content is stored and retrievable via `get()`.

### Search

```rust
use moerae::Scope;

// Search current conversation (default)
let results = conv.search("query", None, None)?;

// Search promoted segments across all conversations
let results = conv.search("query", Some(Scope::Project), None)?;

// With result limit
let results = conv.search("query", None, Some(5))?;

// Debug mode (includes scores, segment IDs)
let debug = conv.search_debug("query", None, None)?;
for item in &debug.items {
    println!("score={:.4} data={}", item.score, item.data);
}
```

### Node retrieval

```rust
// Get full content by node ID (from search results)
let content = m.get(results.items[0].node_id)?;
println!("{}", content.data);
```

### Project stats

```rust
let stats = m.segment_stats()?;
println!("{} segments, {} nodes", stats.total_segments, stats.total_nodes);
```

## Configuration

All parameters have sensible defaults. Override via the builder:

```rust
use moerae::Config;

let config = Config {
    max_indexable_tokens: 128,   // metadata required above this
    segment_capacity: 100,       // nodes per segment before close
    segment_staleness: 3600,     // seconds idle before auto-close
    max_segments: 50,            // closed regular segments before eviction
    max_persist_segments: 100,   // closed persist segments (hard cap)
    max_cached_indexes: 20,      // LRU cache for usearch handles
    base_score: 0.3,             // initial segment relevancy
    hit_boost: 0.1,              // relevancy boost per search hit
    promotion_threshold: 0.7,    // auto-promote to project scope
    default_search_limit: 10,    // top-k results
};

let m = Moerae::builder("project").config(config).build()?;
```

## Architecture

### Storage

- **SQLite** (WAL mode) for content, metadata, segment state, config
- **usearch** HNSW indexes (memory-mapped) for vector search
- Layout: `~/.moerae/projects/{hash}/moerae.db` + `indexes/*.usearch`

### Segments

Segments are the unit of lifecycle management. Each conversation has two open segments (regular + persistence). Segments close at capacity or on staleness, and get evicted when the count exceeds `max_segments`.

- **Relevancy**: starts at `base_score`, boosted by `hit_boost` on each search hit, capped at 1.0
- **Promotion**: segments crossing `promotion_threshold` become visible at project scope
- **Eviction**: composite priority based on relevancy, hit count, and time since last hit. Lowest-value segments evicted first. Persistence segments are never evicted.

### Embedding

- Default model: `embeddinggemma-300m-qat-q8_0` (308M params, 768 dims, Q8_0)
- Local inference via llama.cpp (no network calls, no API keys)
- Cosine similarity for ranking
- Vectors normalized (L2) before storage

### Scope

- **Conversation** (default): only the current conversation's segments
- **Project**: only promoted segments across all conversations

No automatic scope widening. The agent controls scope explicitly.

## Model changes

If you switch embedding models, existing segments become mismatched:

```rust
let stats = m.segment_stats()?;
if stats.mismatched_segments > 0 {
    m.rebuild_mismatched_segments()?; // re-embeds all affected nodes
}
```

Search skips mismatched segments and reports `skipped_mismatched` in results.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `rusqlite` | SQLite with WAL mode |
| `usearch` | HNSW vector search |
| `llama-cpp-2` | GGUF model inference + tokenizer |
| `serde` / `serde_json` | Config serialization |
| `thiserror` | Error types |
| `uuid` | Conversation IDs |
| `sha2` | Content dedup, project ID hashing, model fingerprint |
| `lru` | Index handle cache |
| `rustyline` | REPL (history, autocomplete, hints) |
| `libc` | Stderr redirection for log control |

## Benchmark

Moerae is benchmarked against [BEAM](https://github.com/mohammadtavakoli78/BEAM), a long-term memory evaluation suite with 2,000 probing questions across 100 conversations testing 10 memory abilities.

On the 100K scale (20 conversations, 400 questions), Moerae achieves a mean top-1 cosine similarity of **0.86** with 96.5% of questions above 0.80. Knowledge update retrieval reaches **70% rubric coverage** at top-20, and information extraction reaches **42%**.

| Category | Top-1 | Rubric@20 |
|----------|-------|-----------|
| knowledge update | 0.8824 | 70.0% |
| information extraction | 0.8407 | 41.9% |
| temporal reasoning | 0.8735 | 18.8% |
| multi session reasoning | 0.8587 | 14.2% |
| **overall (10 categories)** | **0.8602** | **16.2%** |

Full analysis: [docs/benchmark-analysis.md](docs/benchmark-analysis.md). Benchmark pipeline: [BENCHMARK.md](BENCHMARK.md).

## License

TBD
