# ADR-009: daimon pins what each MCP tool's definition was

**Status**: Accepted
**Date**: 2026-11-06 (2.5.4)
**Context**: An MCP tool's description is read by the model as guidance, so swapping one after it was
approved is prompt injection with an approval already granted. CVE-2025-54136 ("MCPoison", Cursor) is
that shape for an approved configuration. thoth defends with trust on first use: it pins a digest of
each definition and withholds one that changes. That record lives only in the client, so a client
that loses it has nothing to repair from
([the filing](../development/issues/archive/2026-11-06-a-client-that-loses-its-tool-pins-has-nothing-to-repair-from.md)).
Through 2.5.3 daimon could not help: its registry was in memory, a re-registration overwrote silently
and left no record, and a manifest entry had no identity to compare.

## Decision

**daimon keeps a pin for each tool name: the digest of the first definition it saw, and when. A
registration cannot move it.**

- **The digest covers the bytes a client receives**, so a client computes the same value. It is
  SHA-256 over `name \0 description \0 inputSchema`, name and description decoded, inputSchema the text
  the manifest serves. That is thoth's form. The text is bayan's compact one, and bayan rebuilds it to
  the same bytes, so a client that parses and re-serialises it, as thoth does, gets them too.
  `tests/mcp_pin.tcyr` holds vectors computed by Python's hashlib, `tests/smoke.sh` checks the running
  binary against `sha256sum`, and `fuzz/tool_pin.fcyr` draws schemas and numbers for the fixed point.
  A NUL would make the form ambiguous, and `http_body_json` already refuses one in a top-level string.
- **A later registration that differs is served, and marked.** Its entry says `pin_state: changed`,
  with both digests, and it is audited `mcp.tool.changed`. Serving it keeps every registrant working.
  Refusing it would strand a server upgrade, since nothing can accept one until callers are
  identified, and withholding is the client's decision, which it can now make with what it reads.
- **Deregistering keeps the pin.** A tool that vanishes and returns with a new definition is the swap.
- **The registration itself does not persist.** A pin says what a name meant. Restoring registrations
  across a restart would put callback URLs from an unauthenticated route back into service with
  nobody asking, and a server is expected to register again.
- **A builtin's pin follows the binary.** Its definition is the binary's, and installing one is a
  deliberate act, so a new binary moves it, audited `mcp.pin.builtin.moved`.
- **Accepting a change for everyone is refused** (`POST /v1/mcp/tools/{name}/accept` with no token,
  403, audited), as `nein_allow` and `libro_retention` are, because the caller who swapped a definition
  could approve it. `serve --mcp-pin-accept` opens it for an operator on a trusted single-tenant host.
  It is one decision for the host. A consumer accepts for itself: see the addendum.

**The store is defended the way thoth's is, and claims what thoth's claims.** `<data_dir>/mcp/toolpins`
is one JSON line per pin under a whole-file SHA-256. A file that fails any check is refused whole and
left untouched as evidence, and nothing writes over it, including an empty one. A per-line skip, or
starting afresh, would let corrupting the file turn every pin into a first sight. The state
(`loaded`, `new`, `rejected`, `unavailable`) is in every manifest, with `durable`, so a client can see
how far to trust what it reads. The digest catches a torn write, a disk and a stale format. It does
not stop someone who can write the file, which is the service's own uid, in a 0700 directory.

A tool past the limits (1024 pins, a 256-byte name) is **served `unpinned`**, not refused. A pin table
is filled by the same unauthenticated route, and it persists: refusing would let one flood deny every
later tool across restarts.

## What this does not fix

- **Trust on first use.** The first registration of a name is believed. Whoever registers first,
  before the real server, sets the pin. Identity (2.6.x) is what could say who that was.
- **A swap by daimon itself.** A repair trusts daimon at that moment. A client's own pins stay the
  check on that, which is why this adds a source of repair and does not replace them.

## Consequences

- A client can rebuild from one `GET /v1/mcp/tools`, and tell a repaired pin from a first sight:
  `pin_from_store` says the pin came from a previous run.
- Writing the store is one `fsync` on the loop per first sight, bounded by the table's size. A flush
  rebuilds the file in a kept buffer from rows kept on each pin, so it allocates nothing per row.
- On agnos a store needs `rename#31` to replace a file, which ext2 does since agnos 1.57.17. On an
  older kernel the second write fails, and the state says `unavailable`. daimon does not work around
  it with an unlink: that would give up the crash atomicity the record exists to have.

## Addendum — 2.5.5: a consumer accepts for itself

The filing's item 4 asks for acceptance per consumer, gated through t-ron. That needs a caller to be
known, so 2.5.5 brings a small, real piece of identity, for this one route.

- **A consumer is a name with a token, which the operator issues**: `daimon consumer add thoth`. The
  token is 256 random bits, shown once, and only its SHA-256 is kept, in `<data_dir>/mcp/consumers`. So
  reading the file gives nobody a way in. It is read for each use, so a removal takes effect at once.
- **An accept is that consumer's own record, and moves no pin.** The same tool reads `accepted` for it
  and `changed` for everyone else, because one client's approval is no approval of another's. The
  shared pin stays what a repair reads.
- **The consumer must name the digest it reviewed** (`definition_sha256`). A swap landing between
  review and accept answers 409, instead of being approved unread.
- **A token that names nobody is refused, never read as no token** (401), or a mistyped one would
  quietly give the anonymous answer.
- **The file's absence and its refusal are different answers.** Taken for empty, a symlinked or
  oversized file would let adding one consumer erase all the others.

- **Every accept passes t-ron** (`_pin_gate`), and only an `ALLOW` passes: a `FLAG` is not an accept. With no
  policy loaded (`serve --tron-policy FILE`) no consumer may accept, and t-ron denies by default an agent or
  a tool its policy does not name. The action it judges is `mcp_accept.<tool>`, so a policy says
  `allow = ["mcp_accept.*"]`, per consumer, with deny winning. A policy t-ron refuses stops `serve`: an
  operator who named one did not ask for none.
- **t-ron reads what is being accepted.** The definition is `tron_check`'s parameters, so its scanner sees
  SQL, shell, template and path injection. A swapped description is more often a prompt injection, which
  that scanner does not look for, so `injection_detector_check` reads the description too. A definition
  that fails either cannot be accepted by a consumer: the operator's `--mcp-pin-accept` is what is left for a
  false positive, and is a decision for the whole host, made by someone who read it.

t-ron could not be linked until 2.2.5: t-ron and nein both defined `agent_policy_new` and
`policy_engine_new`, in one global scope. The first was a compile error by arity, and the second, alone, would
have put nein's vec where `policy_check` reads a hashmap. daimon pins t-ron 2.2.5, which renames its two.

This is not 2.6.x's caller identity. A token means something on this route and in a manifest, and every
other route still answers whoever reaches it.
