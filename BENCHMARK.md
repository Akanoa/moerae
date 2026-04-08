# Benchmarking Moerae with BEAM

[BEAM](https://github.com/mohammadtavakoli78/BEAM) is a benchmark for evaluating long-term memory in LLM conversations. It provides 2,000 probing questions across 100 multi-turn conversations at four scales (100K, 500K, 1M, 10M tokens), testing 10 memory abilities: information extraction, temporal reasoning, contradiction resolution, multi-session reasoning, and more.

Moerae integrates as a retrieval backend — conversation turns are ingested via `put()`, probing questions are answered by `search()` + an LLM reader, and BEAM's evaluation pipeline scores the results.

## Prerequisites

- Moerae built (`cargo build --release`)
- Embedding model installed at `~/.moerae/models/embeddinggemma-300m-qat-q8_0.gguf`
- Python 3.10+
- Claude Code CLI installed (`claude --print`)
- Git (for dataset download)

## Pipeline

### 1. Download the dataset

```bash
bash bench/download_beam.sh
```

Downloads BEAM conversations into `bench/beam-data/chats/` via sparse git clone. The directory structure:

```
bench/beam-data/chats/
├── 100K/   (20 conversations)
├── 500K/   (35 conversations)
├── 1M/     (35 conversations)
└── 10M/    (10 conversations, each with plan-0 through plan-9)
```

### 2. Run retrieval (Rust)

Ingest conversations into Moerae and retrieve context for each probing question:

```bash
# Single conversation (quick test)
./target/release/moerae-bench \
    --dataset bench/beam-data/chats \
    --scale 100K \
    --chat 1

# Full scale
./target/release/moerae-bench \
    --dataset bench/beam-data/chats \
    --scale 100K

# 10M scale, single plan
./target/release/moerae-bench \
    --dataset bench/beam-data/chats \
    --scale 10M \
    --chat 1 \
    --plan 0
```

Output goes to `bench/output/` by default (one JSON per conversation).

#### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--dataset <path>` | required | Path to BEAM `chats/` directory |
| `--scale <100K\|500K\|1M\|10M>` | required | Conversation scale |
| `--output <path>` | `bench/output` | Output directory |
| `--chat <N>` | all | Run single conversation index |
| `--plan <N>` | all | For 10M, run single plan index |
| `--search-limit <N>` | 50 | Top-k results per question |
| `--segment-capacity <N>` | 500 | Nodes per segment before close |
| `--max-indexable-tokens <N>` | 512 | Token threshold requiring metadata |
| `--max-segments <N>` | 5000 | Max closed segments before eviction |

### 3. Generate answers (Python)

Uses `claude --print` (Claude Code CLI) — no API key needed:

```bash
python bench/generate_answers.py \
    --input bench/output/100K_1.json \
    --output bench/results/100K/1/
```

This calls `claude --print` for each probing question with retrieved context, and writes `moerae_answers.json` in BEAM's expected format.

To process all outputs for a scale:

```bash
bash bench/generate_all_answers.sh 100K
```

#### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--input <path>` | required | Intermediate JSON from step 2 |
| `--output <path>` | required | Output directory for results |
| `--model <name>` | `sonnet` | Claude model (sonnet, opus, haiku) |
| `--max-context <N>` | 20 | Max retrieved excerpts in prompt |

### 4. Analyze retrieval quality

Analyze Moerae's retrieval performance directly from the Phase 1 output — no LLM judge needed:

```bash
# All conversations in a scale
python bench/analyze.py bench/output/

# Single conversation
python bench/analyze.py bench/output/100K_1.json
```

Reports per-category metrics:

- **Top-1 / Top-5 score**: mean cosine similarity of the best (or top 5) retrieved excerpts
- **Rubric@k**: fraction of BEAM's rubric criteria found in the top-k retrieved excerpts (proxy for recall without LLM judging)
- **Breakdown by difficulty**: easy/medium/hard
- **Score distribution**: histogram of top-1 scores

Example output (100K scale, 20 conversations):

```
  Category                       N   Top-1   Top-5  Rubric@5  Rubric@10  Rubric@20
  information extraction        40  0.8407  0.8202    38.8%     41.9%     41.9%
  multi session reasoning       40  0.8587  0.8440     9.3%      9.8%     14.2%
  knowledge update              40  0.8824  0.8457    60.0%     67.5%     70.0%
  temporal reasoning            40  0.8735  0.8496    17.5%     18.8%     18.8%
  contradiction resolution      40  0.8746  0.8408     0.0%      0.0%      0.0%
  OVERALL                      400  0.8602            13.9%     15.3%     16.2%
```

### 5. Evaluate with BEAM (optional)

For full LLM-as-judge scoring, clone BEAM and run its evaluation pipeline on the answers from step 3:

```bash
git clone https://github.com/mohammadtavakoli78/BEAM.git /tmp/BEAM

python -m src.evaluation.run_evaluation \
    --input_directory bench/results/100K \
    --chat_size 100K
```

See [BEAM's README](https://github.com/mohammadtavakoli78/BEAM) for evaluation details and LLM judge configuration.

## How it works

Each BEAM conversation is processed as follows:

1. **Ingest**: Every user-assistant exchange pair becomes one `put()` call. The combined text (`"User: ...\nAssistant: ..."`) is embedded directly when under 512 tokens; longer turns use a metadata summary for embedding while storing the full text.

2. **Search**: Each probing question is passed to `search_debug()`, returning the top-k most similar conversation excerpts with cosine similarity scores.

3. **Answer**: Retrieved excerpts are formatted as context for an LLM, which generates an answer.

4. **Score**: BEAM's LLM-as-judge evaluates answers against reference answers across 10 memory ability categories.

Each conversation gets its own Moerae project (isolated storage), so results are independent and reruns don't contaminate prior data. Benchmark projects are stored under `~/.moerae/projects/beam-*`.

## Cleaning up

Remove benchmark data after a run:

```bash
# Remove benchmark Moerae projects
rm -rf ~/.moerae/projects/beam-*

# Remove intermediate and result files
rm -rf bench/output bench/results
```
