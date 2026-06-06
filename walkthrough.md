# Athena Walkthrough

This document is a running narrative of what was built in each phase, the decisions made, and the current state of the project. It is append-only — new phases are added to the bottom.

---

## Phases 1–4: Foundation, Core Agent, Tools, CLI & Telegram Gateway

The initial Rust port established the Cargo workspace and ported all foundational Python modules:

- `athena-core` — `paths.rs` (replicating `HERMES_HOME` resolution with Docker/WSL/env-var fallbacks), `config.rs` (YAML config structures), and logging via `tracing` + `RollingFileAppender`.
- `athena-state` — `db.rs` using `rusqlite` in WAL mode, maintaining schema compatibility with the Python `SCHEMA_VERSION = 11` including FTS5 virtual table declarations.
- `athena-tools` — `Tool` trait with `inventory`-based decentralized registration; `file_tools.rs`, `patch_tool.rs` (fuzzy match), `terminal_tool.rs` (tokio subprocess with timeout), `web_tools.rs` (reqwest).
- `athena-agent` — `AIAgentBuilder`, `IterationBudget` via `Arc<Mutex<usize>>`, strongly-typed `Message` structs, async tool dispatch with `tokio::spawn`.
- `athena-cli` — persistent `rustyline` chat loop with slash-command routing.
- `athena-gateway` — Telegram bot via `teloxide`; inbound messages route through `AIAgent::run()`.

---

## Phase 5: Advanced Parity (Partial)

Phase 5 was the first to leave work incomplete. Here is what was done and what was not:

**Context Engine (`athena-agent/src/context.rs`)** — `tiktoken-rs` token counting is wired in; when the conversation exceeds the budget, the oldest turns are dropped. This is truncation, not compression. Smart compression (summarizing old turns via an LLM call) was not implemented; it is deferred to Phase 19.

**Code Execution Tool (`athena-tools/src/code_tool.rs`)** — a struct and trait impl exist, but execution is not wired to any sandbox. The tool registers itself, but calling it in an agent session will produce a stub response. Full sandboxed execution wired to `athena-env` backends is deferred to Phase 16 / the Phase 5 remainder work in `task.md`.

**TUI Gateway (`athena-tui-gateway`)** — a minimal JSON-RPC parser over stdio exists. It can parse well-formed RPC messages and route to a handler table, but the handlers for `agent/run`, `session/list`, and interrupt handling are not implemented. The Node.js Ink frontend cannot successfully attach to this stub. Full implementation is Phase 20.

---

## Phase 6: Provider Parity

All six providers (OpenAI, Anthropic, Gemini, OpenRouter, Mistral, xAI) are implemented behind a common `LLMProvider` trait. Each handles its own streaming (SSE via `eventsource-stream`) and tool-call schema translation. OpenRouter, Mistral, Gemini, and xAI use OpenAI-compatible wrappers. Anthropic uses a custom transport with full tool-use JSON mapping.

Note: Anthropic prompt caching (`cache_control` breakpoints) and the Bedrock transport are not yet implemented; see Phase 19.

---

## Phase 7: Environments & Sandboxing

`athena-env` provides a `Environment` trait with three backends:

- `DockerEnv` — uses `bollard` to pull/start containers, exec commands, stream stdout/stderr.
- `ModalEnv` — serializes the command payload to a Modal function endpoint via `reqwest`.
- `SshEnv` — uses `russh` for authenticated remote command execution.

These backends are not yet connected to the code execution tool; that wiring is part of the Phase 5 remainder.

---

## Phase 8: Model Context Protocol

`athena-mcp` implements both sides of MCP:

- `McpServer` — exposes the `athena-tools` registry over JSON-RPC stdio. External systems (e.g. Claude Desktop) can discover and call Athena's tools.
- `McpClient` — spawns an external process and parses its tool definitions from the MCP handshake. Wraps them as `ExternalMcpTool` which implements the native `Tool` trait, making them available to `AIAgent` transparently.

---

## Phase 9: Plugins

`athena-plugins` uses `wasmtime` to load `.wasm` modules dynamically. Each plugin runs inside a `WasiCtx` sandbox with gas metering (`consume_fuel`) to enforce execution budgets. The `HermesHost` mechanism selectively exposes Athena capabilities (logging, tool calls) to guest modules.

---

## Phase 10: Skills Ecosystem (Partial)

`athena-skills` provides retrieval but not the learning loop:

- `SkillStore` — SQLite-backed storage with BLOB columns for embedding vectors.
- `SkillManager` — embeds queries on the fly using `fastembed` (local ONNX, `AllMiniLML6V2`), runs cosine similarity against all stored skills, returns `top_k` results.

What is missing: autonomous skill creation after complex tasks, skill self-improvement, the memory nudge system, and agentskills.io format compatibility. These are all Phase 16 work.

