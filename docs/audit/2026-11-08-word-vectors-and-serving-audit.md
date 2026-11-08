# Security Audit — 2026-11-08: word vectors and serving hardening (2.5.6)

**Scope**: what 2.5.6 adds or changes: `src/files_vectors.cyr` (a binary table daimon parses, and a builder that reads text),
the vector stage in `src/files_search.cyr`, `index` format 3, `daimon_write_all` and `http_send_response`, `_fx_open_read`,
`--mcp-callback-host`, the relayed tool error, and the detaching of nein's live readers.
**Method**:
- Internal review of the code as written, then a test for each finding.
- Mutation: each new guard was broken in a scratch copy and the suite that should notice was run. 19 of 19 were caught
  against the code as shipped. One more showed a check that could not fail, and the check was removed (below).
- Behaviour measured: `tests/files_vectors.tcyr`, `tests/mcp_builtin.tcyr`, `tests/http.tcyr` over real TCP, `tests/smoke.sh`
  against the linked binary, `fuzz/files_vectors.fcyr`, the AGNOS guest test on the released kernel, and real GloVe vectors.
- External research: model-file parsers, symlink races, URL parser disagreement. Citations are with the findings.

Findings are numbered WV- (word vectors), and SV- (serving). Each states what was found and what became of it. The
residuals at the end are what remains open.

---

## Findings fixed in this release, before it shipped

### SV-1: The callback allow-list and the HTTP client read a URL differently (HIGH)

