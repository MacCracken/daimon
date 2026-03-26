# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- Full axum HTTP API router (`api.rs`) with 20+ endpoints: health, agents, MCP tools, RAG ingest/query, edge fleet, scheduler, metrics.
- Integration test suite — 28 HTTP round-trip tests covering all API subsystems.
- Benchmark suite — 15 benchmarks covering agent registration throughput, MCP dispatch, API latency, vector search, RAG ingest, scheduler scheduling, and edge fleet registration.

## [0.1.0] - 2026-03-25

### Added

- Initial scaffold — api, agent, supervisor, ipc, scheduler, federation, edge, memory, vector_store, rag, mcp, screen, config modules.
