# Athena Implementation Plan — Phases 16–22

Phases 1–15 are complete. This document covers the remaining work needed to reach full feature parity with hermes-agent's most distinctive capabilities: the closed learning loop, full messaging gateway breadth, prompt caching, functional dashboard, and workspace-aware context injection.

---

## Open Questions (Decide Before Starting)

> **Q1 — TUI Gateway vs. native Ratatui TUI?**
> Phase 5 left `athena-tui-gateway` as a stub. The Python original spawns a Node.js Ink process over JSON-RPC stdio. Two paths forward:
> - **Option A (JSON-RPC bridge):** Finish the stdio JSON-RPC server so the existing Node.js Ink frontend in `apps/` can attach. Preserves the existing frontend investment.
> - **Option B (native Ratatui):** Rewrite the TUI in Rust using `ratatui` + `crossterm`. Eliminates the Node.js dependency entirely; better fits the Rust-first architecture.
>
> Recommendation: Option B unless you need the existing Ink frontend for the dashboard. Option A is required for Phase 20.

> **Q2 — WhatsApp bridge: Node.js companion or native Rust library?**
> The Python version spawns a Node.js `whatsapp-web.js` process. A native Rust WhatsApp client library does not exist with full multi-device support. Options:
> - **Option A:** Keep the Node.js companion approach; spawn it from `athena-gateway` as a managed child process.
> - **Option B:** Skip WhatsApp for now; prioritize Discord and Slack which have first-class Rust crates.

> **Q3 — Honcho integration: blocking or stretch goal?**
> Honcho requires an external service call. Recommend treating Phase 21 as strictly optional for now and building the memory nudge system (Phase 16) first as a simpler local substitute.

---

## Phase 16: Closed Learning Loop

This is the highest-priority gap. It spans `athena-skills`, `athena-state`, `athena-agent`, and `athena-cli`.

### 16a. Autonomous Skill Creation

**File:** `athena-skills/src/synthesis.rs` (new)

After the agent completes a run where it made ≥ `skill_synthesis_threshold` tool calls (configurable, default 4), trigger skill synthesis:

1. Build a synthesis prompt: include the full message history, then ask the model to distill the successful approach into a named, reusable skill with a markdown body.
2. Call the active provider with the synthesis prompt (use a small, cheap model if configured).
3. Parse the response to extract `name`, `description`, and `body` fields.
4. Run a deduplication check: embed the new skill's description and compare cosine similarity against all stored skills. If max similarity > `dedup_threshold` (default 0.92), skip storage.
5. Run a quality gate: ask the model "Is this skill reusable and generally applicable? Answer yes or no." Only store if yes.
6. Store via `SkillStore::insert`.

Hook the trigger into `athena-agent/src/agent.rs` at the end of `run()`, after the final response is produced.

### 16b. Skill Self-Improvement

**File:** `athena-skills/src/improvement.rs` (new)

Track skill usage outcomes:

- Add a `usage_count` and `success_count` column to the `skills` table in `athena-state/src/db.rs`.
- When a skill is retrieved and used in a turn, increment `usage_count`. When the agent's subsequent response is rated positively (heuristic: no error tool result follows), increment `success_count`.
- Periodically (on session end, or every N turns), identify skills with `success_rate < 0.4` and `usage_count >= 5`. For each, re-prompt the model: "This skill has a low success rate. Rewrite it to be more accurate." Update the body in-place and re-embed.

### 16c. Memory Nudge System

**File:** `athena-skills/src/memory.rs` (new)

On session end (triggered in `athena-agent/src/agent.rs` after the loop exits):

1. Build a nudge prompt: include the last N turns of conversation (configurable window), then ask: "Identify up to 3 facts from this conversation worth remembering long-term. Format each as a bullet point."
2. Call the provider. Parse bullet points from the response.
3. Append each fact to `~/.athena/MEMORY.md` with a datestamp.
4. If any facts are about the user specifically (detected by a heuristic: contains "I", "my", "the user"), also append to `~/.athena/USER.md`.

**Injection:** At session start in `athena-agent/src/builder.rs`, load `MEMORY.md` and `USER.md` if they exist and prepend their contents to the system prompt.

### 16d. FTS5 Session Search

**File:** `athena-state/src/search.rs` (new)

The SQLite FTS5 virtual table (`messages_fts`) is already declared in the schema. Wire it up:

1. `search_messages(query: &str, limit: usize) -> Vec<SearchHit>` — run `SELECT session_id, content, snippet(...) FROM messages_fts WHERE messages_fts MATCH ?`.
2. For each matching session, retrieve a configurable number of surrounding messages (the context window for that hit).
3. Call the provider with those messages + "Summarize this conversation in 2–3 sentences." Cache the summary in a new `session_summaries` table.
4. Return `Vec<SearchHit>` containing session ID, timestamp, and the LLM summary.
5. Expose as tool `search_past_conversations(query: str)` registered in `athena-tools`.
6. Expose as `/search <query>` slash command in `athena-cli/src/interactive.rs`.