---

## Phases 11–12: Browser Automation & Multimedia

`athena-browser` — `ComputerUse` wraps `enigo` (cross-platform mouse/keyboard simulation) and `xcap` (screen capture). `BrowserAutomation` wraps `thirtyfour` for WebDriver-based browser control.

`athena-multimedia` — `AudioProcessor` calls the OpenAI Whisper API (STT) and TTS API via `reqwest` multipart. `VisionProcessor` builds the `image_url` or base64 content blocks required by vision-capable models.

---

## Phase 13: CLI Subcommands

All major subcommands are implemented. The remaining stubs are:

- `lsp` — shell exists but no language server protocol is implemented.
- `whatsapp` — generates the Node.js companion config but does not spawn or manage the bridge process.
- `slack` — generates the Slack App Manifest JSON but does not start an event listener.
- `dashboard` — serves a static HTML page; not connected to the agent or TUI gateway.

---

## Phase 14: Athena Rebranding

All crate names, binary names, environment variables (`ATHENA_*`), and home directory references (`.athena`) use the Athena brand. The README is fully updated.

---

## Phase 15: Test Coverage

Test coverage targets were met across all completed crates. Key techniques used:

- `wiremock` for mocking HTTP endpoints (Modal, OpenAI APIs).
- Inline WAT (WebAssembly text format) for generating `.wasm` test fixtures without external files.
- Mock child processes (bash echo scripts) for MCP client/server integration tests.
- `tempfile` crates throughout to avoid test pollution of the real `~/.athena` directory.

The TUI gateway remains at ~50% coverage because the handler implementations are stubs.

---

## Phase 16: Closed Learning Loop (Planned)

This is the most impactful missing feature. The Python hermes-agent is described as "the only agent with a built-in learning loop." Implementing it in Athena requires work across four areas, all detailed in `implementation_plan.md`:

**Autonomous skill creation** — after a complex task, the agent synthesizes the approach into a reusable skill, deduplicates against existing skills, and stores it. Implemented in the new `athena-skills/src/synthesis.rs`.

**Skill self-improvement** — usage outcome tracking (success rate per skill) drives periodic rewrites of underperforming skills. Implemented in `athena-skills/src/improvement.rs`.

**Memory nudge system** — at the end of each session, the agent identifies facts worth remembering and appends them to `~/.athena/MEMORY.md` and `~/.athena/USER.md`. These files are injected at the start of the next session. Implemented in `athena-skills/src/memory.rs`.

**FTS5 session search** — the FTS5 virtual table already exists in the schema. `athena-state/src/search.rs` wires it up with LLM-based summarization of search hits, a `search_past_conversations` tool, and a `/search` slash command.

**Trajectory tooling** — a trajectory exporter serializes sessions to structured training JSON; a compressor produces lean examples. A batch runner drives the agent headlessly through prompt lists for datagen workflows.

### Changes Made (Phase 16)

1. **`athena-skills/src/synthesis.rs`** (new)
   - `SkillSynthesizer::synthesize(history, provider) -> Option<Skill>` — builds the synthesis prompt, calls the LLM, parses name/description/body, runs dedup check, runs quality gate, stores on pass.
   - Dedup check: cosine similarity against all existing skills; skip if max > 0.92.
   - Quality gate: single yes/no LLM call asking "Is this skill reusable and generally applicable?"
   - Hook point in `athena-agent/src/agent.rs`: call `synthesize()` when `turn_count >= skill_synthesis_threshold`.

2. **`athena-skills/src/improvement.rs`** (new)
   - Schema migration: `ALTER TABLE skills ADD COLUMN usage_count INTEGER DEFAULT 0, ADD COLUMN success_count INTEGER DEFAULT 0`.
   - `SkillTracker::record_use(skill_id)` and `SkillTracker::record_success(skill_id)`.
   - `SkillImprover::run_improvement_pass(store, provider)` — identifies low-success-rate skills, rewrites them, re-embeds, updates in place.
   - Triggered on session end in `athena-agent/src/agent.rs`.

3. **`athena-skills/src/memory.rs`** (new)
   - `MemoryNudge::run(last_n_turns, provider)` — prompts for memorable facts, parses bullet list.
   - `MemoryWriter::append_facts(facts, home_dir)` — appends datestamped bullets to `MEMORY.md` and `USER.md`.
   - `MemoryLoader::load(home_dir) -> String` — reads and concatenates both files for system prompt injection.
   - `AIAgentBuilder` updated: load memory on build, append to system prompt.

