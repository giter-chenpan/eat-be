# OpenRouter Chat Integration — Design Spec

**Date:** 2026-06-16
**Status:** Draft
**Target file:** `src/api/chat.rs` (currently empty)

## Goal

Add a chat API to the `eatbe` backend that streams LLM responses from OpenRouter. Sessions and messages are persisted in the database. Endpoints are JWT-authenticated and rate-limited per user.

## Scope

- One new module: `src/api/chat.rs` (full CRUD for chat sessions/messages, one streaming send endpoint)
- One new top-level module: `src/openrouter.rs` (thin HTTP client wrapping `reqwest`)
- Two new SeaORM entities: `chat_sessions`, `chat_messages`
- One new migration: `m2025XXXX_create_chat_tables` (auto-discovered by `Migrator`)
- Config additions: `openrouter_api_key`, `openrouter_model`, `chat_rate_limit_per_minute`
- `Rocket.toml` / env-var wiring for the above

## Non-goals

- System-prompt customization (the cooking domain prompt is hardcoded in the module)
- Client-selectable model (model is fixed by config)
- Tool/function calling, vision input, JSON-mode response shapes
- A frontend UI (this is backend only)
- Unit/integration test suite (the project has no test convention; verification is manual smoke test)

## Architecture

### Components

- **`src/api/chat.rs`** — Rocket routes for the chat endpoints. Streams the assistant reply back to the client as SSE while concurrently persisting user + assistant messages to the DB.
- **`src/openrouter.rs`** (new) — `pub async fn stream_chat(req: OpenRouterRequest) -> Result<reqwest::Response, OpenRouterError>`. Owns the HTTP call to `https://openrouter.ai/api/v1/chat/completions` with `stream: true`. Returns the raw response so the caller can pipe its body.
- **`entity/src/chat_session.rs`**, **`entity/src/chat_message.rs`** (new) — SeaORM entities. UUIDv7 primary keys (consistent with the rest of the project).
- **Migration** — adds the two tables with the indexes below.

### Data flow for `POST /api/chat/sessions/<id>/messages` (streaming send)