### 16e. Trajectory Compression & Batch Runner

**File:** `athena-tools/src/trajectory_tool.rs` (new)
**File:** `athena-cli/src/commands/trajectory.rs` (new)
**File:** `athena-cli/src/commands/batch.rs` (new)

Trajectory export:

1. `TrajectoryExporter::export(session_id) -> TrajectoryRecord` — serializes all messages + tool calls + results for a session into a structured JSON object matching the training data schema used by Nous.
2. `TrajectoryCompressor::compress(record) -> CompressedRecord` — strips large tool outputs (truncate at 2000 chars), removes redundant intermediate assistant messages, retains the final answer.
3. CLI: `athena trajectory export [--session <id>] [--output <path>]`

Batch runner:

1. Parse a YAML config: a list of `{prompt, context_files, model, max_turns}` entries.
2. For each entry, spin up an `AIAgent` in headless mode (no CLI prompts), run to completion, capture the trajectory.
3. Write all trajectories to `~/.athena/trajectories/<timestamp>/`.
4. CLI: `athena batch run --config <yaml>`

---

## Phase 17: Context Files & Workspace Context

**File:** `athena-core/src/context_files.rs` (new)
**Modified:** `athena-agent/src/builder.rs`

### 17a. AGENTS.md Loader

Walk up from the current working directory searching for `AGENTS.md`. Also check `~/.athena/AGENTS.md`. Merge all found files (workspace-local takes precedence over home). Inject the merged content as a system prompt segment that sits before the user's SOUL.md content.

```rust
pub fn find_agents_md(cwd: &Path) -> Vec<PathBuf>;
pub fn load_agents_md(cwd: &Path) -> Option<String>;
```

Surface in `athena doctor`: warn if no `AGENTS.md` is found anywhere.

### 17b. MEMORY.md / USER.md Injection

Load at session start, inject into system prompt after AGENTS.md. In `builder.rs`:

```rust
if let Some(memory) = load_memory_file(&athena_home) {
    system_prompt.push_str(&memory);
}
```

Add `athena memory edit` CLI subcommand: open `$EDITOR` (or `nano` as fallback) on `~/.athena/MEMORY.md`.

---

## Phase 18: Full Messaging Gateway Parity

**Modified:** `athena-gateway/src/`

### 18a. WhatsApp Live Bridge

Add a `whatsapp.rs` platform module. On `athena gateway --platform whatsapp`:

1. Locate (or download) the Node.js companion script from `scripts/whatsapp_bridge.js`.
2. Spawn it as a `tokio::process::Child` with a JSON-RPC pipe over stdin/stdout.
3. Handle the pairing code exchange: write pairing code to a configured phone number, wait for the QR/link confirmation event.
4. Persist the authenticated session to `~/.athena/whatsapp_session.json`; restore it on restart.
5. On inbound message event: route through `AIAgent::run()`; post the response back via the bridge's `sendMessage` RPC call.
6. Voice memo handling: if the inbound event has an `audio` attachment, base64-decode it, call `AudioProcessor::transcribe()`, use the transcript as the user message.

### 18b. Slack Live Gateway

Add `slack.rs` platform module. Use the existing webhook infrastructure in `athena-gateway`:

1. Register an HTTP POST handler at `/slack/events` for Slack Events API callbacks.
2. Verify the `X-Slack-Signature` header (HMAC-SHA256 with the signing secret).
3. Handle `app_mention` and `message.im` event types: extract the text, route through `AIAgent`.
4. For long responses, split into ≤3000-char chunks; post each as a Slack message.
5. For tool approval prompts (`yolo_mode: false`), send a Slack Block Kit message with Yes/No buttons. Handle the resulting `block_actions` callback to resume or cancel the agent.
6. Handle `/athena <prompt>` slash command: same routing as a message event.

### 18c. Discord Gateway

Add `discord.rs` platform module using the `serenity` crate:

1. Respond to `@mention` events in any channel and DMs.
2. Stream response tokens as edits to an initial "thinking..." message.
3. Respect the 2000-character Discord message limit; split into thread replies if needed.

### 18d. Cross-Platform Cron Delivery

Modify `athena-cli/src/commands/cron.rs` and `athena-gateway/src/`:

1. Add a `delivery` field to the cron job config: a list of platform names, e.g. `["telegram", "slack"]`.
2. After a cron job's `AIAgent::run()` completes, look up the gateway handle for each delivery target and post the final response.
3. For platform-specific formatting (e.g. Slack markdown vs. plain text), run the response through a simple platform formatter.

---

## Phase 19: Prompt Caching & Context Optimization

**Modified:** `athena-providers/src/anthropic.rs`
**Modified:** `athena-agent/src/context.rs`

### 19a. Anthropic Prompt Caching

In the Anthropic transport, when building the API request body:

1. Wrap the system prompt string in the block format and add `"cache_control": {"type": "ephemeral"}` to it.
2. After the tool definitions array, add `"cache_control": {"type": "ephemeral"}` to the last tool definition entry.
3. Parse `usage.cache_read_input_tokens` and `usage.cache_creation_input_tokens` from the response. Track cumulative savings per session in `athena-state`.
4. Expose cache stats in `/status` output and in `athena insights`.

