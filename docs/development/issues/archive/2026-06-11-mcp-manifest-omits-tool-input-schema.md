# MCP manifest omits per-tool input schema; registration hardcodes `{}` — consumers can't advertise real JSON Schemas

**Status:** RESOLVED in daimon 1.2.7 — filed 2026-06-11 by thoth (consumer),
against daimon 1.2.6. `api_mcp_register` now reads `inputSchema` (accepting the
`input_schema` alias) via bayan's typed engine and stores it verbatim;
`api_mcp_manifest` emits it as raw JSON per tool (permissive `{"type":"object"}`
fallback when unset). Backward-compatible — absent schema still defaults to `{}`.
**Severity:** MEDIUM — functional gap, not a correctness/security bug. It caps the
quality of model-driven tool calling: agents must guess each tool's arguments
because daimon advertises no parameter schema.

## Summary

The `McpToolDescription` struct already carries an `input_schema` field
(`src/main.cyr` ~line 1263: `{name, description, input_schema}`, accessor
`mcp_tool_schema`), but it is **write-once-empty and never exported**:

1. **Registration discards any client schema.** `api_mcp_register` builds the
   tool with a hardcoded empty schema:

   ```
   var tool = mcp_tool_new(name, desc, str_from("{}"));
   ```

   It reads `name`, `description`, `callback_url` from the POST body but **not**
   `inputSchema` (MCP's standard field) / `input_schema`. A registrant cannot
   supply a JSON Schema even though the struct has a slot for it.

2. **The manifest omits the schema.** `GET /v1/mcp/tools` (`api_mcp_manifest`)
   emits only `name` + `description` per tool:

   ```json
   {"tools":[{"name":"echo","description":"echo back"}],"count":1}
   ```

   `mcp_tool_schema(t)` is never read, so even a non-empty schema would not reach
   a consumer.

## Why it matters (consumer context)

thoth drives a model-driven **agentic tool-calling loop**: it fetches
`GET /v1/mcp/tools`, advertises the tools to the LLM (hoosh) as OpenAI
function-tools, the model emits `tool_calls`, thoth executes them via
`POST /v1/mcp/call`. The standard OpenAI/MCP tool definition is
`{name, description, parameters: <JSON Schema>}` — the `parameters` schema is how
the model knows each tool's argument shape, types, and which are required.

Because daimon exports no schema, thoth must advertise every tool with a
permissive `"parameters": {"type":"object"}` (any object accepted). The model
then **guesses** argument names/types from the description prose alone. With real
schemas, tool calls would be correctly typed and validated up front.

This is the one gap blocking high-fidelity tool calling now that the registry
aliasing bug (`2026-06-11-mcp-registry-aliases-request-buffer.md`) is fixed and
the seam is otherwise wire-complete in 1.2.6.

## Requested change

The MCP spec models a tool as `{name, description, inputSchema}` where
`inputSchema` is a JSON Schema object. Two daimon-side edits:

1. **Read the schema on registration.** In `api_mcp_register`, read
   `inputSchema` (accept `input_schema` too if you like) from the POST body and
   pass it to `mcp_tool_new` instead of `str_from("{}")`; default to `{}` when
   absent (back-compatible). Treat it as opaque JSON text (store verbatim), or
   validate it is a JSON object.

2. **Export the schema in the manifest.** In `api_mcp_manifest`, emit the stored
   schema per tool — MCP-canonically as `inputSchema` (raw JSON, not a quoted
   string):

   ```json
   {"tools":[
     {"name":"echo","description":"echo back",
      "inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}
   ],"count":1}
   ```

   Builtins with no schema can emit `{"type":"object"}` (or omit `inputSchema`).

## Consumer adaptation (thoth side, once available)

thoth's `agent_format_tools` (`src/agent.cyr`) currently hardcodes
`"parameters":{"type":"object"}`. When daimon exports `inputSchema`, thoth will
pass it through as the OpenAI `function.parameters` value (falling back to the
permissive object schema when a tool omits it). No daimon API/route change beyond
the additive field is needed — the change is backward-compatible (extra response
field; new optional request field).

## Reproduce

```sh
daimon serve &
# Register with a real schema — daimon currently ignores inputSchema:
curl -s -XPOST localhost:8090/v1/mcp/tools -d '{
  "name":"echo","description":"echo back","callback_url":"http://127.0.0.1:9000",
  "inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}'
# Manifest shows no schema:
curl -s localhost:8090/v1/mcp/tools
# => {"tools":[{"name":"echo","description":"echo back"}],"count":1}
```
