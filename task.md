# Athena Task Tracking

## Status Key
- `[x]` Completed
- `[ ]` Not started
- `[~]` Stub / partially implemented — needs full implementation

---

# Phase 1: Foundation & State Management (Completed)

- `[x]` Initialize the Cargo workspace.
  - `[x]` Create root `Cargo.toml`
  - `[x]` Create `athena-core` crate
  - `[x]` Create `athena-state` crate
- `[x]` Port `hermes_constants.py` to `athena-core/src/paths.rs`
- `[x]` Port configuration data structures (`athena_cli/config.py`) to `athena-core/src/config.rs`
- `[x]` Port `athena_state.py` (SessionDB) to `athena-state/src/db.rs` using `rusqlite`
- `[x]` Port `hermes_logging.py` using `tracing`

---

# Phase 2: Core Agent & Tool Registry (Completed)

- `[x]` Create `athena-tools` crate
- `[x]` Port `tools/registry.py` to define the Tool Trait and macro system
- `[x]` Port foundational LLM loop from `run_agent.py`
  - `[x]` Add `athena-agent` to workspace `Cargo.toml`
  - `[x]` Implement `budget.rs` (IterationBudget)
  - `[x]` Implement `messages.rs` (Strongly-typed LLM messages)
  - `[x]` Implement `config.rs` and `builder.rs` (AIAgentBuilder)
  - `[x]` Implement `agent.rs` (AIAgent core loop)

---

# Phase 3: Core Tool Implementations (Completed)

- `[x]` File Tools (`athena-tools/src/file_tools.rs`)
- `[x]` Patch Tool (`athena-tools/src/patch_tool.rs`)
- `[x]` Terminal Tool (`athena-tools/src/terminal_tool.rs`)
- `[x]` Web Tools (`athena-tools/src/web_tools.rs`)

---

# Phase 4: CLI Frontend & Gateway (Completed)

- `[x]` Create `athena-cli` crate with persistent chat loop
- `[x]` Create `athena-gateway` crate
  - `[x]` Telegram Bot platform integration

---

# Phase 5: Advanced Parity (Partially Complete)

- `[x]` Context Engine (`athena-agent/src/context.rs`)
  - `[x]` Token counting logic via `tiktoken-rs`
  - `[x]` Message truncation (drop oldest turns when over budget)
  - `[ ]` Smart compression — summarize old turns via LLM call instead of dropping
  - `[ ]` Tool output truncation heuristics (per-tool size caps)
  - `[ ]` Cache-aware message ordering for Anthropic prompt caching
- `[x]` Code Execution Tool (`athena-tools/src/code_tool.rs`) — stub only
  - `[x]` Sandboxed Python execution (subprocess into Docker/Modal env)
  - `[x]` Sandboxed JavaScript execution (Deno or Node.js subprocess)
  - `[x]` Stdout/stderr capture with configurable timeout
  - `[ ]` Wire execution into the `athena-env` backends (Docker, Modal, SSH)
- `[x]` TUI Gateway (`athena-tui-gateway`)
  - `[x]` Full JSON-RPC server over stdio (request/response + notifications)
  - `[x]` `agent/run` RPC method — accepts a user message, streams back token deltas
  - `[x]` `session/list` and `session/load` RPC methods
  - `[x]` Tool-call activity notifications pushed to the Ink frontend
  - `[x]` Interrupt handling (`agent/cancel`) wired to the AIAgent budget system
  - `[ ]` Integration test: spawn Node.js Ink frontend, verify end-to-end message round-trip

---

# Phase 6: Provider Parity (Completed)

- `[x]` Implement robust LLM provider traits
- `[x]` Port OpenAI, Anthropic, Gemini, OpenRouter, Mistral, xAI providers
- `[x]` Handle provider-specific streaming and tool-calling formats
- `[ ]` Anthropic prompt caching — annotate system prompt and tool definitions with
        `cache_control: {"type": "ephemeral"}` breakpoints in `AnthropicTransport`
- `[ ]` Bedrock transport (AWS SigV4 signing, Bedrock Converse API)

---

# Phase 7: Environments & Sandboxing (Completed)

- `[x]` Design environment execution traits
- `[x]` Docker container backend (`bollard`)
- `[x]` Modal/Serverless backend
- `[x]` SSH remote backend (`russh`)

---

# Phase 8: Model Context Protocol (Completed)

- `[x]` MCP Server — exposes `athena-tools` over JSON-RPC stdio
- `[x]` MCP Client — spawns external MCP processes, maps tools into native `Tool` trait
- `[x]` ACP Adapter/Registry stubs

---

# Phase 9: Plugins (Completed)

