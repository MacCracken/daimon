# Security Audit — 2026-09-23: daimon on AGNOS (2.4.0)

**Scope**: daimon 2.4.0 running on the agnos kernel 1.57.5. That means its agent lifecycle
(`spawn_path`, `kill`, `waitpid`), its agent channels (`chan_op`), its HTTP API on its own polled loop,
and what the agnos kernel does and does not provide underneath.
**Method**:
- Every kernel claim was read in the agnos source (`kernel/core/syscall.cyr`, `net_tcp.cyr`,
  `net.cyr`, `proc.cyr`, `vfs.cyr`, `arch/x86_64/pic.cyr`).
- Every behavioural claim was measured under QEMU: daimon's guest test (`tests/agnos/run.sh`) and a
  minimal socket server in the guest.
- The agnos-side gaps are filed in the agnos repo, `docs/development/issues/2026-09-23-*.md`
  (ADR-007).

Findings are numbered AG-1 onward. Severity is for daimon on agnos; each finding gives daimon's interim.

---

### AG-1: Any process can read, write or close any TCP connection (HIGH) — agnos filing, daimon cannot mitigate

`sock_send`#48, `sock_recv`#49 and `sock_close`#50 take a connection id (0–7, one table per machine)
and check only its range. Nothing records who owns a connection. An agent daimon starts on agnos can
therefore:
- read requests arriving at daimon's API (`#49` drains the ring before daimon sees it);
- inject answers (`#48`);
- close daimon's listener (`#50` on the LISTEN slot, which also reaps its pending connections).