Estimated cost saving: ~90% on system prompt + tool definition tokens for long sessions.

### 19b. Smart Context Compression

Replace the current truncation logic in `context.rs` with a summarization-based approach:

1. Set `compression_threshold` at 80% of the model's declared context window (read from `athena-core` model registry).
2. When `count_tokens(messages) > compression_threshold`:
   - Identify the oldest `compression_batch_size` (default: 6) assistant + tool turns.
   - Build a compression prompt: include those turns, then ask "Summarize the actions taken and conclusions reached in 3–5 sentences."
   - Replace those N turns with one synthetic `assistant` message: `"[Compressed history] <summary>"`.
   - Keep all `user` turns intact — never compress user messages.
3. After compression, re-check token count. If still over threshold, compress another batch.
4. Log each compression event to `agent.log` with original and reduced token counts.

---

## Phase 20: Dashboard — Full Implementation

**Modified:** `athena-tui-gateway/src/`
**Modified:** `apps/` (React frontend)
**Modified:** `athena-cli/src/commands/dashboard.rs`

### 20a. WebSocket Bridge

In `athena-tui-gateway`, alongside the existing stdio JSON-RPC server, add a WebSocket server (use `tokio-tungstenite`):

1. Listen on `127.0.0.1:8765` (configurable).
2. Accept WebSocket connections from the React frontend.
3. Translate JSON-RPC messages bidirectionally between the WebSocket client and the stdio Ink process (or the Ratatui backend if Option B was chosen in the open question).
4. Push `token_delta`, `tool_call_start`, `tool_call_result`, and `session_change` notifications to all connected WebSocket clients.

### 20b. React Frontend Wiring

In `apps/`:

1. Connect to `ws://127.0.0.1:8765` on load.
2. Transcript pane: append `token_delta` events as streaming text.
3. Session sidebar: call `session/list` on connect; re-fetch on `session_change` events.
4. Tool activity panel: render `tool_call_start` and `tool_call_result` events as collapsible cards.
5. Settings panel: call `config/get` and `config/set` RPC methods; model selector reads from `athena-core` model registry.

### 20c. Dashboard CLI Subcommand

In `athena-cli/src/commands/dashboard.rs`:

1. Start the `athena-tui-gateway` process if not already running.
2. Open `http://127.0.0.1:8000` in the system browser (`open`/`xdg-open`/`start` depending on OS).
3. Print the URL and gateway PID to stdout for debugging.
4. On SIGINT, shut down the gateway process cleanly.

---

## Phase 21: Honcho User Modeling (Stretch Goal)

**File:** `athena-state/src/honcho.rs` (new)

Only implement if Honcho's API is stable and the REST client is straightforward:

1. Add optional `honcho_api_key` to `athena-core/src/config.rs`.
2. On session end, POST the conversation to Honcho's `/apps/{app_id}/users/{user_id}/sessions/{session_id}/messages` endpoint.
3. On session start, GET the user model and inject as an additional system-prompt segment.
4. Gate all Honcho calls behind a feature flag; the agent must work identically without it.

---

## Phase 22: Test Coverage for New Phases

Each new module must ship with tests. Targets:

| Crate / Module | Target Coverage | Test Strategy |
|---|---|---|
| `athena-skills/synthesis.rs` | >85% | Mock the LLM provider; assert synthesis prompt contains full history; assert dedup skips near-duplicate skills |
| `athena-skills/improvement.rs` | >80% | Inject known-bad skills into a test DB; verify rewrite prompt is called after threshold |
| `athena-skills/memory.rs` | >85% | Mock provider; assert facts are appended to a temp MEMORY.md; verify datestamp format |
| `athena-state/search.rs` | >90% | Use an in-memory SQLite DB with FTS5; verify `MATCH` query returns correct hits; mock provider for summary |
| `athena-tools/trajectory_tool.rs` | >90% | Construct a fake session; assert exported JSON matches schema; assert compressed version is smaller |
| `athena-core/context_files.rs` | >95% | Mock filesystem with `tempfile`; verify walk-up logic, precedence, and merge behavior |
| `athena-providers/anthropic.rs` (cache) | >95% | Deserialize the request body; assert `cache_control` fields present; mock cache-hit response and verify stat tracking |
| `athena-agent/context.rs` (smart compress) | >90% | Feed a mock long conversation; assert old turns are replaced with summary; assert user turns are preserved |
| `athena-gateway/slack.rs` | >80% | Mock the Slack Events API; verify signature validation; verify `app_mention` routes to AIAgent |
| `athena-gateway/discord.rs` | >75% | Use `serenity`'s mock framework; verify mention detection and response chunking |
| `athena-tui-gateway` (WebSocket) | >80% | Use `tokio-tungstenite` test client; send RPC message, assert `token_delta` events received |

All new integration tests that touch the network must use `wiremock` for HTTP or mock processes for stdio-based protocols. No live API calls in CI.