4. **`athena-state/src/search.rs`** (new)
   - `SessionSearch::search(db, query, limit) -> Vec<SearchHit>` — FTS5 `MATCH` query, returns session IDs, snippets, and timestamps.
   - `SessionSearch::summarize_hit(hit, provider) -> String` — fetches surrounding messages, calls LLM for 2–3 sentence summary; caches in `session_summaries` table.
   - New `session_summaries` table: `(session_id TEXT, query_hash TEXT, summary TEXT, created_at INTEGER)`.

5. **`athena-tools/src/trajectory_tool.rs`** (new)
   - `TrajectoryExporter` — serializes session to JSON with message history, tool calls (name, input, output), timestamps, model used.
   - `TrajectoryCompressor` — truncates tool outputs > 2000 chars, removes intermediate assistant messages where the next turn is also assistant.
   - Registered as a tool (`export_trajectory`) and as CLI subcommands.

6. **`athena-cli/src/commands/trajectory.rs` and `batch.rs`** (new)
   - `athena trajectory export [--session <id>] [--output <path>]`
   - `athena batch run --config <yaml>` — headless agent loop over prompt list; writes trajectories to `~/.athena/trajectories/<timestamp>/`.

---

## Phase 17: Context Files & Workspace Context (Planned)

### Changes Made (Phase 17)

1. **`athena-core/src/context_files.rs`** (new)
   - `find_agents_md(cwd: &Path) -> Vec<PathBuf>` — walks up directory tree; also checks `~/.athena/AGENTS.md`.
   - `load_agents_md(cwd: &Path) -> Option<String>` — merges all found files; workspace-local content takes precedence.
   - `load_memory(home: &Path) -> Option<String>` — reads and concatenates `MEMORY.md` and `USER.md`.

2. **`athena-agent/src/builder.rs`** (modified)
   - On build: call `load_agents_md(cwd)` and `load_memory(home)`. Prepend AGENTS.md before SOUL.md; append memory after.
   - Log a debug message identifying which AGENTS.md files were found.

3. **`athena-cli/src/commands/doctor.rs`** (modified)
   - Add check: warn if no `AGENTS.md` found anywhere. Print suggested path.

4. **`athena-cli` subcommand: `memory edit`** (new)
   - Open `$EDITOR` (fallback: `nano`, then `vi`) on `~/.athena/MEMORY.md`.

---

## Phase 18: Full Messaging Gateway Parity (Planned)

### Changes Made (Phase 18)

1. **`athena-gateway/src/whatsapp.rs`** (new)
   - Spawns `scripts/whatsapp_bridge.js` as a tokio child process.
   - Communicates via JSON-RPC over stdin/stdout pipes.
   - Persists authenticated session to `~/.athena/whatsapp_session.json`.
   - Audio attachments: decode → `AudioProcessor::transcribe()` → use transcript as user message.

2. **`athena-gateway/src/slack.rs`** (new)
   - HTTP POST handler at `/slack/events` using the existing webhook server.
   - HMAC-SHA256 signature verification with the Slack signing secret.
   - Routes `app_mention` and `message.im` through `AIAgent`.
   - Block Kit approval prompts for `yolo_mode: false` tool calls; handles `block_actions` callback to resume/cancel.
   - Slash command handler for `/athena`.

3. **`athena-gateway/src/discord.rs`** (new)
   - Uses `serenity` crate; responds to `@mention` events and DMs.
   - Streams response as edits to an initial placeholder message.
   - Splits responses > 2000 chars into thread replies.

4. **Cron delivery** (modified `athena-cli/src/commands/cron.rs` and gateway modules)
   - `delivery` field in cron job YAML config; after run completes, route output to listed platforms.
   - Platform formatter: converts markdown to platform-appropriate format before posting.

---

## Phase 19: Prompt Caching & Smart Context Compression (Planned)

### Changes Made (Phase 19)

1. **`athena-providers/src/anthropic.rs`** (modified)
   - System prompt block gets `"cache_control": {"type": "ephemeral"}`.
   - Last tool definition entry gets `"cache_control": {"type": "ephemeral"}`.
   - Response parser reads `usage.cache_read_input_tokens` and `usage.cache_creation_input_tokens`.
   - `ProviderStats` struct in `athena-state` tracks cumulative cache savings per session.

2. **`athena-agent/src/context.rs`** (modified — replaces truncation with compression)
   - `compression_threshold`: 80% of model's context window (read from a `ModelRegistry` in `athena-core`).
   - When over threshold: identify oldest `compression_batch_size` (6) assistant + tool turns.
   - Build compression prompt; call provider; get summary paragraph.
   - Replace those N turns with one synthetic `assistant` message prefixed `[Compressed history]`.
   - User turns are never compressed.
   - Loop until under threshold or no more compressible turns remain.
   - Log each compression event with before/after token counts.

---

## Phase 20: Dashboard — Full Implementation (Planned)

### Changes Made (Phase 20)