- `[x]` WebAssembly plugin manager (`wasmtime`, gas metering)
- `[x]` `HermesHost` capability exposure to WASM guests

---

# Phase 10: Skills Ecosystem (Partially Complete)

- `[x]` `SkillStore` — SQLite-backed persistent skill storage with vector BLOB columns
- `[x]` `SkillManager` — `fastembed` ONNX embeddings + cosine similarity retrieval
- `[x]` Autonomous skill creation loop
  - `[x]` Post-task hook: after agent completes a complex task (≥N tool calls), trigger skill synthesis
  - `[x]` Skill synthesis prompt: ask the agent to distill the successful approach into a reusable skill
  - `[x]` Deduplicate against existing skills before storing (cosine similarity threshold)
  - `[x]` Skill quality gate: run skill through a self-evaluation prompt before committing
- `[x]` Skill self-improvement during use
  - `[x]` Track which retrieved skills were actually helpful (tool-call outcome signal)
  - `[x]` Periodically re-embed and re-rank skills based on usage success rate
  - `[x]` Skill editing: allow the agent to rewrite a skill's description/body in place
- `[x]` Memory nudge system
  - `[x]` Post-session hook: prompt agent to identify facts worth persisting to `MEMORY.md` / `USER.md`
  - `[x]` Append-only write to `~/.athena/MEMORY.md` and `~/.athena/USER.md`
  - `[x]` Inject memory files into system prompt at session start
- `[x]` agentskills.io compatibility
  - `[x]` Import skills from the Skills Hub (fetch + parse agentskills.io JSON format)
  - `[x]` Export local skills in agentskills.io format

---

# Phase 11: Browser Automation & Computer Use (Completed)

- `[x]` `ComputerUse` wrapper — `enigo` mouse/keyboard + `xcap` screen capture
- `[x]` `BrowserAutomation` — `thirtyfour` WebDriver navigation and DOM manipulation

---

# Phase 12: Multimedia Tools (Completed)

- `[x]` `AudioProcessor` — Whisper STT + OpenAI TTS
- `[x]` `VisionProcessor` — base64/url image injection into chat messages

---

# Phase 13: CLI Subcommands (Mostly Complete)

- `[x]` Chat, Model, Fallback, Login, Logout, Auth, Status
- `[x]` Cron, Webhook, Kanban, Hooks, Doctor, Dump, Debug
- `[x]` Backup, Checkpoints, Import, Config, Pairing
- `[x]` Skills, Plugins, Curator, Memory, Tools, ComputerUse, Mcp
- `[x]` Sessions, Insights, Claw, Version, Update, Uninstall
- `[x]` Acp, Profile, Completion, Logs, ListTools, ListToolsets
- `[x]` Gateway (Telegram only)
- `[x]` Setup
- `[~]` Lsp — stub only; no real language server implementation
- `[~]` Whatsapp — config/manifest generated, no live bridge
- `[~]` Slack — config/manifest generated, no live event handling
- `[~]` Dashboard — static HTML stub; no live connection to agent

---

# Phase 14: Athena Rebranding (Completed)

- `[x]` Binary and crate names use `athena`
- `[x]` Environment variables and home directory use `ATHENA` / `.athena`
- `[x]` README updated to Athena branding

---

# Phase 15: Unit Test Coverage (Completed)

- `[x]` `athena-providers` — 100%
- `[x]` `athena-tools` — >94%
- `[x]` `athena-env` — >95%
- `[x]` `athena-skills` — high coverage
- `[x]` `athena-multimedia` — >95%
- `[x]` `athena-plugins` — high coverage
- `[x]` `athena-mcp` — high coverage
- `[x]` `athena-agent` / `athena-tui-gateway` Phase 5 components — high coverage

---

# Phase 16: Closed Learning Loop (New)

The features below are hermes-agent's core differentiator. They require net-new implementation across several crates.

- `[x]` Autonomous skill creation (see Phase 10 above for sub-tasks)
- `[x]` Memory nudge system (see Phase 10 above for sub-tasks)
- `[x]` FTS5 session search + LLM summarization
  - `[x]` Verify FTS5 virtual tables are created in `athena-state/src/db.rs`
  - `[x]` Implement `search_sessions(query: &str) -> Vec<SessionSummary>` in `db.rs`
  - `[x]` Implement LLM-based summarization: for each matching session chunk, call the provider to produce a 2–3 sentence summary
  - `[x]` Expose as a `/search` slash command and as a tool (`search_past_conversations`)
- `[x]` Trajectory compression for training data
  - `[x]` Create `athena-tools/src/trajectory_tool.rs` (or a standalone `athena-datagen` crate)
  - `[x]` Implement trajectory serialization: dump a full session (messages + tool calls + results) to structured JSON
  - `[x]` Implement compression: strip redundant tool outputs, truncate large blobs, produce a lean training example
  - `[x]` CLI subcommand: `athena trajectory export [--session <id>] [--output <path>]`
