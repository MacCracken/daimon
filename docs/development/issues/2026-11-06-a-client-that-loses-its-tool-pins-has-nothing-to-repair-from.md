# A client that loses its tool pins has nothing to repair trust from

**Status:** 🔴 **OPEN.** A consumer need, not a defect: daimon does what it says. daimon decides the
mechanism; this filing states what thoth has to be able to ask and get back, and the constraints the
answer has to fit.
**Filed:** 2026-11-06, by thoth, against daimon 2.5.3. Line numbers are from that tree.
**Affects:** every daimon consumer that shows registered tools to a model.
**Severity:** daimon's to rank. It holds no thoth release: thoth ships as it is and takes this up when it lands.

## What thoth does today

An MCP tool's description is read by the model as guidance, so swapping one after it was approved is
prompt injection with an approval already granted (the CVE-2025-54136 shape). thoth defends with
trust on first use:

- It pins SHA-256 over `name \0 description \0 inputSchema` at first sight, by host and name.
- An unchanged definition passes silently.
- A changed one is withheld from the model and refused at dispatch until the operator runs `/tools trust`.
- Pins persist in `~/.thoth/toolpins` under a whole-file digest (thoth's ADR-0022).

## The hole

Trust lives only in the client, so when the client loses it there is nowhere to get it back.

thoth refuses a pin store that fails its digest, is a symlink, is unreadable, is oversized or belongs to
another user. The pin table is then empty, every tool is a first sight for that run, and a swapped
definition reaches the model with one red line shown. A deleted store and a new machine are the same
case, with less said.

The maintainer ruled on 2026-11-06 that thoth keeps that behaviour. The alternative was withholding every daimon
tool until the operator re-trusts, and an agent with no tools has lost the thing it is for. So the
client should not close this by going without. It should be able to repair its pins from the place
the definitions come from.

## Why this lands in daimon

thoth's ADR-0022 records this as the end state: pin once, in the spine, for every consumer. At 2.5.3:

- **The registry is in memory,** so whatever a client trusted is gone when daimon restarts.
- **A re-registration overwrites silently.** `mcp_register_external` calls `map_set` on the name
  (`src/mcp.cyr:125`), whether or not the definition changed.
- **A successful registration leaves no record.** `api_mcp_register` audits a rejected callback and a
  shadowed builtin, and nothing when a definition is added or replaced (`src/api_mcp.cyr:11`).
- **Registration takes no caller auth,** which `roadmap.md` already places at 2.6.x.
- **The manifest has no identity to compare.** An entry is name, description and `inputSchema`
  (`src/mcp.cyr:133`), with no digest, no time and no server id.

## What thoth needs

1. **A definition record that outlives a restart,** so a pin held by daimon means the same thing tomorrow.
2. **A digest and a date on every manifest entry,** builtins and externals alike. For example
   `definition_sha256` and `pinned_at`, with the bytes hashed stated exactly, so a client can compute the same value.
3. **A change that is visible,** not an overwrite: an audit event, and a state a client can read, so a
   changed registration is told apart from the one that was approved.
4. **A way to accept a change,** per consumer and gated through t-ron, so re-trusting is a deliberate act
   and not a side effect of re-registering.

## Constraints on the answer

- A client whose store was refused must be able to rebuild its pins from daimon in one call, and must be
  able to tell a rebuilt pin from a first sight.
- The digest must cover the bytes a client receives. If daimon re-serialises `inputSchema`, the two
  digests disagree on an unchanged tool.
- Item 4 means little until callers are identified, and a registration stays unauthenticated until 2.6.x,
  so this probably sequences on or after that work. Anyone who can `POST /v1/mcp/tools` today can
  rewrite what a repair would read.

## What this does not fix

A repair from daimon trusts daimon at that moment. It closes a swap by an MCP server behind daimon, and it does not
close a swap by daimon itself. thoth's own store remains the check on that, so this adds a source of repair
and does not replace the client's pins.

## What thoth will do when it lands

- On a refused store, rebuild from daimon's pins instead of treating every tool as a first sight, and say so.
- Compare its own digest with daimon's on every probe. A mismatch means the two disagree about the
  canonical form, and thoth announces it.
- Keep `~/.thoth/toolpins` as the client-side floor.

## Reproducing the hole

With a daimon serving at least one tool and a thoth that has already pinned it:

1. Corrupt the store, for example by appending a line to `~/.thoth/toolpins`.
2. Re-register that tool with a changed description (`POST /v1/mcp/tools`).
3. Start thoth. The greeting reads `REJECTED`, and the changed definition is advertised to the model.

`/tools trust` cannot help here. With no pins left, thoth has nothing to compare the changed definition
against, so it has no difference to report.
