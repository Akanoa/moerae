---
name: moerae-second-memory
description: Use moerae as an out-of-context second memory to cut the tokens sent to the AI platform. Use when a session handles large artifacts (logs, dumps, API responses, file trees, transcripts, research notes), when context is filling up, when facts must survive across sessions, or when the user asks to reduce token usage / context size / cost.
---

# Moerae as a second memory

The context window is the expensive channel: everything in it is re-uploaded on
every single turn. A 30k-token log pasted once costs 30k tokens *per request*
for the rest of the session.

Moerae is a local memory database (`put` / `search`, embeddings computed
on-device, no network, no API keys). Move bulk content there and keep only
*pointers* in context. The rule:

> Large content goes to moerae. A one-line summary and a node id stay in context.
> Full content comes back only when a specific task needs it.

## Setup (once per session)

Search is scoped to a conversation. `moerae search` **without `-c` creates a new
empty conversation and returns nothing** — the single most common mistake. Pin
one UUID at the start of the session and reuse it for every call.

```sh
# Create the session conversation and remember its UUID
CONV=$(moerae put -p "$PROJECT" "session start: <one line on what we are doing>")
echo "$CONV" > .moerae-conv     # or keep it in the session notes
```

Reuse it later, including in a future session:

```sh
CONV=$(cat .moerae-conv)
moerae put    -p "$PROJECT" -c "$CONV" "..."
moerae search -p "$PROJECT" -c "$CONV" "..."
```

`-p` defaults to `default`; always pass a real project name so memories from
different repos do not mix.

## Offloading large content

Anything over ~128 tokens needs a `-m` summary. **Only the summary is embedded
and returned by search**; the full body is stored and fetched on demand. That
asymmetry is the whole token saving.

```sh
# A log you would otherwise paste into context
kubectl logs deploy/api --since=1h \
  | moerae put -p "$PROJECT" -c "$CONV" --stdin \
      -m "api pod logs 2026-08-06 14:00-15:00, repeated 502 from upstream auth"

# A large file
moerae put -p "$PROJECT" -c "$CONV" --stdin -m "schema.sql: 40 tables, billing domain" < schema.sql

# A web page / API response you fetched
curl -s "$URL" | moerae put -p "$PROJECT" -c "$CONV" --stdin -m "stripe webhook payload reference"
```

Write the `-m` summary as the *question it answers*, not as a title. `"auth
service 502s caused by expired upstream cert"` retrieves far better than
`"logs"`, because the summary is what gets embedded and matched.

## Retrieving narrowly

Two-step retrieval keeps the cost proportional to what you actually need:

```sh
# 1. Cheap: summaries only, a handful of lines
moerae search -p "$PROJECT" -c "$CONV" -n 3 "why did auth start failing"
# 0.8412  17  api pod logs 2026-08-06 14:00-15:00, repeated 502 from upstream auth

# 2. Expensive, and only if step 1 points at something you need in full
moerae get -p "$PROJECT" 17
```

Output is `score<TAB>node_id<TAB>data`. Use `--json` when parsing.

Grep the retrieved body before reading it whole — `moerae get -p "$PROJECT" 17
| grep -c 502` costs a handful of tokens instead of thousands.

Keep `-n` small (3–5). The default is 10, and ten summaries in context on every
lookup adds up.

## Facts that must outlive the session

`--persist` content is never evicted and becomes visible at project scope, so it
is reachable from any conversation without knowing its UUID:

```sh
moerae put -p "$PROJECT" --persist "deploy runbook: ./ops/deploy.sh, needs VAULT_TOKEN"
moerae search -p "$PROJECT" --project-scope "how to deploy"
```

Use this for stable project knowledge (conventions, decisions and their reasons,
where things live, past incidents). Non-persistent content is evicted as
segments age out, which is the intended behaviour for session scratch.

Note that `~/.moerae` is a plaintext local SQLite database. Store the *location*
of a secret, not the secret.

## What goes where

| Content | Action |
|---|---|
| Log dumps, stack traces, CI output | `put --stdin -m "<what it shows>"` |
| Large file or schema you only need parts of | `put --stdin -m`, then `get` + grep |
| Fetched docs, API responses, transcripts | `put --stdin -m` |
| A conclusion, decision, or convention | `put --persist` (short, stays cheap) |
| Small values in active use right now | keep in context — round-tripping costs more than it saves |
| Secrets, tokens, credentials | never; store where to find them |

## Working loop

1. **Before** answering a recall-flavoured question ("what was the error we saw",
   "what did we decide about X", "what's the deploy process"), search moerae
   first instead of asking the user to re-paste.
2. **After** producing or receiving anything bulky, `put` it and keep the node id
   in your reply rather than the content.
3. **At the end** of a work chunk, persist the durable outcome in one or two
   lines: what changed, why, what is still open.

State briefly what you offloaded (`"stored the full log as node 17"`) so the user
knows the content is retrievable and not lost.

## Checking state

```sh
moerae stats -p "$PROJECT"     # segments / nodes
moerae convs -p "$PROJECT"     # conversation UUIDs with counts
moerae projects                # all projects
```

If `stats` reports `mismatched` segments, the embedding model changed and those
segments are skipped by search until rebuilt (`rebuild_mismatched_segments()` in
the library API).

## Cost model, concretely

A 40k-token log stored with a 15-token summary, in a session that runs 30 more
turns:

- pasted into context: ~40k × 30 = **1.2M tokens** billed
- in moerae: 15 tokens per turn, plus one `get` if it is actually needed —
  **under 1k tokens**, and only the relevant part comes back

The saving is not the `put`; it is every turn afterwards that no longer carries
the payload.

## Installing this skill

Copy this directory to `~/.claude/skills/moerae-second-memory/` for every
project, or to `<repo>/.claude/skills/` for a single one.
