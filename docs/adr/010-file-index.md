# ADR-010: A file index on the loop, off unless the operator names a root

**Status**: Accepted
**Date**: 2026-11-06 (2.5.4)
**Context**: crab, the AGNOS file manager, needs a persisted, disk-wide view of files: an index, tags,
ratings and smart folders ([the filing](../development/issues/2026-09-23-crab-needs-a-file-index-tags-and-ranked-search.md),
items 1–4; item 5, ranked search, is crab's next release). None of it existed. Four things constrain
the answer: the HTTP API has no caller authentication until 2.6.x, daimon is one thread whose handlers
must not block (ADR-006), `alloc` never frees, and on agnos the machine has 16 process slots.

## Decision

**Nothing is indexed unless the operator names a root (`serve --index-root DIR`).** Whatever the index
holds, any local process can read, which is why the agent memory API has no route. A disk-wide index
of every user's file names, on by default, would be a disclosure. With no root the state is `none`,
nothing is reserved and nothing is written. A path outside the roots is refused, and a client names
no path the walk then trusts: tags and ratings are strings under a root, and the walk's own paths come
from the directory entries it read. Every route under `/v1/files` refuses a browser's request, reads
included, because what they answer is the names of a person's files.

**A path is the identity**, spelled one way: absolute, with no empty, `.` or `..` component, no
trailing slash, and valid UTF-8. That spelling is what makes a string comparison a containment test.
A file name that is not text cannot be carried in JSON, and would make every page holding it
unreadable to a strict parser, so the walk counts it as skipped. A rename outside daimon's sight
loses a file's tags: an inode would survive it, but no client asks for one, and agnos's filesystems
do not all have them. What a person gave is kept against the path, so a file that comes back has
its tags back.

**The walk runs on the loop, in slices.** A thread would arm the stdlib's heap lock for every
allocation, for good (ADR-006). A child would take one of agnos's 16 process slots for as long as a
walk lasts, and hold a copy of the heap. A slice stops at 3 ms, or at a count of steps should the
clock stand still, and the loop serves everything else between slices. Pausing on battery is not
stepping. Measured at 41,200 files on tmpfs: 0.69 s to `fresh`, with `/v1/health` at p99 1.21 ms and
max 3.99 ms during it, against 0.23 and 1.88 idle (BENCHMARKS.md). What one slice cannot divide is
one `getdents` run and one read, so a hung network mount would hold the loop. Roots are the operator's.

**Two generations are reserved when the index opens, and a walk fills the one not being served.** A
query never sees half a walk, and a walk never allocates. `tests/files.tcyr` checks that five
re-walks leave `alloc_used` where it was, and that polling every read route does too: an answer is
built in a kept buffer. A generation's size is fixed (`--index-max`, 262,144 records, 65,536 on agnos,
whose memory is real when reserved). A walk that runs out says `truncated`.

**Duplicates compare a whole SHA-256 of the whole content.** crab offers to delete them, so a sample
or a truncated digest, which could call two different files the same, would lose data. A size is
compared first, so only files sharing one are read, and a digest is reused when a file's path, size
and mtime still match.

**The two files fail differently, because they hold different things.** `index` is derived, so one
that fails its checks is dropped and rebuilt, and it is not synced. `meta` is what a person gave, and
nothing rebuilds it: it is a journal, synced per request, where a line that cannot be read costs only
itself, and the file is then not rewritten. It has a size bound of its own. With no caller
authentication, re-tagging one file over and over would otherwise grow it for as long as the disk
lasted, and compacting while serving would hold the loop.

**A query is query-string text** (`untagged=1&under=/home/me`), so one parser reads `GET /v1/files` and
a saved smart folder, and saving a search is keeping its text. An unknown key refuses the query: a
misspelled filter must not answer with every file. The six folders crab's canvas names ship as
queries. The canvas gives their names and no definitions, so "Large & old" (100 MiB, a year) and
"Raw only" (camera raw extensions) are daimon's reading, and saving a query over one replaces it.

**A client tells three kinds of nothing apart**, as crab asks: no connection means daimon is not
running, `none` that it holds no index, and `building` that one is coming.

## Consequences

- crab's item 3, a reach from a GUI client on agnos: HTTP, as now. Answers are paged and bounded
  (8 KB of rows there), so the machine is not held by one write, and every kernel gap crab's filing
  cites closed in agnos 1.57.7. The rate limit applies: 120 requests a minute per client on Linux.
- Nothing watches the disk, so the index is an hour stale at worst. `indexed_at` says how stale it is,
  and a person's re-index does not wait.
- `Power` on agnos is `unknown` and a walk runs, because the kernel has no power source to read (filed
  with agnos, 2026-11-06).
- Item 5 needs a real embedding, and a query cannot rank by meaning on what RAG has today. It is on
  the roadmap. Whatever is built there should share this index with mneme, as the filing asks.
