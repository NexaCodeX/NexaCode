# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

NexaCode is an AI coding-assistant desktop app: **Tauri 2 + React 19 (TypeScript/Vite) frontend** driving a **Rust backend** that runs an agentic tool-using loop against OpenAI-compatible / Anthropic LLMs. UI text and many code comments are in Chinese.

## Commands

```bash
npm install              # install frontend deps (Rust deps fetch on first build)

npm run tauri:dev        # PRIMARY dev command — runs the full desktop app (Vite + Rust, hot reload)
npm run dev              # Vite only. The UI loads but every Tauri `invoke` fails — almost nothing works. Rarely useful.
npm run build            # tsc -b && vite build  (use this to typecheck the frontend)
npm run lint             # eslint
npm run tauri:build      # production bundle → crates/nexacode-desktop/target/release/bundle/

cargo build                              # build the Rust workspace
cargo test                               # all Rust tests
cargo test -p nexacode-core              # one crate
cargo test -p nexacode-core test_name    # one test by name substring
```

There is no frontend test runner. To verify a change end-to-end you generally need `npm run tauri:dev`.

## Architecture

Two layers, bridged only by Tauri commands + events. Keep business logic in the core crate, not in the Tauri shell.

### Rust: `crates/`
- **`nexacode-core`** — pure, UI-agnostic logic. No Tauri dependency.
  - `agent/mod.rs` — the agent loop (see below).
  - `llm/` — provider abstraction. `LLMClient` + `providers/` (`openai`, `anthropic`, `openai_compatible`). The streaming path is `chat_stream_with_tools` → yields `StreamChunk { delta, tool_call_delta, finish_reason }`.
  - `tools/` — every capability the agent has. `registry.rs::default_registry()` is the canonical list (Read, Write, Ls, Grep, Glob, Edit, MultiEdit, Bash, WebFetch, Diagnostic, Task, CodeGraph). Each tool implements the `Tool` trait (`name` / `parameters` / `validate` / `execute` / `requires_confirmation` / `risk_level`). **Add a new tool by implementing `Tool` and registering it in `default_registry()`.**
  - `session/` — `SessionStorage` (persisted chats) + `SessionLogger` (per-run debug log).
- **`nexacode-desktop`** — the Tauri shell. `lib.rs::run()` registers ALL `#[tauri::command]`s and manages three shared states: `LLMManager`, `ToolState` (holds the `ToolRegistry` + a `ToolContext` whose `working_dir` is global app state), `AgentState` (cancellation flag). Commands live in `llm/commands.rs`, `tools/mod.rs`, `agent/mod.rs`. This crate mostly translates between core types and frontend-facing serde types and emits events — put real logic in core.

### Frontend: `src/`
- `services/llm.ts` — the ONLY place that calls Tauri `invoke` / `listen`. Two classes: `LLMService` (plain chat + streaming) and `AgentService` (agent loop). Streaming results arrive as Tauri **events** (`chat-chunk`/`chat-end`, `agent-event`/`agent-end`), not return values.
- `hooks/useLLM.ts` / `hooks/useAgent.ts` — wrap those services into React state. `useAgent` translates the raw `agent-event` stream into an ordered `AgentStep[]` (thinking / tool_call / tool_result), linking results to calls by tool-call id.
- `App.tsx` — top-level state, session list, and the two **chat modes**: `'build'` (full agent loop with tools) vs `'chat'` (plain LLM streaming, no tools).
- Components: `AgentStep` / `ToolCallView` / `DiffPreview` render the agent trace; `CodeGraphView` and `Terminal` are the bottom utility panels; `MarkdownRenderer` handles markdown + thinking parsing.
- Styles: SCSS partials in `src/styles/`, imported via `main.scss`. Design tokens live in `_variables.scss` (a **warm light theme**); the Terminal/CodeGraph panels are intentionally dark.

### The agent loop (the core of the app)
`AgentLoop::run_streaming` in `crates/nexacode-core/src/agent/mod.rs` is a **single ReAct loop — one agent, one shared message context, no sub-agents**:

1. Stream the LLM with the full tool set. Text deltas are emitted as `Thinking` events; tool-call deltas are accumulated.
2. No tool calls → emit `Completed` and return.
3. Otherwise execute each tool **serially** via `ToolRegistry::execute`, emit `ToolCall` + `ToolResult`, append results to the message list, loop (up to `max_iterations` → `MaxIterationsReached`).

The `Task` tool is a to-do/task-management tool, **not** an agent spawner. There is no orchestrator/worker split yet.

## Conventions & gotchas

- **Camel/snake boundary:** Rust session types use `#[serde(rename_all = "camelCase")]`. Runtime agent types in TS are snake_case (matching event payloads), but persisted session JSON is camelCase. `App.tsx` has explicit `stepToSessionData` / `sessionDataToStep` converters — keep them in sync when changing step shape.
- **Reasoning vs. content:** the OpenAI provider wraps model `reasoning_content` as `[THINKING]...[/THINKING]` inline in the text delta stream. `MarkdownRenderer.parseThinking` (recognizes `[THINKING]`, `<think>`, `<THINK>`) is what splits reasoning from the user-facing answer on the frontend. Treat `step.thinking` as possibly containing BOTH.
- **Edit backups & rollback:** before any file edit, tools snapshot the original to `~/.nexacode/backups/{session_id}/`. The UI "Undo Changes" button → `agent_rollback` command → `tools::backup::rollback_session` restores them. Don't bypass the backup path when adding file-mutating tools.
- **State on disk (all under `~/.nexacode/`):** `config.toml` (LLM providers, see `config.example.toml`), `sessions/` (chat history), `logs/{session_id}.log` (per-run agent logs), `backups/{session_id}/`.
- **Working directory is global**, held in `ToolState.context.working_dir` and set via the `tool_set_working_dir` command. File tools resolve relative paths against it.
- **CodeGraph** is a SQLite-backed (`rusqlite`) symbol/call-graph index. The frontend drives it through the generic `tool_execute` command with `{ name: 'CodeGraph', args: { action: 'index' | 'list_nodes' | 'get_call_hierarchy' | 'get_file_dependencies', ... } }`.

## Reference docs

`README.md`, `LLM_INTEGRATION.md`, `OPENAI_COMPATIBLE.md`, `QUICK_START.md`, `TEST_LLM.md`, and `.nexacode-roadmap.md` cover provider setup and roadmap in more depth.