**CWE**: [CWE-284](https://cwe.mitre.org/data/definitions/284.html), Improper Access Control.
**Filed**: `2026-09-23-socket-ids-have-no-owner.md` (the ask: an owner and epoch per connection,
checked on every call, as `chan_auth` does for channels).
**daimon**: documented exposure. Until agnos fixes it, anything an agent can do on the box includes
interfering with daimon's API.

### AG-2: The API cannot be loopback-only on agnos (MEDIUM) — agnos filing; daimon warns and audits

`sock_listen`#56 takes a port and no address, so daimon's default 127.0.0.1 binding (VULN-011) cannot
hold there. The listener serves the NIC's address. TCP to 127.0.0.1 is also dropped
(`net_handle_tcp`'s `dst_ip != net_ip` check). Measured: 17/17 checks against the guest's own
address, and every request failing against 127.0.0.1.
**Filed**: `2026-09-23-tcp-server-cannot-be-loopback-only.md`.
**daimon**: `serve` warns and audits `http.listen.not_loopback`. The Host check (VULN-012) still refuses
requests that do not name a loopback host, which stops browsers and DNS rebinding. It does not stop a
network client that sends `Host: 127.0.0.1`. Today inbound SYNs are dropped (AG-8), so the API is not
reachable from the network at all. That changes when AG-8 is fixed.

### AG-3: Agents run without memory or CPU limits (MEDIUM) — agnos filing; operator ruling

There is no rlimit equivalent on agnos: `grep -ci rlimit` over the kernel finds 0. VULN-010's
guarantee (a start whose limits fail is refused) cannot hold there.
**Filed**: `2026-09-23-no-per-process-resource-limits.md`.
**daimon**: by the operator's ruling (2026-09-23), agents start without limits on agnos. Every start
is audited `agent.limits.unenforced`, and the agent's JSON says `"limits_enforced":false`.

### AG-4: A stop can only ask (MEDIUM) — agnos filing

`kill`#16 sets a pending bit. No signal has a default action, so SIGKILL does not end a process that
does not read its signalfd. A hung or hostile agent keeps running after its stop, and keeps its slot
of the 16-entry process table.
**Filed**: `2026-09-23-parent-cannot-end-stop-or-continue-a-child.md`.
**daimon**: the stop sends SIGTERM, waits the grace period, sends SIGKILL (a no-op there), gives up and
audits `agent.stop.pending`. The agent stays *Stopping*, and a later reap collects it if it ever
exits. Pause and resume answer 501.

### AG-5: Every agent inherits every descriptor daimon holds (MEDIUM) — agnos filing; capture refused

`spawn_path` copies the parent's whole fd table into the child, and there is no close-on-exec. This is
VULN-013's class. Channel fds are safe, because an inherited channel is inert by construction
(`chan_auth`). Files and pipes are not.
**Filed**: `2026-09-23-child-inherits-every-fd-and-spawn-arms-leak.md`.
**daimon**:
- `serve --agent-output capture` is refused on agnos. Capture would put every agent's output pipe in
  every later agent, which could then read or forge another agent's output.
- Without capture, the kernel fds daimon holds at a spawn are its console (0–2), its channel ends and,
  since 2.4.1, the read end of the pipe of each detached call under way (AG-13).
  Its listener and connections are the cyrius peer's user-space handles, not kernel descriptors, and
  every file it opens is opened and closed within one call (`src/` checked: no file descriptor is kept
  across calls).

### AG-6: A failed `spawn_path` leaves the channel endowment armed (LOW) — agnos filing; daimon closes the channel

`CH_ENDOW` is a per-CPU one-shot that `#43`'s failure path does not clear. Left armed, it would place
daimon's endpoint into whatever that CPU spawns next.
**daimon**: on a failed spawn it closes both ends of the channel. `chan_place_into_child` then refuses
the closed endpoint.

### AG-7: `sleep_ms` holds the CPU (LOW, availability) — agnos filing; daimon yields

`sleep_ms`#41 disables preemption for the whole sleep. Measured in the guest: waiting with it, the
agents a stop was waiting for did not run until the stop gave up.
**daimon**: every wait of its own is `daimon_yield_ms` (`pause`#14 until the deadline).

### AG-8: Inbound connections are mostly never accepted (HIGH, availability) — agnos filing

The passive open refuses a SYN in interrupt context (`net_tcp.cyr:768`), and the RX ring is drained in
interrupt context by the timer tick and the NIC MSI. Measured:
- a minimal server that waits between polls: 0/12 connections accepted;
- a busy-polling one: 2/12;
- a busy-polling one with MSI-X off: 9/12;
- daimon: 8/20.

**Filed**: `2026-09-23-inbound-tcp-syn-dropped-by-isr-drain.md`.
**daimon**: nothing to do in ring 3. Its own loop polls, and yields between passes so its agents can
run. Clients on the same box reach it through the loopback queue.

### AG-9: The receive path never reports EOF after the peer's FIN (LOW) — agnos filing

**Filed**: `2026-09-23-sock-recv-never-reports-eof-after-peer-fin.md`.
**daimon**: its server does not depend on EOF. It reads a request to its own framing and closes after
answering.

### AG-10: An agent's command line cannot carry a space (LOW) — agnos filing; 422

**Filed**: `2026-09-23-spawn-path-args-cannot-contain-spaces.md`.
**daimon**: an agent whose name has a space, or whose command line is over 127 bytes, is refused (422)
rather than started with a split name.

### AG-11: A failed `spawn_path` gives no reason (LOW, observability) — agnos filing (2.4.1)

`#43` answers -1 for every failure, from a missing executable to a full process table. `proclist`#99
cannot tell them apart either: agnos has no zombie state, so an exited, unreaped process holds its
slot without being listed.
**Filed**: `2026-09-23-spawn-path-failure-gives-no-reason.md`.
**daimon**: a failed spawn answers 500, "its process table is full, or it cannot load the
executable", and is audited `agent.spawn.fail`. A full *channel* table is distinct (`-CH_E_FULL`):
daimon answers it with 503 and leaves the agent as it was. Measured: 12 agents run at once in the
guest, where 4 of the 16 process slots are already taken.

### AG-12: A refused TSC calibration stopped daimon's clock (MEDIUM, availability) — fixed in daimon; agnos and cyrius filings (2.4.1)

cyrius's `clock_now_ms` on agnos reads `uptime_us`#95. The kernel calibrates that once at boot, and
refuses a result outside 100–10000 cycles per µs. After a refusal, #95 answers -1 for the whole boot
and `clock_now_ms` returns 0 forever. Through 2.4.0 every deadline in daimon read `clock_now_ms`, so
none would ever pass:
- a connection's read deadline, so a silent client would hold its slot, and 5 of them the whole
  server;
- deferred answers;
- a stop's SIGKILL escalation;
- `daimon_yield_ms`, which would spin forever.

Measured: a guest booted with QEMU held to 25% of a CPU logged `tsc: calibration REFUSED`, and hung
in its first 10 ms wait.
**Filed**: agnos `2026-09-23-tsc-calibration-refused-stops-the-us-clock.md`; cyrius
`2026-09-23-daimon-agnos-clock-stands-still-when-tsc-calibration-refused.md`.
**daimon**: `daimon_now_ms` (`src/syscalls.cyr`) reads #95, and after a -1 reads `uptime_ms`#40, the
100 Hz tick. Every deadline in daimon goes through it. Measured: with the calibration refused, the
guest test passes. The vendored libraries' own reads of `clock_now_ms` are not covered, but daimon's
loop on agnos does not run sandhi's.

### AG-13: A call that waited on another server ran on the loop (MEDIUM, availability) — fixed in daimon (2.4.1)

On Linux, an MCP forward and the builtins that wait on another server (`web_fetch`, `web_search`)
run in a child process (`server_detach`, 2.3.4), and the loop keeps serving. Through 2.4.0 they ran
in place on agnos, because `server_detach` had no agnos arm. 2.4.0 put that down to agnos having no
fork. It has one: `fork`#96, since 1.56.54. So a slow upstream held every other client, every agent
channel and every stop. The peer bounds each receive at 30 s by the RTC, so that could be up to 30 s
for each receive that got nothing. External MCP registration is unauthenticated (2.5.x), so any local
client could point daimon at such an upstream.
**Fixed** (2.4.1): `server_detach` forks on agnos too. Measured in the guest: with a forwarded call
waiting on its server, `GET /v1/health` is answered in 2 ms, and a 9 KB answer comes back whole
through the child's pipe. Residuals, each on an agnos filing:
- a child past the deadline cannot be killed (AG-4). The client gets its 504, and the child runs until
  its upstream gives up. At most 4 children are alive at once, and one more call answers 503;
- an agent started while a detached call is under way inherits that call's pipe (AG-5), and could
  read its answer. Today that adds nothing to AG-1, under which any process can already read daimon's
  connections;
- a child's connect to a host that does not answer holds the CPU for up to ~8 s (AG-14).

### AG-14: A send over 2 KB to a local process stops the machine (HIGH, availability) — agnos filing; daimon mitigates (2.4.1)

A TCP connection's receive ring on agnos is 2048 bytes. `sock_send`#48 holds the CPU while it waits
for each chunk's ACK, and sends 1-byte probes into a zero window, each waiting up to ~8 s. The
receiver, a process on the same machine, cannot run to drain its ring. So a single write over
~2 KB to it never finishes, and nothing else runs meanwhile. `sock_connect`#47 holds the CPU the same
way for up to ~8 s.
Measured: a `GET /v1/agents` with a 2389-byte body stopped the guest (150 s, no progress), and so did
a 9 KB answer sent to daimon's detached child.
**Filed**: `2026-09-23-sock-send-and-connect-hold-the-cpu.md`.
**daimon**: on agnos it writes every answer, and every relayed one, in pieces of at most 1 KB, with a
yield between them (`daimon_write_all`). A client that reads keeps up: the same `GET` now arrives
whole. A local client that stops reading can still stop the machine from inside daimon's `#48`, and
only agnos can fix that.

---

## What 2.4.0 fixed in daimon itself

- **`sys_access` is a stub on agnos**, always -1, so no agent executable could ever be found.
  `_agent_runnable` uses stat#33.
- **daimon's blocking waits held the CPU on agnos**: `agent_stop`'s loop and `daimon_reap_within`
  used `sleep_ms`. The blocking stop gave up on an agent that would have exited; measured, it now
  completes with the agent's exit code 0.
- **The channel's frame bound is per target.** On agnos a frame over 4092 bytes is refused
  (NACK_INVALID, channel closed, audited `ipc.frame.oversize`). A longer one would lose its first
  records in the kernel's drop-oldest inbox before daimon read them.

## What 2.4.1 fixed in daimon itself

- **daimon's clock on agnos** (AG-12).
- **CI never verified the committed `cyrius.lock`.** `cyrius deps` rewrote it just before
  `cyrius deps --verify` ran, so the verify step checked a lock written a moment before. CI now
  resolves with `--no-lock`, so the committed lock is what gets verified. The committed lock also
  carried 36 entries that a clean resolution does not vendor: leftovers from a local `lib/`,
  identical to the 6.6.6 stdlib's own files and included by nothing. They are gone.

## Verification

- 2.4.1: the guest test runs in CI on the released agnos 1.57.5 and gnoboot 0.7.2, pinned by SHA-256
  (`tests/agnos/run.sh --release`), with 85 checks. Measured locally, it passes unrestricted, with
  QEMU held to 50% of a CPU, and at 25%, where the kernel refused its TSC calibration.
- The guest test passes 44 + 20 checks on agnos 1.57.5 under QEMU (57 s; 2.4.0).
- The Linux suites, smoke (131) and fuzz are unchanged and green.
- The agnos-specific refusals (pause 501, the 422, capture at startup) and the audits
  (`agent.limits.unenforced`, `http.listen.not_loopback`) are in the code paths the guest test runs.
  The startup refusal and the listen audit are read from the code, not asserted in the guest.
