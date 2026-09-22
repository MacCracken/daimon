# RAG-stored chunks alias the transient request buffer — the 1.2.5 MCP-registry bug, unswept in `src/rag.cyr`

**Status:** 🔴 **OPEN — MEASURED, REPRODUCED ON BOTH TOOLCHAINS.** `POST /v1/rag/ingest` stores its
chunk text and metadata as `str_sub` **views** into the per-connection request buffer, not copies. The
buffer is reused, so `POST /v1/rag/query` returns whatever now occupies those bytes — including
**another client's request body**.
**Filed:** 2026-09-22, during the 2.1.5 toolchain bump, by the roadmap's own post-bump check ("one
agent lifecycle through the HTTP API and a vector-store round trip"). No test caught it; see
§"Why the suite is green".
**Affects:** every release carrying `src/rag.cyr`'s `chunk_text` retention path. **Not a 2.1.5
regression** — see §"A/B".
**Severity:** **P1 — cross-request data disclosure on an unauthenticated endpoint.**

⚠ **This is a recurrence of a documented, consumer-reported, already-fixed bug.**
[`2026-06-11-mcp-registry-aliases-request-buffer.md`](2026-06-11-mcp-registry-aliases-request-buffer.md)
is the same defect in the MCP registry — filed by **thoth**, severity HIGH, **RESOLVED in daimon
1.2.5**. `src/mcp.cyr` carries two ⚠ comment blocks calling it *"a documented CVE-class bug in this
very file"* and instructing *"str_clone EVERY FIELD, exactly as `mcp_register_external` does."* The
1.2.5 fix was applied to the MCP registry and **never swept to the other retaining stores**:

```
$ grep -c str_clone src/mcp.cyr src/rag.cyr src/vector_store.cyr src/fed_vector_store.cyr src/memory.cyr
src/mcp.cyr:16   src/rag.cyr:0   src/vector_store.cyr:0   src/fed_vector_store.cyr:0   src/memory.cyr:0
```

## What was measured

`build/daimon` (2.1.5, cyrius 6.6.6). One ingest, then one query:

```
$ curl -s -XPOST :18090/v1/rag/ingest -d '{"text":"Daimon is the AGNOS agent orchestrator. It supervises agents and dispatches MCP tools.","metadata":"smoke"}'
{"chunk_ids":[1]}

$ curl -s -XPOST :18090/v1/rag/query -d '{"query":"what orchestrates AGNOS agents?"}'
{"formatted_context":"... [1] hat orchestrates AGNOS agents?\"} rator. It supervises agents and dispatches MCP tools. ..."}
                             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                             the QUERY body, where the stored text should be
```

Chunk `[1]` should begin `Daimon is the AGNOS agent orchest…`. Its first 35 bytes are instead the bytes
of the *query* request just received; the tail (`rator. It supervises…`) is the remainder of the
original ingest body that the shorter query did not overwrite.

With padding the aliasing is unambiguous — ingest 64 `A`s, then query with `Z`s:

```
{"formatted_context":"... [1] ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ\"} AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
                          [2] ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ\"} AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",\"metadata\":\"m2\"} ols. ..."}
```

Three different requests' bytes in one stored record: the live query, the previous ingest, and `ols.`
— the tail of `…MCP tools.` from the first ingest, two requests back.

## Root cause

1. **`str_sub` is a view by contract.** `lib/str.cyr:191-203`: *"Substring (allocates new Str header,
   shares data with parent) … the data ptr is shared with the parent Str — this is a view, not a
   copy."*

2. **`chunk_text` builds every chunk with it** (`src/rag.cyr`), over `text` = `jget(pairs, "text")`
   (`src/api_rag.cyr:9`), which points into the request body:
   ```cyrius
   var chunk = str_sub(text, pos, end - pos);
   vec_push(chunks, chunk);
   ```

3. **The view is RETAINED past the request** — `src/rag.cyr:140-141`:
   ```cyrius
   var entry = vec_entry_new(id, emb, dim, chunk, metadata);
   vindex_insert(load64(rp), entry);
   ```
   The 16-byte `Str` header outlives the request; the bytes it points at do not.

The buffer is explicitly reused — `src/server.cyr:210`: *"sandhi sizes its **reused per-batch arena**
at max_conns × (HSV_REQ_BUF_SIZE + slack) … allocated once."* `metadata` is the same shape (a raw
`jget` result stored into the entry), which is why `,"metadata":"m2"}` appears inside a chunk above.

