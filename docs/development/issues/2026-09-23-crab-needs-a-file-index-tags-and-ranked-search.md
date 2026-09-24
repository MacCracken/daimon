# crab needs a file index, per-file tags and ranked file search — and daimon has no surface for any of them

**Status:** 🔴 **OPEN.** A consumer need, not a defect: nothing in daimon is broken. daimon decides
the mechanism; this filing states what crab has to be able to ask and get back, and the constraints
the answer has to fit, and deliberately stops there.
**Filed:** 2026-09-23, by crab (the AGNOS file manager), at the daimon 2.1.4 → 2.4.3 pin move —
the first pin at which daimon builds and runs on agnos.
**Affects:** crab's M7 (the index: disk-wide index, tags, smart folders) and M8 (assisted search).
Both are crab's last two feature milestones before its 1.0.
**Severity:** daimon's to rank. It holds no crab release; it holds every crab feature that needs a
persisted, disk-wide view of files.

## Thank you, and what the agnos build did and did not unblock

[`archive/2026-09-14-daimon-does-not-build-for-agnos.md`](archive/2026-09-14-daimon-does-not-build-for-agnos.md)
is resolved, and crab re-derived that rather than reading it: at 2.4.3, `cyrius build --agnos
src/main.cyr` is **0 errors** (one unreachable `sys_dup2` warning), and the 2.4.3 commit's CI job
**"AGNOS guest test — Guest test on the released agnos kernel" passed**.

That filing named what it was blocking: *"everything that needs a persisted, disk-wide index: tags,
`Untagged`, `Unrated`, and the `Local · N files` surface."* The build was necessary for those and it
was not sufficient — **none of them exists in daimon on any target.** crab's own roadmap treated the
agnos build as the last gate and that was crab's error, not daimon's; this filing is the correction,
put where it can be acted on.

## What crab is, for this purpose

A GUI file manager on agnos, spawned by the compositor, drawing through dhancha and setu. It declared
daimon on 2026-09-14 as the back end for **files-as-memory** — the sibling of mneme's notes-as-memory
over the same substrate: *crab reads the shared index; it does not silo its own.* It does **not**
link daimon (no `modules`; daimon ships no `dist/` and should not be folded into a file manager). It
reaches daimon the way 2.3.0's CHANGELOG says: *"crab reaches it over HTTP."*

## What crab needs to be able to ASK, and what it needs BACK

Each item quotes crab's design canvas (`crab/Crab File Manager Mockups.dc.html`), which is where the
need comes from. **None of it prescribes a route, a schema or a storage format.**

### M7 — the index