The first version of `_mcp_callback_host_ok` worked out the host itself, taking what follows the last `@`. sandhi, which makes
the connection, has no userinfo: its host runs to the first `:`, `/` or `?`, and what follows a port's digits is ignored. So
`http://evil.test:80@127.0.0.1/` passed a list holding `127.0.0.1`, and daimon would have connected to `evil.test:80`.
This is the known way past such a list: [CVE-2025-68458](https://corgea.com/advisories/vulnerabilities/CVE-2025-68458) (webpack,
userinfo) and [CVE-2026-25960](https://corgea.com/advisories/vulnerabilities/CVE-2026-25960) (vLLM, a validator and a client that
disagree). **CWE**: [CWE-918](https://cwe.mitre.org/data/definitions/918.html). **Found by** reading sandhi's parser after the
research, with a test that had asserted the wrong thing (`http://user:pw@127.0.0.1/` as allowed).
**Fixed**: the check asks `sandhi_url_parse_a`, the parser the forward uses. `tests/mcp.tcyr` has the bypass and its variants.

### WV-1: A small index never matched by meaning (MEDIUM, correctness)

A file had to stand two deviations above the question's mean. The most a sample of n can stand above its own mean is
(n − 1)/√n deviations, so among five files nothing ever would. Below 32 files only the similarity floor applies.
**Found by** `tests/files_vectors.tcyr` on its first run.

### WV-2: A weak similarity would have been called close (LOW, correctness)

Two deviations above a mean below zero is not close, and `why` would have said *close in meaning (0.08)*. Seen against real
GloVe vectors. A file must be at least 0.25 similar.

### SV-2: One large write was not bounded by its timeout (MEDIUM)

`SO_SNDTIMEO` bounds a single write on TCP, but on an AF_UNIX stream a large blocking write stayed in the kernel for over 40 s
while a test's reader took a trickle of it: the timer restarts for each buffer. daimon serves TCP, so this was measured on a
test's socket and not on the listener, but nothing should rest on the difference. One write now offers at most 64 KB on Linux.

### A check that could not fail

`_fx_carries` compared the served generation's table with today's. `_fx_load` already refuses an index from another table,
and a table cannot change while daimon runs, so the comparison could never be false. Mutation found it. It was removed.

---

## Hardening the release was for

### WV-3: The table is a file daimon parses

Model-file parsers are a known way in, and the failures are size arithmetic on counts read from the file:
[CVE-2026-33298](https://osv.dev/vulnerability/CVE-2026-33298), [CVE-2024-21836](https://vuln.today/cve/CVE-2024-21836),
[CVE-2024-21825](https://vuln.today/cve/CVE-2024-21825) and [CVE-2025-53630](https://vuln.today/cve/CVE-2025-53630) (llama.cpp GGUF,
integer overflow to a short allocation). `files_vectors_open`:
- bounds `dims`, `words` and `slots` before multiplying any of them, and refuses a body over a bound of its own;
- reads exactly the body's bytes, and refuses a file with more;
- checks every slot and weight **whatever the digest says**. A digest proves the file is the one written, not that it is well
  made, and anyone who can write the table can write a matching digest;
- knows a probe ends before running one: as many slots used as there are words, and more slots than words.

`fuzz/files_vectors.fcyr` runs 400 corrupted tables, most under a digest they pass, and probes every one that opens. A table is
never executable and holds no pointer, so what a bad table can do, once it has passed these, is misrank files.

### SV-3: A symlink swapped in after the walk (2.5.4 audit R-3, closed on Linux)

`O_NOFOLLOW` covers a file's last component, so a directory replaced by a link between the walk and the read could have a file
outside a root read for its digest or its words: the same race as [CVE-2026-29518](https://git.almalinux.org/rpms/rsync/commit/cc69bbdd82a094cf40114d5b21f1bd6a9e6e6613)
(rsync, a parent directory swapped for a symlink). `_fx_open_read` opens through
[`openat2`](https://man.linuxreviews.org/man2/openat2.2.html) beneath a descriptor for the root, with `RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS`, so a link below a root fails and a link above one, which is the operator's, does not. rsync's fix
found the same trap: refusing every symlink breaks legitimate ones. A kernel older than 5.6 falls back to `O_NOFOLLOW`.

### SV-4: A client reading a trickle held the loop

`sandhi_server_send_response` has no bound on a whole answer, and each write that makes progress restarts `SO_SNDTIMEO`. An
answer is now written by `daimon_write_all` with a 10 s deadline on Linux, measured over real TCP against a reader taking
256 bytes every 5 ms. Same bytes as sandhi's, which `tests/http.tcyr` compares.

### SV-5: The relayed tool error

bote's consumer audit (2026-10-15) found the relayed error envelope using `"type":"text/plain"`, which MCP does not define, and
reported the branch dead: sandhi counts a JSON-RPC error as a failed call, so `sandhi_rpc_ok` is 0 and every such answer became a
502. Both were right. (This audit's first draft concluded the branch was live, from reading `sandhi_rpc_ok` and not what sets
its error kind. A smoke check that drove it said otherwise.) So through 2.5.5 what the API guide promised, `200` with
`"isError":true`, never happened, and a resource or prompt read lost the server's code. The JSON-RPC error is now looked for
first on both paths, the block is `"text"`, and `tests/smoke.sh` drives each. Running that path for the first time showed
one more defect: sandhi's accessor answers the value as it stands in the JSON, so escaping it again doubled every backslash.
The message is now read with the typed parser. The non-standard `code` stays, documented, until 3.0.0.

### SV-6: nein's live readers on the loop

Each waits on `nft` for up to 60 s. When an operator has opened them they now run in a child, with `nein.live.read` audited
first, because a child's memory is a copy. A refused one still answers on the loop, where its audit entry belongs.

### The frame arena (2.5.1 lead, measured and closed)

The stdlib's growable arena keeps a new chunk for an allocation larger than the next one. Frames are capped at 64 KB, so that
stops at a high-water mark: after frames of rising size to the cap, 200 of the largest grow daimon by 0 bytes
(`tests/ipc.tcyr`). Nothing to change.

---

## Residuals

- **Any local process can search by meaning**, as by words, until caller identity (2.6.x). A vector built from content is the
  same oracle as a posting: `--index-content` remains the opt-in for it.
- **A table is the operator's to trust.** It is not signed. Whoever can replace it can change what files rank where, and read
  nothing. Keep it where only the operator can write.
- **With no `--mcp-callback-host`, any http(s) host is allowed**, loopback and private addresses included. Refusing them by
  default would break the servers on loopback that are the main use, so that waits on 2.6.x.
- **A name on the list is a name.** It says nothing of what the name resolves to, so list addresses where that matters.
- **The deadline bounds a slow reader, and does not remove it**: for up to 10 s, the loop waits on that client. Roadmap 2.8.x.
- **agnos has no `openat2`**, so SV-3 stays open there (roadmap 2.7.x).