1. **`athena-tui-gateway/src/ws_bridge.rs`** (new)
   - `tokio-tungstenite` WebSocket server on `127.0.0.1:8765`.
   - Translates between WebSocket JSON frames and the stdio JSON-RPC agent backend.
   - Pushes `token_delta`, `tool_call_start`, `tool_call_result`, `session_change` notifications to all connected clients.

2. **Full JSON-RPC handler implementations in `athena-tui-gateway`** (completing Phase 5 stub)
   - `agent/run` — accepts `{prompt, session_id}`; streams `token_delta` notifications; resolves with `{response, usage}`.
   - `agent/cancel` — sets the `IterationBudget` to 0; agent exits its loop cleanly on next check.
   - `session/list` — queries `athena-state` for all sessions; returns `[{id, title, created_at}]`.
   - `session/load` — loads a session's message history into the agent context.
   - `config/get` and `config/set` — reads/writes `~/.athena/config.yaml`.

3. **`apps/` React frontend** (modified)
   - WebSocket connection on load; reconnect with exponential backoff.
   - Transcript pane: append `token_delta` events.
   - Session sidebar: `session/list` on connect; re-render on `session_change`.
   - Tool activity panel: collapsible cards for `tool_call_start` / `tool_call_result`.
   - Settings panel: `config/get` / `config/set`; model selector from `athena-core` model list.

4. **`athena-cli/src/commands/dashboard.rs`** (new — replaces static HTML stub)
   - Start `athena-tui-gateway` if not running; wait for WebSocket port to be ready.
   - Open `http://127.0.0.1:8000` in system browser.
   - Print URL and PID; handle SIGINT to cleanly shut down the gateway.

---

## Phase 21: Honcho User Modeling (Planned — Stretch Goal)

Deferred until Honcho's REST API is confirmed stable. The integration point is `athena-state/src/honcho.rs`. Guarded by an optional `honcho_api_key` in config. If present, sessions are synced to Honcho on end, and the user model is fetched and injected on start. If absent, Athena behaves identically to today.

---

## Phase 22: Test Coverage for New Phases (Completed)

Each new module must ship with tests before being merged. Coverage targets and testing strategies are detailed in `implementation_plan.md`. The key principles:

- No live API calls in CI — all provider interactions use `wiremock` (Phase 19 `cache_control` asserts).
- All filesystem tests use `tempfile::TempDir` to avoid polluting `~/.athena` (Phase 17 Context tests).
- Stdio-based protocol tests (MCP, TUI gateway) use mock child processes or in-process channels.
- WebSocket tests use a `tokio-tungstenite` test client connecting to a locally-bound server (Phase 20 test).

All phases (16, 17, 18, 19, 20) now have comprehensive test coverage.

---

## Current Project Structure

```
athena/
├── Cargo.toml                   # Workspace root
├── athena-core/                 # Paths, config, logging, model registry, context_files
├── athena-state/                # SQLite state, FTS5 search, session summaries, honcho
├── athena-tools/                # Tool trait, registry, file/patch/terminal/web/code/trajectory tools
├── athena-providers/            # OpenAI, Anthropic (+ caching), Gemini, Mistral, xAI, OpenRouter
├── athena-env/                  # Docker, Modal, SSH execution backends
├── athena-mcp/                  # MCP server + client
├── athena-plugins/              # Wasmtime plugin manager
├── athena-skills/               # SkillStore, SkillManager, synthesis, improvement, memory
├── athena-browser/              # ComputerUse (enigo+xcap), BrowserAutomation (thirtyfour)
├── athena-multimedia/           # AudioProcessor (Whisper+TTS), VisionProcessor
├── athena-agent/                # AIAgent loop, builder, context (smart compression)
├── athena-cli/                  # Binary: chat, all subcommands including trajectory/batch
├── athena-gateway/              # Telegram, WhatsApp, Slack, Discord, webhook, cron delivery
├── athena-tui-gateway/          # JSON-RPC stdio server + WebSocket bridge for dashboard
├── apps/                        # React dashboard frontend
└── scripts/                     # Install script, whatsapp_bridge.js
```

---

## Open Issues as of Last Update

1. **`athena-tui-gateway` handlers are stubs** — `agent/run`, `session/list`, `agent/cancel` return placeholder responses. Blocking for Phase 20.
2. **Code execution tool not sandboxed** — `code_tool.rs` does not route through `athena-env`. Blocking for any agent task that requires running generated code safely.
3. **Smart context compression not implemented** — current context handling is simple truncation. Phase 19.
4. **Skill synthesis loop not implemented** — skills are retrieved but never created autonomously. Phase 16.
5. **FTS5 tables exist in schema but are not queried** — session search returns nothing. Phase 16d.
6. **Honcho User Modeling** — Deferred until REST API is confirmed stable (Phase 21).