The **embedding is unaffected** — computed into a fresh `alloc(dim * 8)` at ingest — so similarity
ranking still reflects the original text. Only the returned *content* is corrupt, which is what makes
this quiet: results look correctly ranked and read as garbage.

## Blast radius is contained to one call site

`vec_entry_new` / `vindex_insert` have exactly one caller in the tree:

```
$ grep -rn 'vec_entry_new\|vindex_insert' src/ | grep -v '^src/vector_store.cyr:.*fn '
src/rag.cyr:140:        var entry = vec_entry_new(id, emb, dim, chunk, metadata);
src/rag.cyr:141:        vindex_insert(load64(rp), entry);
```

`src/vector_store.cyr` and `src/fed_vector_store.cyr` have no `str_clone` but also no request-derived
retention of their own — they are reached through this path. `src/memory.cyr` writes its value to a
file *within* the request, so its zero clone count is safe. **The fix is one site.**

## Why the suite is green

All 235 `.tcyr` assertions and `fuzz/vector_store.fcyr` drive the store with `str_from(...)` literals.
`str_from` is itself a view (`lib/str.cyr`, stores the cstr pointer), but over **static** bytes that
are never reused — so the view is stable and the assertion passes. The bug requires a reused arena
underneath the parent `Str`, which only the live HTTP path provides.

## A/B: pre-existing, not a bump regression

Identical probe, identical corruption, same tree, differing only in `cyrius.cyml`:

| build | toolchain + deps | chunk `[1]` after a `ZZZZ…` query |
|---|---|---|
| `daimon-664` | cyrius 6.6.4, 2.1.4 dep pins | `ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"}` |
| `daimon-666` | cyrius 6.6.6, 2.1.5 dep pins | `ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"}` |

The stored marker `DAIMON_ORIGINAL_STORED_TEXT_MARKER` is absent from both. The 2.1.5 bump neither
introduced nor masked it.

## Fix

Copy at the **retention** boundary, not the view boundary. `str_sub` is right as a view — chunking a
5 KB body into overlapping windows without copying is the point, and `tokenize` consumes the views
harmlessly inside the request. The defect is retaining one.

1. **`src/rag.cyr:140`** — clone both retained fields, exactly as `mcp_register_external` does:
   ```cyrius
   var entry = vec_entry_new(id, emb, dim, str_clone(chunk), str_clone(metadata));
   ```
   `str_clone` (`lib/str.cyr:218`) allocates `slen + 1` and null-terminates; the bump allocator never
   frees, so the copy is stable for the process lifetime. One `memcpy` per stored chunk over bytes the
   chunker already walked — `rag_ingest_5k_chars` is 4.2 µs today; re-bench after.
2. **Carry the `src/mcp.cyr` ⚠ convention comment to `src/rag.cyr`**, so the next store added there
   inherits the rule rather than rediscovering it in a third module.
3. **Regression test — the pattern already exists in-repo.** 1.2.5 shipped an `mcp_registry` test that
   "registers a tool from a mutable buffer, overwrites every source byte as a later request would, and
   asserts the registry still resolves the original name + URL." Clone it for RAG: ingest from a
   heap buffer, overwrite every source byte, then assert the retrieved chunk is byte-identical to what
   went in. This runs in a `.tcyr` with no socket and catches the class.
4. **Sweep, don't spot-fix.** Audit every `jget` result that outlives its handler across `src/api_*`;
   the two occurrences so far were found three months apart by a consumer and by a smoke test, never
   by review.

## Related

- [`2026-06-11-mcp-registry-aliases-request-buffer.md`](2026-06-11-mcp-registry-aliases-request-buffer.md)
  — same defect, MCP registry, HIGH, resolved 1.2.5. Its fix and test are the template.
- `src/mcp.cyr:88-94` and `:163-172` — the two ⚠ convention blocks `src/rag.cyr` does not follow.
- `lib/str.cyr:191-203` (`str_sub` view contract), `:218` (`str_clone`).
- `src/server.cyr:210` — the reused per-batch arena this aliases into.
- VULN-007 (roadmap, Security Gates) — the bump-allocator reuse family, seen here from the consumer
  side: not a reset that fails to zero, but a retained pointer into memory legitimately reused.