1. **A disk-wide file index that daimon keeps.** *"INDEX · Local · 41,208 files"*, *"index fresh"*,
   *"Index · embedding new files 31% · 251 / 812 · local · pauses on battery"*. crab needs to learn
   whether an index exists, how many files it holds, whether it is current or catching up (and how
   far), and to ask for a path or a selection to be (re-)indexed (*"Re-index selection"* in the
   canvas's `Index` menu). It must **survive a daimon restart**, it is built **in the background**,
   and it **pauses on battery**.
2. **Tags, per file, persisted.** Add and remove a tag on a file; read a file's tags; list the files
   carrying a tag; list the tags that exist, with counts (*"TAGS · toolchain · specs · archive ·
   wip"*, *"select 48 · delivered 120 · reject 77"*); and **suggested** tags for a file or a folder
   (*"SUGGESTED TAGS · src → + toolchain + cyrius + wip"*). A tag match is **exact** — see
   *Why what exists cannot stand in* below.
3. **A rating per file**, because *"Unrated 301"* is a smart folder: set it, read it, and ask which
   files have none.
4. **Smart folders: saved queries over the index, each with a count** — *Recent 64 · Duplicates 12 ·
   Untagged 318 · Large & old 27 · Raw only 812 · Unrated 301*. Each answers *the files it holds* and
   *how many*. ⚠ crab already finds duplicates **within one listing** by itself (crab 0.10.0,
   `Shift+D`: a free size pre-filter, then a bounded content hash) and needs nothing from daimon for
   that; **disk-wide** duplicates are the index's.

### M8 — assisted search

5. **A question in words, answered with RANKED FILES** — file identities crab can turn into paths,
   in rank order, each with a score or rank; **why it matched**, as a sentence (*"Contains an invoice
   total and a paid stamp; dated inside the requested window"*); **where it appears** (the tags and
   smart folders that hold it); facets to refine by (*Kind · Date · Size · Location*); duplicates
   within the result set; the cost (*"42 hits in 31 ms · vector + name match"*); and a way to **save
   the query as a smart folder**. The canvas makes a promise in its UI — *"no external service ·
   index stays on device"* — so the embedding has to be local, and crab will print that line only if
   it is true.

M8 is crab's release after M7. Items 1–4 are the ones crab can build against first.

## Why what exists cannot stand in

Measured on 2.4.3's source, not inferred:

- **No route is about files.** None of the 25 path branches in `http_route` (`src/router.cyr`) names
  a file, a path, a tag or an index.
- **The memory store has no route and no tag field.** `src/memory.cyr` is a per-agent key-value store
  whose only caller is its constructor in `app_init`; its own KNOWN GAP note records that
  `memory_store_list_by_tag` is *"a SUBSTRING search over the whole record — a tag "a" matches any
  record containing the letter a."*
- **RAG is not an index of files**, in five independent ways:
  1. **It does not persist** — nothing in `src/vector_store.cyr`, `src/fed_vector_store.cyr` or
     `src/rag.cyr` writes to disk, so an index of a disk would die with the process;
  2. **no file identity comes back** — `POST /v1/rag/query` answers `{"formatted_context": "Use the
     following context to answer the question…"}`, a prompt; crab would have to parse paths out of
     prose, which is a guess dressed as a contract;
  3. **it cannot rank by meaning** — the embedding is 32 slots indexed by the sum of a token's bytes
     mod 32, so `stop`, `pots` and `tops` are one token;
  4. **a disk will not go through it** — 120 requests/min per source is 41,208 files in ≈ 5.7 h, and
     on agnos every local client shares the one "unidentified" bucket (2.1.7);
  5. **and a 64 KB body cap** stands between it and file content.
- ⚠ **mneme's README names `/v1/knowledge/*`, which no route serves.** Recorded because mneme is the
  other consumer of "the shared index" — whatever daimon designs here, the two should share it.

## The constraints the answer has to fit, on crab's side

Facts, measured in crab and in daimon's own agnos work — not a transport design. **crab has no
attachment to HTTP**; if daimon would rather serve local GUI clients some other way, that is
daimon's call and crab will follow it.

- **crab's event loop must never block.** It idles with `pause`#14 and never `sleep_ms`#41 — a crab
  0.5.0 draft that slept froze the whole desktop (placed 2, presented 0). Whatever crab calls, it
  has to be able to interleave with frames.
- **daimon's own agnos filings sit squarely on an HTTP client in crab**, and crab would inherit all
  of them (in `agnos/docs/development/issues/`):
  - `2026-09-23-sock-send-and-connect-hold-the-cpu.md` — `#47` / `#48` hold the CPU while they wait;
    a local send over 2 KB can stop the machine (audit AG-14);
  - `2026-09-23-sock-recv-never-reports-eof-after-peer-fin.md` — a client must frame by
    `Content-Length`;
  - `2026-09-23-tcp-server-cannot-be-loopback-only.md` — TCP to 127.0.0.1 is dropped, so a local
    client dials `sys_net_ip()` (as `tests/agnos/http_client.cyr` does) and sends `Host: 127.0.0.1`;
  - `2026-09-23-inbound-tcp-syn-dropped-by-isr-drain.md` — a server accepted 0 to 9 of 12
    connections, depending on how it waits;
  - `2026-09-23-socket-ids-have-no-owner.md` — any process can read, write or close any connection.
  - and the kernel has **8 TCP slots for the whole machine**, of which daimon serves at most 5
    (`SERVE_MAX_CONNS`, `src/server.cyr:285`) — so every crab connection is one of those.
- **The index is an enrichment, not a precondition.** crab lists, copies, moves and deletes with no
  daimon on the box, and must be able to tell three kinds of nothing apart: *daimon is not running*,
  *daimon holds no index*, and *the index is still building*. crab's rule is to say which kind of
  nothing it is showing.

## What crab will not do meanwhile

- **It will not build its own disk-wide index.** That would silo the shared index the stack's design
  says crab reads, and duplicate daimon's job in a file manager.
- **It will not ship tag or smart-folder rows that answer "unavailable".** A row that answers
  identically on every box for every file forever is a probe — the failure crab's sidebar VOLUMES
  entry exists to name, where crab declined to approximate and agnos minted `mountlist`#104 instead.
- It ships nothing for M7's remaining half until daimon has a surface for it.

## What would close it

daimon chooses the mechanism. At minimum, crab needs:
1. a documented way to ask for items 1–4 and get back **file identities crab can turn into paths**;
2. **persistence** across a daimon restart;
3. a reach from a GUI client on agnos that fits *the constraints* above.

Item 5 (M8) can be a later daimon release; it is crab's release after M7.

## Related

- crab `cyrius.cyml` `[deps.daimon]` — declared, not linked; pinned 2.4.3 as of crab 0.10.4.
- crab `docs/development/roadmap.md` § M7 and § M8 — what the index is for, and what it must not
  require.
- crab's copy of this filing: `crab/docs/development/issues/2026-09-23-daimon-has-no-file-index-tags-or-ranked-search.md`.
- [`archive/2026-09-14-daimon-does-not-build-for-agnos.md`](archive/2026-09-14-daimon-does-not-build-for-agnos.md) — the predecessor.
- `docs/guides/api.md`; `src/memory.cyr` (the KNOWN GAP note); `src/rag.cyr`.
- mneme `README.md` — `/v1/rag/*` and `/v1/knowledge/*`.
