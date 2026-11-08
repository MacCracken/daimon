# ADR-010: A file index on the loop, off unless the operator names a root

**Status**: Accepted
**Date**: 2026-11-06 (2.5.4)
**Context**: crab, the AGNOS file manager, needs a persisted, disk-wide view of files: an index, tags,
ratings and smart folders ([the filing](../development/issues/archive/2026-09-23-crab-needs-a-file-index-tags-and-ranked-search.md),
items 1–4; the addendum below covers item 5, ranked search). None of it existed. Four things constrain
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

## Addendum — 2.5.5: a question in words

`GET /v1/files?q=paid+invoices`, ranked, with why each file matched, where it appears, facets, duplicates
among the results and what it cost. `src/files_search.cyr`.

**It ranks by words, not by meaning, and every answer says so** (`cost.method`). It finds `invoices` for
`invoice`, and will not find `receipt` for `invoice`. Meaning needs an embedding model, and none exists
to run locally: mneme's embedder is a stub that defers to the same stack, hoosh forwards to Ollama,
and this machine has no model to test one against. A hash of a word's bytes is not an embedding,
which is the filing's complaint about RAG, so daimon did not ship another and call it one. crab prints
"no external service" only if it is true, and it is: no byte leaves daimon. (2.5.6 found the embedding
this paragraph said did not exist: the second addendum.)

**Content is a second opt-in** (`serve --index-content`). `--index-root` offers names. Content offers more:
with no caller authentication, a local process can ask whether a file it could not open holds a word.
An operator who passes `--index-root` has not agreed to that. Without the flag no file is opened for its
words, and a question searches names.

**An inverted index in the generation.** Each posting is one u64, `(term << 32) | (record << 8) | byte`.
They are grouped by the term's top 16 bits with an in-place sort that can stop anywhere, so a query
scans only its words' buckets, and costs what those cost rather than the size of the disk: 3–6 ms at
41,200 files with thousands of hits. Scores and masks are kept per record, and reset through the list of
what a query touched. Like the walk, all of it is reserved at open, and an unchanged file keeps its
postings through a pass over the served generation, so a re-walk reads only what changed. A 32-bit term
can collide, and two words then rank as one: the cost of a wrong rank, not of a wrong file.

**Ranking is BM25 in integers** (k1 1.2, b 0.75), the same on every target, with a name match worth three
times a word's weight, and multiplied by how many of the question's words a file has, so a file with
two ranks above one that says one of them often. `why` is built from what matched: nothing in it is
inferred. A window in words (`last week`) is rolling, as the rest of the query language is.

**A smart folder can be a question** (`q=paid+invoices`), because a query is text. A facet's `query` is
what to add to the question, and `tests/files_search.tcyr` checks each one gives the count it claims.

What it does not do: a PDF or an office file is found by name alone, since content is the first 64 KB of a
file with no NUL in it. Stemming is English. 256 distinct words are kept per file, first seen first.

## Addendum — 2.5.6: by meaning, from a table of word vectors

`serve --index-vectors FILE`. `src/files_vectors.cyr`. crab's item 5 asks for a local embedding, and 2.5.5 said none
existed to run. That was too narrow: it looked only for a model *server*.

**A table of word vectors, not a model server.** A model server is a process to call, so every file would be a socket
round trip in a child, and a question would wait on one. It would also not exist on agnos, where crab lives: nothing
there serves a model. Trained word vectors (GloVe, word2vec, fastText) are an embedding that is just numbers in a file.
A file's vector is arithmetic over a table in memory, in the walk's own slices, so it pauses on battery, allocates
nothing per file, uses no fork and no socket, and runs the same on agnos. `daimon vectors build` turns the text form they
ship in into a compact table, once, on any machine, and the table is portable between targets.

**What this is, and is not.** A file's vector is the weighted sum of the vectors of its distinct words, so it knows what a
file is *about*, and nothing about word order: it is not a sentence model. It is an embedding in the sense the filing
asks for, and not a hash: `receipt` sits near `invoice` because of how the words are used. Measured with GloVe at 100
dimensions: the cosine of `invoice` and `receipt` is 0.639, and of `invoice` and `banana` 0.069.

**The operator supplies the table.** It is data with its own licence, and 8.7 MB at the useful size, so it is not in this
repository. With none, search is exactly what 2.5.5 was and says so. A flag that named a table and got words would be
worse than no flag, so a table that is absent or damaged stops `serve`, in a different message for each.

**A word's weight falls with how common it is, and the common direction is taken out** (Arora, Liang and Ma, *A Simple
but Tough-to-Beat Baseline for Sentence Embeddings*, ICLR 2017): a / (a + p(w)), with p(w) from the word's rank by
Zipf, so `the` counts for little. The table is mean-centred by frequency when it is built. Removing a corpus-wide
direction from the files' own vectors was tried and rejected: at 6,454 real documents it cost `receipt` all three of the
files it should find, and it would have needed a second pass over the whole index.

**Biased int8, compared on the loop.** A word's vector is int8, stored plus 128, so a dot product needs no sign fix per
byte: 26.4 ms with one, 7.7 without, over 41,200 files of 100 dimensions, and 6.8 unrolled by eight. So a question
compares itself with every file, on the loop. A 1-bit prefilter was reasoned through and rejected: at 100 dimensions its
noise is as large as the gap it would have to see.

**A similarity means nothing by itself, so a file must stand out.** Measured on GloVe: the same 0.5 is the best answer one
question has and an average file for another. A file is admitted if it stands two deviations above what this question
scores against the whole index, is at least 0.25 similar, and is among the best 64. Two deviations need a population, as
the most a sample of n can stand above its mean is (n − 1)/√n of them, so below 32 files only the floor applies.

**Words and meaning on one scale**, each worth up to 1000: the best word match, and the closest file. A file with both
comes first, and a strong meaning outranks a weak mention. Without a table a score is what it always was.

**`why` and `cost.method` say which ran.** *"Close in meaning to the question (0.64)"* is built from the similarity, and
never says a file contains a word it does not. `method` is `vector + words + name`, or `vector + name` without
`--index-content`, and `words + name` when no word of the question is in the table, because that is then what ran.

**A generation holds its files' vectors beside its postings**, carried over for an unchanged file the same way, so a
re-walk reads only what changed. `index` is format 3. A file built with another table is not of use to this run, and is
rebuilt; that is a different audit entry from a damaged one (`files.index.vectors.changed`).

**A table is trusted no further than its checks.** A digest proves the file is the one that was written, not that it is
well made, so every slot and weight is checked whatever the digest says, and a probe is known to end before it is ever
run. A model file's parser is a known way in: the GGUF CVEs are in the audit
(`docs/audit/2026-11-08-word-vectors-and-serving-audit.md`).

What it does not do: a file is the first 256 distinct words of its first 64 KB, so a long file is judged by how it opens.
Without `--index-content` a file's vector comes from its name and its folder's. Whether a machine's files have words
the table holds is the table's business: `vectors_files` in the status says how many got a vector.
