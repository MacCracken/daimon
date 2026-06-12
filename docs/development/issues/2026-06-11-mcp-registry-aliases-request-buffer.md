# MCP host registry aliases the transient request buffer — entries corrupt as later requests arrive

**Status:** open — filed 2026-06-11 by thoth (consumer), daimon 1.2.4
**Severity:** HIGH — external MCP tool calls fail or fire at attacker-influenced
URLs depending on byte offsets of *subsequent unrelated requests*

## Symptom

Register an external MCP tool, let any other HTTP request arrive, then call
the tool: `POST /v1/mcp/call` returns
`{"error":"upstream MCP call failed","code":502,"upstream":""}` even though
the callback endpoint is up and reachable. `GET /v1/mcp/tools` then shows the
registry visibly corrupted — tool names and descriptions render as fragments
of **later requests' bytes**:

```json
{"tools":[
  {"name":"ded\r\n","description":"cho\",\"ar"},
  {"name":"ed\r\n\r\n{\"n","description":"\"arguments\":{\"text\":\"via mitm\"}}.0.1:9778/v1"}
],"count":4}
```

(Those fragments are recognizably HTTP header tails and the body of a
*subsequent* `/v1/mcp/call` request.)

## Root cause (from the consumer's black-box evidence)

`api_mcp_register` (src/main.cyr ~3250) stores the `name` / `description` /
`callback_url` Strs produced by `json_parse(body)` directly into the
registry. Those Strs point into the **per-connection request buffer**, which
is reused for later requests in sync mode — nothing copies them into stable
storage. Every later request overwrites the bytes the registry points at.

Whether a given call still works is pure offset luck:

- Registering with a ~1000-byte description (parking `callback_url` beyond
  the write range of every later request) makes the same call succeed
  reliably — verified end-to-end (thoth → daimon → hoosh `/v1/tools/call`
  → bote dispatcher round-trip).
- The tool *name* usually survives because every `/v1/mcp/call` body happens
  to place the same name at the same offset (`{"name":"...`), which is why
  the failure surfaces as a 502 (clobbered URL) rather than 404.

## Repro

```sh
./build/daimon serve 8090 &
# any MCP JSON-RPC endpoint works as callback; hoosh 2.4.5 shown
hoosh serve &   # bote_echo at http://127.0.0.1:8088/v1/tools/call

curl -X POST localhost:8090/v1/mcp/tools -H 'Content-Type: application/json' \
  -d '{"name":"bote_echo","description":"e","callback_url":"http://127.0.0.1:8088/v1/tools/call"}'
curl localhost:8090/v1/mcp/tools          # <- any intervening request
curl -X POST localhost:8090/v1/mcp/call -d '{"name":"bote_echo","arguments":{}}'
# -> 502 {"upstream":""}; /v1/mcp/tools now shows corrupted names
```

Control: pad `description` to ~1 KB and the call succeeds every time.

## Security note

This is not just availability: a clobbered `callback_url` is *whatever bytes
a later request left at that offset*. A crafted request body could in
principle steer an external tool call to an attacker-chosen URL after the
fact, sidestepping the `validate_callback_url` SSRF guard, which runs only at
registration time. Suggested fix: deep-copy `name` / `description` /
`callback_url` into registry-owned allocations in `mcp_register_external`
(the bump allocator never frees, so a plain copy is stable for the process
lifetime), and re-validate the URL at call time.

## Consumer impact

thoth 0.3.0 (M4) wires daimon as its MCP tool host; its `/tools` and `/call`
are correct against a fresh registration but inherit this instability. thoth
ships with the limitation documented and no workaround in-tree (padding the
registration is the operator-side stopgap).
