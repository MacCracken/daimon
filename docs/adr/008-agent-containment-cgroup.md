# ADR-008: A cgroup per agent, where daimon may make one

**Status**: Accepted
**Date**: 2026-09-23 (2.4.2)
**Context**: Since 2.3.4 each agent leads its own process group, and a stop, pause or resume signals
the group. A process that calls `setsid` (or `setpgid`) leaves the group. So a stop does not reach it,
and it keeps running after its agent is gone. Measured on 2.4.1: an agent's `setsid sleep` was still
running after the stop answered, with status 5. The 2.3.4 audit recorded this as a residual "as with
any process-group supervisor", and noted that only a cgroup would hold it.

## Decision

Where daimon's own cgroup (cgroup v2) is its to divide, each start of an agent runs in a cgroup of
its own:

    /sys/fs/cgroup<daimon's cgroup>/daimon-<pid>/agent-<id>-<run>

- **The agent moves itself in**, between fork and exec, by writing `0` to its `cgroup.procs`. So
  nothing it starts is ever outside it. The parent then checks that the agent is there, from
  `/proc/<pid>/cgroup`, and the agent's JSON says so: `"contained":true`. The check does not read
  the cgroup's `cgroup.procs`, which lists live processes only. An agent that had already exited
  would be taken for one that never entered, and what it started would be out of reach (found in
  2.4.2's review).
- **A stop, pause or resume signals every process in the cgroup**, as well as the group.
  SIGKILL is `cgroup.kill` (Linux 5.14): one write, atomic, and it reaches a process forked meanwhile.
  Before 5.14, it is one kill per member of `cgroup.procs`.
- **An agent's processes end with it.** When its own process is collected (it exited, or a stop ended
  it), whatever is still in its cgroup gets what a stop gives: SIGTERM, `AGENT_STOP_GRACE_MS`, then
  `cgroup.kill`. The cgroup is then removed. This is systemd's `KillMode=control-group`.
- **A daimon that died leaves its `daimon-<pid>` behind.** Its agents died with it
  (`PR_SET_PDEATHSIG`), but what they had started did not. The next daimon started in the same cgroup
  ends what is in any `daimon-<pid>` whose pid is dead, and removes it.
- **Where daimon cannot make cgroups, agents run as before**, in their process group. That covers a
  root-owned cgroup such as a login session's scope, cgroup v1, and agnos. It also covers a cgroup
  whose `cgroup.procs` daimon may not write: moving a process needs that for the cgroup it leaves
  (the common ancestor), not only a `mkdir`. It is warned and audited
  once at startup (`agent.cgroup.unavailable`), and each agent's JSON says `"contained":false`. A
  per-agent failure is audited as `agent.cgroup.fail`, and that agent runs uncontained.

Where daimon's cgroup is its own: a systemd service with `Delegate=yes`, anything the user's systemd
runs (`systemd-run --user`, a user service), and a cgroup made for it and handed to its user (CI does
this with sudo).

## Alternatives rejected

- **Walk `/proc` for the agent's descendants at a stop** (by parent pid). A double fork breaks the
  chain: the grandchild's parent becomes init. It is also racy against a process forking during the
  walk.
- **`PR_SET_CHILD_SUBREAPER` on daimon.** Orphans would come back to daimon rather than init, but
  nothing would say whose they were, and a child that is not orphaned would still be out of reach.
- **Refuse to start agents where no cgroup can be made** (fail closed, as a failed rlimit refuses a
  start, VULN-010). An rlimit bounds what the agent may do. A cgroup is about reaching what it
  started. Refusing would stop daimon working in every login shell and undelegated service, for a P3
  gap. The JSON says which case applies, so an operator who needs containment can see it.
- **`cgroup.freeze` for pause and resume.** It is cleaner than SIGSTOP. But it changes what a paused
  agent sees, and pause/resume semantics are not what this decision is about.

## Consequences

- A stop reaches everything an agent started, however it detached (`setsid`, a double fork, a
  daemon). Measured with `tests/containment.sh`: the `setsid sleep` is gone after the stop. A process
  that ignores SIGTERM and is left behind by an exiting agent is ended once the grace period is over.
  What an agent left when daimon was killed outright is ended by the next daimon.
- **Behaviour change**: an agent that exits no longer leaves processes running where daimon can
  contain it. Anything it started ends with it.
- **Not a boundary against the agent.** An agent runs as daimon's user, so it may write its own
  processes into any cgroup that user may write, daimon's own included. A process moved that way is
  out of reach, as it would be from systemd's `KillMode=control-group`. Holding an agent that acts
  against daimon needs another user or a namespace, which is not this decision.
- A contained start costs 0.25 ms more than one that is not: `agent_spawn_reap_contained`
  1.616 ms against `agent_spawn_reap` 1.362 ms (medians of five runs, same process, under
  `systemd-run --user --scope`, at a load average under 2). That is the `mkdir`, the move, the check
  and the `rmdir`. Each stop also reads `cgroup.procs`.
- An agent can create cgroups inside its own. Their processes are ended with the rest
  (`cgroup.kill` is recursive). Once nothing runs in the cgroup or below it (`cgroup.events`), daimon
  removes them, deepest first, 16 levels down. That happens when the agent's process is collected, or
  later from the removal queue. The walk allocates, and the queue runs on every tick, so the queue
  tries it at most three times for each cgroup. The queue holds at most 256 entries. Past that, a
  cgroup's members get `cgroup.kill` at once, and it is not waited for.
- On agnos nothing changes: agnos has no cgroups and no process groups. Reaching an agent's
  descendants there is part of the 2026-09-23 agnos filing on ending a process.
