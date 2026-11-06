# Security Audit — 2026-11-06: tool definition pins and the file index (2.5.4)

**Scope**: what 2.5.4 adds: `src/mcp_pin.cyr`, `src/files.cyr`, `src/api_files.cyr`, the routes under
`/v1/files` and `/v1/mcp/tools/{name}/accept`, and the two data files each keeps.
**Method**:
- Internal review of the code as written, then a test for each finding, run against the fix and
  (where it says so) against the code before it.
- Every behavioural claim was measured: `tests/mcp_pin.tcyr`, `tests/files.tcyr`, `tests/smoke.sh`
  against the linked binary, `fuzz/tool_pin.fcyr`, and the AGNOS guest test on the released kernel.
- External research for the attack surface: MCP definition swaps, indexers, link following.

Findings are numbered TP- (tool pins) and FX- (the file index). Each states what was found and what
became of it. The residuals at the end are what remains open.

---

## Findings fixed in this release, before it shipped

### TP-1: An empty store read as no store, resetting every pin (HIGH)

`_pin_read` answered 0 both for "no file" and for a file of no bytes. Truncating
`<data_dir>/mcp/toolpins` therefore made it `new`: every pin a first sight, and the next write over
the evidence. It is the hole the filing reports in thoth, reopened in the fix for it.
**CWE**: [CWE-354](https://cwe.mitre.org/data/definitions/354.html), Improper Validation of Integrity
Check Value. **Found by** `tests/mcp_pin.tcyr` on its first run.
**Fixed**: absent is its own answer, and an empty file fails `_pin_load` like any other.

### FX-1: A snapshot record's offset could wrap its bounds check (HIGH)

`_fx_load_check` tested `off + plen + 1 > used`, with `off` read from the file. An offset near 2⁶³
wraps negative and passes, and the next read of the path is a wild pointer. The digest is no defence:
`index` is written by whoever can write the directory, and its SHA-256 is unkeyed.
**CWE**: [CWE-190](https://cwe.mitre.org/data/definitions/190.html), Integer Overflow.
**Found by** review, after the first test (an offset of 2⁴⁰) passed on code that was still wrong.
**Fixed**: `off > used` is tested first, for the path and the digest. The test now uses
`0x7FFFFFFFFFFFFFFF`.

### FX-2: The journal had no size bound (MEDIUM)

`FILES_META_MAX` caps files, but re-tagging one file appends a line each time. With 256 paths a request
and rows of up to 1 KB, one unauthenticated local caller could write about 34 MB a minute for as long
as the disk lasted.
**CWE**: [CWE-400](https://cwe.mitre.org/data/definitions/400.html), Uncontrolled Resource Consumption.
**Fixed**: `meta` stops at 64 MiB. An edit past that answers 409 and is audited `files.meta.full`, and a
restart compacts the file. Nothing compacts while serving: it would rewrite the file on the loop.

### FX-3: A file name that is not text would poison every page holding it (MEDIUM)

A name on Linux may hold any byte, and JSON cannot carry them. One such file would make each page
that includes it unreadable to a strict client, so anyone able to create a file under a root could
blind the client to the rest of the page.
**Fixed**: `files_utf8_ok` is strict (RFC 3629: no overlong form, surrogate or code point past
U+10FFFF), the walk counts such a name as skipped, and `files_path_ok` refuses one, which also refuses
an overlong `/`, the classic filter bypass ([CWE-176](https://cwe.mitre.org/data/definitions/176.html)).

### FX-4: The temp file followed a link (LOW)

`_fx_save_start` opened `index.tmp` with no `O_NOFOLLOW`. In a 0700 directory nobody else can plant
one, but nothing relied on that. **Fixed**.

---

## Checked, and found sound

- **Path containment.** A string prefix alone would pass `/home/ab` for the root `/home/a`, and `..`
  would walk out of a root. `files_under` tests the boundary, and `files_path_ok` allows one spelling
  of a path. ([CWE-22](https://cwe.mitre.org/data/definitions/22.html). CVE-2026-46724 is this class in
  a CMS's file indexer, which did not normalise its configured path.)
- **Link following.** The walk uses `lstat` and records no symlink, so a link inside a root does not
  lead out of it. Digests open with `O_NOFOLLOW`. ([CWE-61](https://cwe.mitre.org/data/definitions/61.html).
  CVE-2025-11489, in an MCP file tool, is this class.) `O_NOFOLLOW` is spelled by symbol: its value
  differs on aarch64, where x86's number means `O_LARGEFILE`.
- **A FIFO.** The walk records only regular files, and a digest opens `O_NONBLOCK`, so a FIFO swapped
  in after the `lstat` cannot hold the loop. The test tree holds a real one.
- **A forged `index` or store.** Every record's offsets, and every row's fields, are checked after the
  digest passes, and a store is refused whole on any one bad row. Each shape has a test, and
  `fuzz/tool_pin.fcyr` feeds `_pin_load` noise with and without a passing digest.
- **A root removed by the operator.** An `index` holding anything outside today's roots is dropped at
  once: a file must not stay served from a root that is gone.
- **Browsers.** Every route under `/v1/files` refuses a request carrying `Origin`, reads included,
  beyond what `handle_request` does for state changes. Binding loopback and the Host check stand.
- **A digest clients can compute.** `mcp_definition_sha256` hashes what is served, and bayan's compact
  form was measured to be a fixed point: of 80,000 generated documents and numbers, none that bayan
  accepts moved, and the harness fails if most of them were refused.
  A NUL cannot enter name or description (`http_body_json`), so the separator cannot be forged.

## Residuals

These are open, and each says why.

### R-1: Any local process can read the index and its tags (MEDIUM, by design until 2.6.x)

Nothing tells callers apart. An operator who names a root offers the names, sizes and times of the
files under it, and whether two of them are identical, to every local process, as the user daimon
runs as. If that is root, it crosses every user's permissions under that root. This is why the index
is off until a root is named, and `docs/guides/api.md` and SECURITY.md say so. Identity is roadmap
2.6.x. **CWE**: [CWE-306](https://cwe.mitre.org/data/definitions/306.html).

### R-2: Any local process can edit tags and ratings, and fill their limits (MEDIUM, until 2.6.x)

The same cause. It can also register tools, set a first pin before a real server does (trust on first
use, ADR-009), and leave `absent` pins that take table slots. A full table serves tools `unpinned`
rather than refusing them, so that is visible and costs no service, and `/v1/mcp/tools` says how many.

### R-3: A symlink swapped into a directory on the path, between walk and digest (LOW)

`O_NOFOLLOW` covers the final component only. If a directory on the path is replaced by a link after
the walk, a digest can read a file outside the root. Only a digest of it comes out, and only as
"these two files are duplicates", so what leaks is whether a file outside matches a chosen one of the
same size. `openat2(RESOLVE_NO_SYMLINKS)` would close it on Linux 5.6+, and agnos has no peer.
([CWE-367](https://cwe.mitre.org/data/definitions/367.html).)

### R-4: A hung filesystem holds the loop (LOW, the operator's choice of root)

A slice is bounded between syscalls, not within one. An `lstat`, a `getdents` or a read on a dead
network mount waits there, and the loop with it. Roots are named by the operator, and nothing but a
child could divide this.

### R-5: The stores' digests are unkeyed (accepted)

A process with daimon's uid can rewrite a store and recompute its digest. A MAC key would live beside
the file under the same uid, so it would add nothing. The directory is 0700. thoth's ADR-0022 reaches
the same conclusion for the same reason.

## Sources

- [CVE-2025-54136 (MCPoison)](https://nvd.nist.gov/vuln/detail/CVE-2025-54136): an approved MCP
  configuration swapped after approval, in Cursor. The closest published CVE to the swap the pins
  show. A pure tool-description swap has no CVE of its own that this search found.
- The MCP specification leaves what a client does about a changed definition undefined, so a server
  cannot rely on clients noticing ([rug pull overview](https://testdino.com/blog/mcp-tool-poisoning)).
- [CVE-2026-46724](https://secualive.jp/en/feed/nvd/vulnerability/detailinfo/CVE-2026-46724/): path
  traversal through a file indexer's configured directory.
- [CVE-2025-11489](https://avd.aquasec.com/nvd/2025/cve-2025-11489): improper link resolution in an
  MCP file tool.