1. Auth via `Claims` (JWT, mandatory). Increment Redis rate-limit counter; reject with `Rep` JSON if over the per-minute cap.
2. Load the session row by `id`. Return 404 if not found OR `user_id != claims.sub`.
3. Load the last 20 messages for the session, ordered by `created_at ASC` (filter to `role IN ('user','assistant')` — defensive, in case any `system` rows exist).
4. Build the OpenRouter request: messages = `[system, ...history, new user msg]`, `stream: true`, model = configured default.
5. Issue the request. On HTTP/connect/timeout failure, return JSON `Rep` error immediately and do **not** persist either the user or the assistant message (we don't know if the LLM ever saw the request).
6. On success, persist the new user message to `chat_messages` with `status='complete'`, then insert a "pending" assistant message row (`status='pending'`, empty content).
7. Open the SSE response to the client (`Content-Type: text/event-stream`, `Cache-Control: no-cache`).
8. Concurrently `tokio::spawn` a task that consumes the OpenRouter `reqwest::Response` byte stream:
   - For each SSE event, append the `delta.content` to an accumulator, and forward the chunk to the client as `data: <delta>\n\n`.
   - On stream end, `UPDATE` the pending assistant row with full content + `status='complete'`, then emit `data: [DONE]\n\n` to the client.
   - On stream error, mark the row `status='failed'` with the error text, emit `data: {"error":"..."}\n\n` then `data: [DONE]\n\n`.
9. The handler `return`s the Rocket `Stream<Bytes>` so the framework takes over driving it.

The DB write side runs concurrently with the stream, so the user sees the first token as soon as OpenRouter produces it. The handler orchestrates both sides; the `tokio::spawn` owns the write side and the response stream is the read side.

## Data model

```sql
CREATE TABLE chat_sessions (
    id          TEXT PRIMARY KEY,           -- UUIDv7
    user_id     TEXT NOT NULL,              -- sub from JWT
    title       TEXT NOT NULL,              -- first user message, truncated to 50 chars
    created_at  TEXT NOT NULL,              -- RFC3339
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_chat_sessions_user_updated
    ON chat_sessions(user_id, updated_at DESC);

CREATE TABLE chat_messages (
    id                TEXT PRIMARY KEY,     -- UUIDv7
    session_id        TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role              TEXT NOT NULL,        -- 'user' | 'assistant'
    content           TEXT NOT NULL,
    status            TEXT NOT NULL,        -- 'complete' | 'pending' | 'failed'
    error             TEXT,                 -- nullable; set when status='failed'
    prompt_tokens     INTEGER,
    completion_tokens INTEGER,
    created_at        TEXT NOT NULL         -- RFC3339
);

CREATE INDEX idx_chat_messages_session_created
    ON chat_messages(session_id, created_at);
```

## API surface

All endpoints require a valid JWT (`Claims` extractor). Standard `Rep<T>` JSON response envelope (see existing `translation.rs` and `category.rs`).

| Method | Path | Body | Response | Notes |
|---|---|---|---|---|
| `POST` | `/api/chat/sessions` | `{ "title": "optional" }` | `Rep<ChatSession>` | Auto-title from first user message if not provided |
| `GET` | `/api/chat/sessions?page=1&page_size=20` | — | `Rep<ChatSessionListRep>` | Paginated, ordered by `updated_at DESC`. Scoped to caller. |
| `GET` | `/api/chat/sessions/<id>` | — | `Rep<ChatSession>` | 404 if not owner |
| `DELETE` | `/api/chat/sessions/<id>` | — | `Rep<()>` | Cascade deletes messages |
| `GET` | `/api/chat/sessions/<id>/messages?page=1&page_size=50` | — | `Rep<ChatMessageListRep>` | Paginated, oldest-first |
| `POST` | `/api/chat/sessions/<id>/messages` | `{ "content": "..." }` | SSE stream | Streams assistant reply; persists user + assistant messages |

The streaming endpoint's response is `text/event-stream`, not the standard `Rep<T>` envelope.

### SSE event format

```
data: {"delta": "First chunk "}

data: {"delta": "of the reply."}

data: [DONE]
```

On error mid-stream:

```
data: {"error": "upstream connection lost"}

data: [DONE]
```

## Configuration

### `Rocket.toml` (and env-var overrides)

```toml
openrouter_api_key = "..."               # OVERRIDE via env OPENROUTER_API_KEY
openrouter_model = "openai/gpt-4o-mini"   # OVERRIDE via env OPENROUTER_MODEL
chat_rate_limit_per_minute = 20           # OVERRIDE via env CHAT_RATE_LIMIT_PER_MINUTE
```

`AppConfig::new()` reads these via `Config::figment().extract_inner::<...>()` — same pattern as the existing `translation_url`. Failure to set `OPENROUTER_API_KEY` causes startup to fail with a clear `expect` message.

### System prompt

Hardcoded at module level in `src/api/chat.rs`:

```rust
const SYSTEM_PROMPT: &str = "你是一个友好的烹饪助手,擅长根据用户的问题和已有的对话历史,给出实用、简洁的中文回答。";
```

Server-injected, not client-controllable. Client-supplied content is treated purely as user role.

## Error handling

### Categories

1. **Auth / validation / rate-limit** (no stream started yet) — standard `Rep<()>` JSON with codes from the existing `Code` enum (`Unauthorized`, `BadRequest`, `BusinessError`).

2. **OpenRouter pre-stream failure** (non-2xx, network error, timeout before first response) — same JSON shape. Neither the user message nor the assistant message is persisted (we don't know if the LLM ever saw the request). If OpenRouter returns a 200 with an error in the first SSE event, that's treated as a mid-stream failure (case 3) — the user and pending assistant rows are persisted, the assistant row is then updated to `status='failed'`.

3. **Mid-stream failure** (connection drop, partial response) — stream is already open. Emit a final `data: {"error": "..."}\n\n` event followed by `data: [DONE]\n\n`. Mark the assistant row `status='failed'` with whatever content was accumulated and the error text.

4. **Client disconnect** — OpenRouter stream ends without a clean terminal event. Persist accumulated content as `status='complete'` (best-effort) or `status='failed'` with `error="client disconnected"` if nothing was accumulated.

5. **Missing config at startup** — fail loudly in `AppConfig::new()` with a clear `expect("OPENROUTER_API_KEY env var must be set")`. Same pattern as the existing `translation_url`.

6. **Redis rate-limit failure** — fail open (allow the request), log a warning. Rate limiting is best-effort protection; chat should not break because Redis is down.

### DB write failures

Best-effort. The final `UPDATE` of the assistant message is a side effect, not part of the user-visible response contract. If it fails, log the error and continue. The user already got their answer streamed back; surfacing a DB error after the fact would be confusing.

## Verification plan

No automated tests. Manual smoke test:

1. **Compile check** — `cargo build` in the workspace. Must pass.
2. **Create** — `POST /api/chat/sessions` returns a session.
3. **List** — `GET /api/chat/sessions` includes the new session.
4. **Stream** — `POST /api/chat/sessions/<id>/messages` with `curl -N` produces a stream of `data: {...}` events ending with `data: [DONE]`.
5. **Persist** — `GET /api/chat/sessions/<id>/messages` shows both the user message and the complete assistant message.
6. **Delete** — `DELETE /api/chat/sessions/<id>` returns 200; subsequent GET returns 404.
7. **Auth boundary** — request with another user's session_id returns 404 (not 403, to avoid leaking existence).
8. **Rate limit** — 21 requests in one minute; the 21st returns the rate-limit error.
9. **OpenRouter failure** — with a bad API key, the send endpoint returns JSON error, not a stream, and no message rows are persisted (pre-stream failure case).

## Open questions

None at draft time.
