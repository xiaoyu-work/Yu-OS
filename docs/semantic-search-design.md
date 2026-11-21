# Semantic Search Architecture

Yu OS currently has three related but distinct local-search surfaces. They
do not yet form one unified document-search pipeline.

| Surface | Implementation | Storage | Current consumer | Status |
| --- | --- | --- | --- | --- |
| Document keyword search | Recoll / Xapian | `~/.recoll/` | Files product's `docs` App | Shipped |
| Filesystem semantic prototype | `yuos-semantic` orchestration + `yuos-embed` primitives | JSON-backed `MemoryStore` | `yuos-semantic` CLI only | Experimental |
| Agent semantic memory | `crates/yuos-embed` + core memory adapter | SQLite `SemanticStore` | Agent indexing and `yu_recall_semantic` | Shipped when embedding is configured |

The Recoll and `yuos-semantic` user units are enabled under
`default.target`, so they can run in headless user sessions. Each unit uses
`ConditionPathIsExecutable` and skips cleanly when its optional binary is not
installed.

## Document Search: Recoll

The `docs` App, owned by
[`yuos-app/products/files/apps/docs`](https://github.com/xiaoyu-work/yuos-app/tree/main/products/files/apps/docs),
currently uses Recoll only. OS builds consume its fixed revision through
`packaging/apps.lock.json`; Recoll binaries and background service installation
remain system responsibilities.

1. `recollindex` builds the Xapian index over configured top directories.
2. `recollq` executes keyword queries.
3. The App converts Recoll results into its structured response.

Recoll is strong for exact terms, names, numbers, and code tokens. Config and
index data live under `~/.recoll/`.

The current `docs.search` MCP tool does **not** query either semantic
store and does not perform result fusion.

## Filesystem Semantic Prototype: `yuos-semantic`

`crates/yuos-semantic` remains a Phase-1 daemon and CLI:

```text
watched files (`yuos-semantic` orchestration)
  -> yuos_embed::TextExtractor
  -> yuos_embed::chunks_for
  -> yuos_embed::StubEmbedder (384 dimensions)
  -> yuos_embed::MemoryStore persisted as JSON
  -> yuos-semantic CLI queries
```

The daemon watches configured directories with notify/inotify. Its
`StubEmbedder` deterministically hashes content, so the pipeline can be
exercised end to end, but the resulting similarity scores are not meaningful
semantic embeddings.

This prototype is not connected to the `docs` App.

`yuos-embed` is the sole owner of the chunk, embedder, extractor, filesystem
walker, and store contracts. `yuos-semantic` retains compatibility re-exports
for its former library module paths, but new callers should import
`yuos_embed` directly.

## Agent Semantic Memory: `yuos-embed`

The production semantic-memory path lives in `crates/yuos-embed` and
`core/src/agent/memory/semantic.rs`:

```text
agent message or memory item
  -> configured embedding task
  -> SemanticStore (SQLite)
  -> cosine search
  -> yu_recall_semantic
```

The default local embedding task uses the bundled Qwen3-Embedding-0.6B model
through ONNX Runtime GenAI and emits 1024-dimensional vectors. Embedding
provider `auto` prefers the configured local task and can use an
OpenAI-compatible provider when configured.

`SemanticStore` records the embedding model identity with stored vectors and
rejects incompatible model changes instead of silently mixing vector spaces.
The default database is the Agent semantic store below the Yu OS data
directory.

This path provides real semantic recall for Agent memory, but it does not
index the user's document directories for the `docs` App.

## Persistence and Caller Compatibility

The consolidation does not migrate or rewrite filesystem semantic data.
`MemoryStore` still reads and writes the same JSON array at
`$XDG_STATE_HOME/yuos-semantic/store.json` (falling back to
`~/.local/state/yuos-semantic/store.json`), with the same row fields and
384-dimensional `stub-sha256` vectors. Existing indexes therefore open in
place.

The `yuos-semantic` and `yuos-semantic-daemon` executable names, command JSON,
configuration path, and systemd user unit remain unchanged. Existing Rust
imports through `yuos_semantic::{chunk, embed, extract, store, watch}` remain
available as compatibility re-exports; callers can migrate to `yuos_embed`
without a coordinated data or service cutover.

The re-exported `VectorStore` methods and `MemoryStore::open` retain their
former `anyhow::Result` signatures, so existing store implementations and
callers remain source-compatible. The shared implementation constructs typed
`DocumentStoreError` causes internally; callers that need structured handling
can downcast the returned `anyhow::Error`.

The former `yuos_semantic::Embedder::embed(&[String])` contract is the one
source-level change: implementers and direct trait callers migrate to the
shared async `yuos_embed::Embedder` request/response API by constructing
`EmbedRequest` and awaiting `embed`. The shipped daemon and CLI already use
that path; persisted vectors and service protocols are unchanged.

## Why the Search Paths Are Separate

- Recoll is an external document-indexing system with broad format support.
- `yuos-semantic` is a standalone filesystem-watching prototype.
- `yuos-embed` is a reusable embedding/storage library integrated with Agent
  memory.

Keeping these boundaries explicit prevents the prototype daemon from being
mistaken for the production Agent semantic store.

## Planned Document Fusion

The intended `docs.search` design is still:

1. query Recoll for keyword matches,
2. query a real filesystem semantic index,
3. merge the ranked lists with Reciprocal Rank Fusion.

```text
score(doc) = sum over each source S of:
                1 / (k + rank_S(doc))
```

Before implementing fusion, the filesystem daemon must be upgraded from the
compatibility stub embedder and JSON store to a real embedding provider and a
production-scale document store. Those implementations belong behind the
contracts in `yuos-embed`; daemon lifecycle remains in `yuos-semantic`.

Only after that decision should the `docs` App gain semantic queries and RRF
fusion.

## Current Status

- `docs`: Recoll keyword search only.
- `yuos-semantic`: optional filesystem daemon/CLI using `yuos-embed`
  primitives with compatibility JSON persistence and stub embeddings.
- Agent semantic recall: real embeddings and SQLite storage when configured.
- Document semantic fusion: not implemented.
