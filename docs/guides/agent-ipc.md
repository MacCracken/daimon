# Agent IPC — the agent's channel (2.3.3; 2.3.4; AGNOS 2.4.0)

Every agent daimon starts gets a **channel** to daimon: one end of a Unix socketpair, open in the
agent as **file descriptor 3**. daimon announces it in the agent's environment as
`AGNOS_IPC_FD=3`. The agent writes messages to it and reads one reply byte per message.

There is no socket file to find and nothing to connect to. The descriptor is already open when the
agent's program starts, and only that agent's process received it, so daimon knows who is talking.
The source of every message is the agent daimon started on that channel, whatever the message
says.

On AGNOS (2.4.0) the channel is the kernel's own: a `chan_op` pair, one end moved into the agent at
spawn. The frames and replies are the same. See [On AGNOS](#on-agnos) for the three differences.

## The wire format

A **frame** is a 4-byte **big-endian length**, then that many bytes of UTF-8 **JSON**:

```
 byte 0      byte 1      byte 2      byte 3      bytes 4 .. 4+len-1
[len >> 24] [len >> 16] [len >> 8 ] [len & 255] [ the body: len bytes of JSON ]
```

- `len` is at most **65536** (`MAX_MESSAGE_SIZE`).
- `len` = 0 is a **keep-alive**: skipped, with no reply.
- Several frames may be sent in one write, and one frame may arrive over several writes.

### The body

One JSON **object**:

| field | required | meaning |
|---|---|---|
| `target` | yes | a non-empty string of at most 256 bytes: an agent id (`"7"`), a registered name, or `"*"` (or `"broadcast"`, in any case) for every agent |
| `type` | no | `"request"`, `"response"`, `"event"`, `"heartbeat"` or `"command"`. Absent or `null` means `"event"` |
| `payload` | no | any JSON value. daimon keeps it as compact JSON text; absent is `null` |

Other fields are ignored. That includes any field naming a source: daimon takes the source from the
channel.

A body is refused (reply 3) if it is not JSON, has content after the object, is not an object, has a
`target` that is missing, empty, longer than 256 bytes or not a string, or has an unknown `type`.
The same two rules as HTTP request bodies also apply (see the [API guide](api.md#request-bodies)):
- no top-level key may appear twice;
- no top-level string may contain U+0000.

## Replies

daimon answers each frame with **one byte**:

| byte | name | meaning |
|---|---|---|
| 1 | `ACK` | queued for its target (a broadcast: on every queue that had room) |
| 2 | `NACK_QUEUE_FULL` | the target's queue is full, or the bus holds its limit in bytes (a broadcast: every queue was full). Nothing was kept; send it again later |
| 3 | `NACK_INVALID` | the frame was refused (see above). The channel stays open, except for the two faults below |
| 4 | `NACK_NO_TARGET` | no agent has that id or name. Nothing was kept. (Since 2.3.4; 2.3.3 answered ACK and dropped it) |

Since 2.3.4 the reply is given after the message is routed, so it says where it went. In 2.3.3,
`ACK` meant only "accepted for routing".

daimon never waits on an agent. A reply that the agent's socket buffer cannot hold (the agent has
stopped reading them) is dropped.

## What closes a channel

| cause | what the agent sees | audit action |
|---|---|---|
| a length over 65536 | reply 3, then end-of-file | `ipc.frame.oversize` |
| a frame begun but not finished within 5 s | reply 3, then end-of-file | `ipc.frame.timeout` |
| the agent closes its end in the middle of a frame | — (the partial frame is dropped) | `ipc.frame.short` |
| the agent exits, or is stopped or reaped by daimon | — | — |

When daimon stops or reaps an agent, it first reads and delivers every frame the agent had
written, then closes its end. A child process the agent left holding descriptor 3 can no longer
speak as the agent.

Audit entries go to daimon's libro chain (`libro_export` over MCP). They carry the agent's **id**.

## Where messages go

daimon routes a frame onto its **message bus** as soon as its event loop reads it (2.3.4; in 2.3.3
this waited for the next HTTP request):
- a target that is an agent **id** goes to that agent's queue;
- otherwise a **name** goes to the agent registered under it;
- `"*"` goes to the queue of every registered agent, the sender's included (as in the Rust
  original).

An id is tried before a name. A **name** belongs to the first agent registered with it, until that
agent is deleted; a later agent with the same name gets no name route (the Rust original let the
last one take it). A name that is all digits, or `*`, or `broadcast`, never routes.

An agent has a queue from registration (`POST /v1/agents`) until it is deleted, whether or not it
is running. Each queue holds **100** messages, and the bus as a whole at most **64 MiB** of them. A
message held by several queues counts once. Past either limit a message is not queued; that is
counted (`bus_dropped`), and a sender on a channel is told (`NACK_QUEUE_FULL`).

## Reading and sending over HTTP

```
# Take an agent's queued messages, oldest first (they leave the queue). Body optional.
POST /v1/agents/7/messages/take
{"max": 10}
→ {"messages":[{"id":12,"source":"3","target":"7","type":"command","payload":{"k":"v"},"timestamp":1790137973}],
   "count":1,"remaining":0}

# Queue a message for an agent. Its source is "api".
POST /v1/agents/7/messages
{"type":"command","payload":{"k":"v"}}
→ 201 {"id":13} · 404 no such agent · 429 its queue (or the bus) is full
```

Both refuse browsers, like agent control. The API has no authentication until 2.5.x, so any
client that can reach it can read any agent's queue. A message's memory is freed when it is taken,
or when its agent is deleted (VULN-018).

## Metrics

`GET /v1/metrics` adds, since 2.3.3:

| field | counts |
|---|---|
| `ipc_messages` | frames put on the bus |
| `ipc_refused` | frames answered 2, 3 or 4, plus channels closed on a fault |
| `bus_dropped` | messages not queued because a queue, or the bus, was full |
| `ipc_channels` | open channels (2.3.4) |
| `bus_bytes` | bytes of messages the bus holds (2.3.4) |

## Examples

Shell (the frame is 14 bytes: `\016`):

```sh
printf '\000\000\000\016{"target":"*"}' >&3
head -c 1 <&3 | od -v -An -tu1          # 1
```

Python:

```python
import json, os, struct

fd = int(os.environ["AGNOS_IPC_FD"])
body = json.dumps({"target": "*", "type": "event", "payload": {"status": "ready"}}).encode()
os.write(fd, struct.pack(">I", len(body)) + body)
reply = os.read(fd, 1)[0]    # 1 ACK, 2 NACK_QUEUE_FULL, 3 NACK_INVALID, 4 NACK_NO_TARGET
```

## On AGNOS

The channel is a `chan_op`#97 pair. daimon mints it (`CH_MINT`) and moves one end into the agent as it
spawns (`CH_ENDOW`). **Three things differ from Linux:**

1. **The fd is not 3.** The kernel chooses it. Read `AGNOS_IPC_FD` from the environment; an agent
   written that way works on both.
2. **Bytes travel in records of at most 64 bytes.** Write a frame with `CH_SEND` in chunks of up to
   64 bytes, and read the reply with `CH_RECV`. daimon joins the records back into the same stream.
   An agent that uses plain `write` / `read` on the fd works too: the kernel routes them to the
   channel.
3. **A frame is at most 4092 bytes**, and an agent should wait for each reply before its next frame.
   An inbox holds 64 records (4096 bytes) and drops its **oldest** when full, so a longer burst
   would lose its first bytes before daimon read them. A declared length over 4092 is answered 3 and
   closes the channel, as an oversize frame does on Linux.

A reply is one record of one byte. When daimon closes the channel (the agent is reaped), the agent's
next `CH_RECV` answers `CH_E_PEERGONE`.

The machine has 16 channels in all, one for each running agent and any other program's. When none is
free, a start answers **503** and the agent stays as it was (2.4.1). A reaped agent's channel is free
again at once.

## Limits

| limit | value | constant (`src/ipc.cyr`) |
|---|---|---|
| frame body | 65536 bytes (AGNOS: 4092) | `IPC_FRAME_MAX` |
| target | 256 bytes | `IPC_TARGET_MAX` |
| time to finish a begun frame | 5000 ms | `IPC_FRAME_TIMEOUT_MS` |
| frames one channel may complete per pass of the loop | 16 | `IPC_FRAMES_PER_PASS` |
| messages per bus queue | 100 | `IPC_BUS_QUEUE_MAX` |
| bytes the bus holds | 64 MiB | `IPC_BUS_BYTES_MAX` |

Why the channel works this way is in [ADR-005](../adr/005-agent-channels.md), and why daimon
reads it on its own event loop in [ADR-006](../adr/006-own-event-loop.md).
