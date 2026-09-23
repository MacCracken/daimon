# Agent IPC — the agent's channel (2.3.3)

Every agent daimon starts gets a **channel** to daimon: one end of a Unix socketpair, open in the
agent as **file descriptor 3**. daimon announces it in the agent's environment as
`AGNOS_IPC_FD=3`. The agent writes messages to it and reads one reply byte per message.

There is no socket file to find and nothing to connect to. The descriptor is already open when the
agent's program starts, and only that agent's process received it, so daimon knows who is talking.
The source of every message is the agent daimon started on that channel, whatever the message
says.

On AGNOS, agents cannot be started yet (a start answers 501 until 2.4.x maps `chan_op`, the kernel's
capability channels, which work the same way).

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
| `target` | yes | a non-empty string of at most 256 bytes: an agent id (`"7"`), a registered name, or `"*"` for every agent |
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
| 1 | `ACK` | accepted: the message will be put on daimon's message bus |
| 2 | `NACK_QUEUE_FULL` | daimon is holding 1024 messages it has not yet routed. Nothing was kept; send it again later |
| 3 | `NACK_INVALID` | the frame was refused (see above). The channel stays open, except for the two faults below |

`ACK` means *accepted for routing*, not *delivered*. A message to an id or name with no queue is
dropped when it is routed.

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

daimon's main thread routes accepted messages onto its **message bus** at the start of each HTTP
request:
- a target that is an agent **id** goes to that agent's queue;
- a **name** goes to the agent registered under it;
- `"*"` goes to the queue of every registered agent, the sender's included.

An agent has a queue from registration (`POST /v1/agents`) until it is deleted, whether or not it
is running. Each queue holds **100** messages. Messages past that are dropped and counted.

⚠ 2.3.3 is the first half:
- no route reads a queue or sends to an agent yet;
- registration does not yet register names, so a name target reaches no one.

Both come in the next release. Until then the bus fills, and `/v1/metrics` shows the traffic.

## Metrics

`GET /v1/metrics` adds, since 2.3.3:

| field | counts |
|---|---|
| `ipc_messages` | messages put on the bus |
| `ipc_refused` | frames answered 2 or 3, plus channels closed on a fault |
| `bus_dropped` | messages dropped at a full queue |

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
reply = os.read(fd, 1)[0]    # 1 ACK, 2 NACK_QUEUE_FULL, 3 NACK_INVALID
```

## Limits

| limit | value | constant (`src/ipc.cyr`) |
|---|---|---|
| frame body | 65536 bytes | `MAX_MESSAGE_SIZE` |
| target | 256 bytes | `IPC_TARGET_MAX` |
| time to finish a begun frame | 5000 ms | `IPC_FRAME_TIMEOUT_MS` |
| messages waiting for the main thread | 1024 | `IPC_HANDOFF_CAP` |
| frames one channel may complete per service pass | 16 | `IPC_FRAMES_PER_PASS` |
| messages per bus queue | 100 | `IPC_BUS_QUEUE_MAX` |

Why the channel works this way, and what it costs daimon, is in
[ADR-005](../adr/005-agent-channels.md).
