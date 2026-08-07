# Moerae - AI Memory

Local memory database for AI agents. Store facts with `put`, retrieve them with `search`. Content is embedded and indexed automatically.

## Setup

Download the embedding model once:

```sh
mkdir -p ~/.moerae/models
curl -L -o ~/.moerae/models/embeddinggemma-300m-qat-q8_0.gguf \
  "https://huggingface.co/ggml-org/embeddinggemma-300m-qat-q8_0-GGUF/resolve/main/embeddinggemma-300m-qat-Q8_0.gguf"
```

## Storing Content

```sh
# Store a fact (returns conversation UUID for future use)
moerae put -p myproject "The API rate limit is 1000 req/min"

# Store into an existing conversation
moerae put -p myproject -c <uuid> "The deploy key is in vault under /prod/keys"

# Store from stdin (pipe documents, logs, etc.)
cat error.log | moerae put -p myproject --stdin -m "prod error log 2026-04-08"

# Store persistent content (never forgotten, visible across all conversations)
moerae put -p myproject --persist "Database connection string: postgres://..."
```

Content longer than 128 tokens requires a `-m` metadata summary. Only the summary is indexed; full content is stored and retrievable.

## Searching

```sh
# Search within a conversation
moerae search -p myproject -c <uuid> "rate limit"
# 0.9793  1  The API rate limit is 1000 req/min

# Search across all promoted/persistent content in the project
moerae search -p myproject --project-scope "database connection"

# Limit results
moerae search -p myproject -n 5 "deploy"

# JSON output (for programmatic consumption)
moerae search -p myproject --json "error log"
```

Output format: `score\tnode_id\tdata` (tab-separated). Higher score = better match.

## Retrieving Full Content

```sh
# Get the full stored content by node ID (from search results)
moerae get -p myproject 42
```

Useful when the original content was large and search returned only the metadata summary.

## Forgetting Outdated Content

Eviction removes content through disuse — it cannot help with content that is simply
wrong. A stale fact that keeps being retrieved has a high relevancy score, so it is the
last thing evicted. Remove it explicitly.

```sh
# Preview: forget is a DRY RUN unless --yes is passed
moerae forget -p myproject --node 42
moerae forget -p myproject -c <uuid> --query "old API rate limit"

# Apply
moerae forget -p myproject -c <uuid> --query "old API rate limit" --yes
```

- **`--yes` is required to remove anything.** Without it you get the candidate list and
  nothing changes.
- `--query` needs `-c <uuid>` or `--project-scope`; it is refused otherwise, because a
  search without a conversation silently matches nothing.
- `--min-score` defaults to 0.90. Lower it if the fact you mean isn't listed. A false
  positive is unrecoverable, a false negative just means running again.
- Removal is node-precise. Neighbouring content in the same segment is preserved.

The correction workflow is forget-then-put:

```sh
moerae forget -p myproject -c $CONV --query "api rate limit is 1000" --yes
moerae put -p myproject -c $CONV "The API rate limit is 2000 req/min"
```

## Managing State

```sh
# List all projects
moerae projects

# Project statistics
moerae stats -p myproject

# List conversations in a project
moerae convs -p myproject
```

## Key Behaviors

- **put** is synchronous (~5-15ms). Content is searchable immediately.
- **search** returns results ranked by cosine similarity. No configuration needed.
- **Conversations** isolate memory. Content in one conversation is invisible to searches in another unless promoted.
- **Persistence** (`--persist`) makes content visible across all conversations in the project and prevents eviction.
- **Eviction** is automatic. Old, rarely-accessed content is forgotten when segment limits are reached. Persistent content is never evicted.
- **forget** is the explicit counterpart to eviction, for content that is wrong rather than stale. It previews by default and only removes with `--yes`. It is the only way to remove persistent content.
- **Projects** are independent databases. Use `-p` to target a specific project.