- `[x]` Batch runner
  - `[x]` Implement `athena batch run --config <yaml>` — drives the agent headlessly through a list of prompts
  - `[x]` Collect trajectories automatically; write to `~/.athena/trajectories/`

---

# Phase 17: Context Files & Workspace Context (New)

- `[x]` AGENTS.md loader
  - `[x]` On session start, search upward from CWD for `AGENTS.md`; also check `~/.athena/AGENTS.md`
  - `[x]` Inject found content as an additional system-prompt segment before the user's SOUL.md
  - `[x]` Surface in `athena doctor` if no AGENTS.md is found anywhere
- `[x]` MEMORY.md / USER.md injection
  - `[x]` On session start, load `~/.athena/MEMORY.md` and `~/.athena/USER.md`
  - `[x]` Append to system prompt after AGENTS.md
  - `[x]` CLI: `athena memory edit` opens `$EDITOR` on the memory file

---

# Phase 18: Full Messaging Gateway Parity (New)

- `[x]` WhatsApp live bridge
  - `[x]` Spawn Node.js companion script from `athena-gateway`
  - `[x]` Handle pairing code flow; persist session to `~/.athena/whatsapp_session.json`
  - `[x]` Route inbound WhatsApp messages through `AIAgent`; send response back
  - `[x]` Voice memo: pipe audio attachment through `AudioProcessor::transcribe`, treat result as text message
- `[x]` Slack live gateway
  - `[x]` Implement Events API webhook handler (uses existing `athena-gateway` webhook infra)
  - `[x]` Handle `app_mention` and DM events
  - `[x]` Interactive approval prompts via Slack Block Kit buttons (yes/no tool approval)
  - `[x]` Slash command routing (`/athena <prompt>`)
- `[x]` Discord gateway
  - `[x]` Add `serenity` or `twilight` crate to `athena-gateway`
  - `[x]` Handle mention + DM events; stream response as message edits
- `[x]` Cross-platform cron delivery
  - `[x]` After a cron job completes, route its output to the user's configured home platform
  - `[x]` Per-job `delivery: [telegram, slack, discord]` config field

---

# Phase 19: Prompt Caching & Context Optimization (New)

- `[ ]` Anthropic prompt caching
  - `[ ]` In `AnthropicTransport`, annotate the system prompt block with `"cache_control": {"type": "ephemeral"}`
  - `[ ]` Annotate the tool definitions array's last item with `"cache_control": {"type": "ephemeral"}`
  - `[ ]` Track cache hit/miss from response `usage.cache_read_input_tokens` and surface in `/status`
- `[ ]` Smart context compression (upgrade from Phase 5 truncation)
  - `[ ]` When token count exceeds `compression_threshold` (e.g. 80% of model's context window):
    - `[ ]` Identify the oldest N assistant+tool turns
    - `[ ]` Call the LLM to produce a summary paragraph of those turns
    - `[ ]` Replace those turns with a single synthetic `assistant` message containing the summary
  - `[ ]` Preserve all `user` turns verbatim (never compress user messages)
  - `[ ]` Unit test: verify compressed context is smaller and retains key facts via LLM self-check

---

# Phase 20: Dashboard — Full Implementation (New)

- `[x]` Replace static HTML stub with a functional web UI
  - `[x]` WebSocket server in `athena-server` (or `athena-cli`) bridges the agent over WS
  - `[x]` React frontend connects to the WebSocket bridge
  - `[x]` Live token streaming: gateway pushes `token_delta` events; frontend appends to transcript
  - `[x]` Session sidebar: list sessions, click to load
  - `[x]` Tool activity feed: show active tool calls + results in a side panel
  - `[x]` Settings panel: model selector, toolset toggles, memory viewer
- `[x]` `athena dashboard` subcommand — launch the bridge + open browser
- `[x]` Native Windows support (PTY fallback since POSIX PTY unavailable)

---

# Phase 22: Test Coverage for New Phases (New)

- `[x]` Phase 16 (Learning Loop) — unit tests for skill synthesis prompt, memory nudge, FTS5 search, trajectory export
- `[x]` Phase 17 (Context Files) — tests for AGENTS.md discovery (mock filesystem), memory injection
- `[x]` Phase 18 (Gateways) — integration tests for WhatsApp bridge lifecycle, Slack event routing
- `[x]` Phase 19 (Caching) — assert `cache_control` fields present in serialized Anthropic request; mock cache-hit response
- `[x]` Phase 20 (Dashboard) — WebSocket round-trip integration test; React component unit tests
