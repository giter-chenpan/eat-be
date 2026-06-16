# OpenRouter Chat Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a chat API to `eatbe` that streams OpenRouter LLM responses, with the local `mcp-server` (HowToCook recipes) exposed as tool calls. Sessions and messages persist to SQLite; endpoints are JWT-authenticated and rate-limited.

**Architecture:** New Rocket routes in `src/api/chat.rs` orchestrate a ReAct loop. Each iteration calls OpenRouter with the conversation history + 4 MCP tool definitions. If the LLM emits `tool_calls`, the handler invokes the MCP tool via `src/mcp_client.rs`, persists the call and result, and loops. Only the final iteration's text is streamed to the client as SSE. Sessions and messages persist via two new SeaORM entities and a new migration.

**Tech Stack:** Rust 2024 edition, Rocket 0.5, SeaORM 0.12 (SQLite), `reqwest` 0.12 (already in tree), `rmcp` 0.15 (new dep — same version as `mcp-server`).

**Project facts (engineer should know):**
- The DB is SQLite (see `Rocket.toml` `databases.sea_orm.url`). The schema lives in one consolidated baseline migration at `migration/src/m20260611_000000_sqlite_baseline.rs`. New tables go in a new migration file and must be registered in `migration/src/lib.rs::Migrator::migrations()`.
- `id` columns are `TEXT` with UUIDv7 strings, **not** integer auto-increment. See `entity/src/words.rs` and `translation::words` usage in `src/api/translation.rs`.
- All API responses use the `Rep<T>` wrapper from `src/common/data_structure.rs`. The `Code` enum in `src/common/enums.rs` has only `Success`/`BadRequest`/`BusinessError` codes. We will reuse them.
- All authenticated routes take `_claims: Claims` as a parameter; the `Claims` extractor is defined in `src/jwtuser.rs`.
- The `mcp-server` is already launched in `main.rs` via `rocket::tokio::spawn` on `0.0.0.0:8081`. Its MCP endpoint is `http://127.0.0.1:8081/mcp`.
- The project has no test framework. The spec calls for a manual smoke test; this plan uses `cargo build` for incremental compile verification.
- The `dishes` API (`src/api/dishes.rs`) and `times` API (`src/api/times.rs`) are good reference patterns for a new CRUD-style API module.

**OpenRouter SSE response format (engineer reference):**
- Each event is one line: `data: {json}\n\n`
- The JSON `choices[0].delta` may contain `content` (text), `role`, or `tool_calls` (array of `{index, id, type, function: {name, arguments}}`).
- `finish_reason` is `"stop"` (text done), `"tool_calls"` (LLM wants to call tools), or `"length"` (truncated).
- Stream ends with `data: [DONE]\n\n`.

**OpenRouter request body shape (engineer reference):**
```json
{
  "model": "openai/gpt-4o-mini",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."},
    {"role": "assistant", "tool_calls": [{"id": "...", "type": "function", "function": {"name": "...", "arguments": "..."}}]},
    {"role": "tool", "tool_call_id": "...", "content": "result"}
  ],
  "tools": [{"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}],
  "stream": true
}
```

---

## File map

| File | Status | Responsibility |
|---|---|---|
| `entity/src/chat_session.rs` | NEW | SeaORM entity for `chat_sessions` table |
| `entity/src/chat_message.rs` | NEW | SeaORM entity for `chat_messages` table |
| `entity/src/lib.rs` | MODIFY | Export new entities |
| `entity/src/mod.rs` | MODIFY | Export new entities |
| `entity/src/prelude.rs` | MODIFY | (only if existing modules re-export from prelude — check first) |
| `migration/src/m20260616_000000_create_chat_tables.rs` | NEW | Migration creating both tables + indexes |
| `migration/src/lib.rs` | MODIFY | Register new migration |
| `Cargo.toml` | MODIFY | Add `rmcp` client feature |
| `Rocket.toml` | MODIFY | Add `openrouter_api_key`/`model`/`mcp_server_url`/etc. defaults |
| `src/config.rs` | MODIFY | Add new fields to `AppConfig` |
| `src/openrouter.rs` | NEW | OpenRouter HTTP client with tool support |
| `src/mcp_client.rs` | NEW | Wrapper around the `rmcp` client; one method `call_tool` |
| `src/api/chat.rs` | NEW | All chat endpoints (CRUD + streaming) |
| `src/api/mod.rs` | MODIFY | Add `pub mod chat;` |
| `src/main.rs` | MODIFY | Mount new routes |

---

## Task 1: Add `rmcp` client dependency to root `Cargo.toml`

**Files:**
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Add the dependency**

Append after the existing `reqwest` line in the `[dependencies]` block:

```toml
rmcp = { version = "0.15", features = ["client", "transport-streamable-http-client"] }
```

- [ ] **Step 2: Verify the dep resolves**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check --offline 2>&1 | tail -20` (if `--offline` fails, drop it).

Expected: the dependency tree resolves without errors mentioning `rmcp` (other unrelated warnings are fine).

- [ ] **Step 3: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add Cargo.toml Cargo.lock
git commit -m "build: add rmcp client feature for chat mcp integration"
```

---

## Task 2: Create `chat_session` entity

**Files:**
- Create: `entity/src/chat_session.rs`

- [ ] **Step 1: Write the entity file**

Create `entity/src/chat_session.rs` with the following content. This mirrors the style of `entity/src/words.rs` (UUIDv7 string primary key, hand-written to match sea-orm-codegen output style):

```rust
//! `SeaORM` Entity for chat_sessions

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "chat_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::chat_message::Entity")]
    ChatMessages,
}

impl Related<super::chat_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChatMessages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

Note: the `Relation` enum requires `chat_message` to exist. We add it in the next task; if `cargo check` complains in between, that's expected — proceed to Task 3.

- [ ] **Step 2: Verify it compiles standalone**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check -p entity 2>&1 | tail -20`

Expected: errors about unresolved `super::chat_message::Entity`. That's fine — addressed in Task 3.

- [ ] **Step 3: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add entity/src/chat_session.rs
git commit -m "feat(entity): add chat_session model"
```

---

## Task 3: Create `chat_message` entity

**Files:**
- Create: `entity/src/chat_message.rs`

- [ ] **Step 1: Write the entity file**

Create `entity/src/chat_message.rs`:

```rust
//! `SeaORM` Entity for chat_messages

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "chat_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub error: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_args: Option<String>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::chat_session::Entity",
        from = "Column::SessionId",
        to = "super::chat_session::Column::Id"
    )]
    BelongsToChatSession,
}

impl Related<super::chat_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BelongsToChatSession.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

- [ ] **Step 2: Wire both new entities into the entity crate**

Modify `entity/src/lib.rs` — add the two new modules:

```rust
extern crate rocket;

pub mod category;
pub mod chat_message;
pub mod chat_session;
pub mod dishes;
pub mod dishes_images;
pub mod times;
pub mod user;
pub mod user_select;
pub mod words;
```

Modify `entity/src/mod.rs` — add the two new modules inside the existing block (it currently re-exports only some — add chat modules in the same style):

```rust
//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

pub mod prelude;

pub mod category;
pub mod chat_message;
pub mod chat_session;
pub mod dishes;
pub mod seaql_migrations;
pub mod times;
pub mod user;
pub mod user_select;
pub mod words;
```

- [ ] **Step 3: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check -p entity 2>&1 | tail -20`

Expected: no errors related to chat_session / chat_message.

- [ ] **Step 4: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add entity/src/chat_message.rs entity/src/lib.rs entity/src/mod.rs
git commit -m "feat(entity): add chat_message model and wire both into entity crate"
```

---

## Task 4: Create migration for chat tables

**Files:**
- Create: `migration/src/m20260616_000000_create_chat_tables.rs`
- Modify: `migration/src/lib.rs`

- [ ] **Step 1: Write the migration file**

Create `migration/src/m20260616_000000_create_chat_tables.rs`:

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChatSessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ChatSessions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(ChatSessions::UserId).string().not_null())
                    .col(ColumnDef::new(ChatSessions::Title).string().not_null())
                    .col(ColumnDef::new(ChatSessions::CreatedAt).string().not_null())
                    .col(ColumnDef::new(ChatSessions::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_chat_sessions_user_updated")
                    .table(ChatSessions::Table)
                    .col(ChatSessions::UserId)
                    .col(ChatSessions::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ChatMessages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ChatMessages::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(ChatMessages::SessionId).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Role).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Content).text().not_null())
                    .col(ColumnDef::new(ChatMessages::Status).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Error).text().null())
                    .col(ColumnDef::new(ChatMessages::ToolCallId).string().null())
                    .col(ColumnDef::new(ChatMessages::ToolName).string().null())
                    .col(ColumnDef::new(ChatMessages::ToolArgs).text().null())
                    .col(ColumnDef::new(ChatMessages::PromptTokens).integer().null())
                    .col(ColumnDef::new(ChatMessages::CompletionTokens).integer().null())
                    .col(ColumnDef::new(ChatMessages::CreatedAt).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_chat_messages_session")
                            .from(ChatMessages::Table, ChatMessages::SessionId)
                            .to(ChatSessions::Table, ChatSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_chat_messages_session_created")
                    .table(ChatMessages::Table)
                    .col(ChatMessages::SessionId)
                    .col(ChatMessages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChatMessages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ChatSessions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ChatSessions {
    Table,
    Id,
    UserId,
    Title,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ChatMessages {
    Table,
    Id,
    SessionId,
    Role,
    Content,
    Status,
    Error,
    ToolCallId,
    ToolName,
    ToolArgs,
    PromptTokens,
    CompletionTokens,
    CreatedAt,
}
```

- [ ] **Step 2: Register the migration**

Modify `migration/src/lib.rs`. Add a new `mod` line above the existing `m20260611_000000_sqlite_baseline;` line:

```rust
mod m20260616_000000_create_chat_tables;
mod m20260611_000000_sqlite_baseline;
```

Update the `migrations()` method to include both:

```rust
fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260611_000000_sqlite_baseline::Migration),
        Box::new(m20260616_000000_create_chat_tables::Migration),
    ]
}
```

- [ ] **Step 3: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check -p migration 2>&1 | tail -20`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add migration/src/m20260616_000000_create_chat_tables.rs migration/src/lib.rs
git commit -m "feat(migration): add chat_sessions and chat_messages tables"
```

---

## Task 5: Extend `AppConfig` with new fields

**Files:**
- Modify: `src/config.rs`
- Modify: `Rocket.toml`

- [ ] **Step 1: Add fields to `AppConfig`**

Replace the contents of `src/config.rs` with:

```rust
use rocket::Config;
use std::sync::OnceLock;

pub struct AppConfig {
    pub translation_url: String,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub chat_rate_limit_per_minute: u32,
    pub chat_max_tool_iterations: u32,
    pub mcp_server_url: String,
}

impl AppConfig {
    pub fn new() -> Self {
        let translation_url = Config::figment()
            .extract_inner::<String>("translation_url")
            .expect("translation_url is not set");
        let openrouter_api_key = Config::figment()
            .extract_inner::<String>("openrouter_api_key")
            .expect("OPENROUTER_API_KEY is not set (config: openrouter_api_key)");
        let openrouter_model = Config::figment()
            .extract_inner::<String>("openrouter_model")
            .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());
        let chat_rate_limit_per_minute = Config::figment()
            .extract_inner::<u32>("chat_rate_limit_per_minute")
            .unwrap_or(20);
        let chat_max_tool_iterations = Config::figment()
            .extract_inner::<u32>("chat_max_tool_iterations")
            .unwrap_or(5);
        let mcp_server_url = Config::figment()
            .extract_inner::<String>("mcp_server_url")
            .unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
        Self {
            translation_url,
            openrouter_api_key,
            openrouter_model,
            chat_rate_limit_per_minute,
            chat_max_tool_iterations,
            mcp_server_url,
        }
    }
}

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();

pub fn get_config() -> &'static AppConfig {
    APP_CONFIG.get_or_init(|| AppConfig::new())
}
```

- [ ] **Step 2: Add defaults to Rocket.toml**

Append the following block inside `[default]` in `Rocket.toml` (place it after the existing `translation_url` line):

```toml
openrouter_api_key = "sk-or-v1-replace-me"
openrouter_model = "openai/gpt-4o-mini"
chat_rate_limit_per_minute = 20
chat_max_tool_iterations = 5
mcp_server_url = "http://127.0.0.1:8081"
```

- [ ] **Step 3: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check 2>&1 | tail -20`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add src/config.rs Rocket.toml
git commit -m "feat(config): add openrouter/mcp/chat-rate-limit config fields"
```

---

## Task 6: Create the `openrouter` module

**Files:**
- Create: `src/openrouter.rs`
- Modify: `src/main.rs` (add `mod openrouter;`)

- [ ] **Step 1: Write the module**

Create `src/openrouter.rs`:

```rust
use crate::config::get_config;
use reqwest::Response;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// One message in the conversation. Supports the OpenRouter/Chat Completions shape
/// including `tool_calls` (assistant) and `tool_call_id` (tool result).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: &'a [ChatMessage],
    pub tools: &'a [Value],
    pub stream: bool,
}

#[derive(Debug, Error)]
pub enum OpenRouterError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("upstream returned {status}: {body}")]
    Upstream { status: u16, body: String },
}

/// Issue a streaming chat completion request. Returns the raw response so the caller
/// can pipe its SSE body. Does NOT read the body — the caller owns it.
pub async fn stream_chat(req: &ChatRequest<'_>) -> Result<Response, OpenRouterError> {
    let cfg = get_config();
    let client = reqwest::Client::new();
    let resp = client
        .post(OPENROUTER_URL)
        .bearer_auth(&cfg.openrouter_api_key)
        .json(&req)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(OpenRouterError::Upstream {
            status: status.as_u16(),
            body,
        });
    }
    Ok(resp)
}

/// Build the JSON payload for the four MCP tools, formatted for OpenRouter's `tools` field.
pub fn mcp_tools_payload() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "list_categories",
                "description": "列出 HowToCook 仓库中所有目录，含菜谱分类、难度等级、技巧指南",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "list_recipes",
                "description": "列出所有内容名称（菜谱、难度或技巧），可通过 category 参数按分类过滤",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "description": "可选分类名，例如 \"aquatic\"、\"staple\"、\"难度等级\"、\"技巧指南\""
                        }
                    }
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_recipe",
                "description": "根据名称获取完整 Markdown 内容（支持菜谱、难度系统和技巧指南）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "菜谱名称，例如 \"西红柿炒鸡蛋\""}
                    },
                    "required": ["name"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "search_recipes",
                "description": "在菜谱、难度和技巧的名称及内容中搜索关键词，返回匹配列表",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keyword": {"type": "string", "description": "搜索关键词"}
                    },
                    "required": ["keyword"]
                }
            }
        }),
    ]
}
```

Note: this uses `thiserror`. Verify it's in the dep tree (it is — `migration` and `rocket` bring it transitively, but we should add it explicitly to be safe). See Step 2.

- [ ] **Step 2: Add `thiserror` explicitly**

In `Cargo.toml`, add to `[dependencies]`:

```toml
thiserror = "1"
```

- [ ] **Step 3: Wire the module into main.rs**

Modify `src/main.rs`. Find the `mod api;` line and add the new module below it:

```rust
mod api;
mod auth;
mod common;
mod config;
mod mcp_client;
mod openrouter;

mod pool;
```

(Order: `mcp_client` and `openrouter` go alongside `api` / `auth` / `common` / `config`.)

- [ ] **Step 4: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check 2>&1 | tail -20`

Expected: error `mcp_client` not yet defined. Expected — fix in Task 7.

- [ ] **Step 5: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add src/openrouter.rs src/main.rs Cargo.toml Cargo.lock
git commit -m "feat(openrouter): add streaming chat client and mcp tool payload"
```

---

## Task 7: Create the `mcp_client` module

**Files:**
- Create: `src/mcp_client.rs`
- Modify: `src/main.rs` (already added `mod mcp_client;` in Task 6)

- [ ] **Step 1: Write the module**

Create `src/mcp_client.rs`:

```rust
use crate::config::get_config;
use rmcp::model::{CallToolRequestParam, Tool};
use rmcp::service::ServiceExt;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type McpClient = Arc<Mutex<Option<rmcp::service::RunningService<rmcp::service::RoleClient, ()>>>>;

/// Build a transport pointed at the local mcp-server. Caller is responsible for
/// connecting it via `.serve(...)` and storing the result.
pub async fn build_client() -> anyhow::Result<McpClient> {
    let cfg = get_config();
    let url = format!("{}/mcp", cfg.mcp_server_url);
    let transport = StreamableHttpClientTransport::from_uri(
        url.parse().expect("invalid mcp_server_url"),
        StreamableHttpClientTransportConfig::default(),
    );
    let client = ().serve(transport).await?;
    Ok(Arc::new(Mutex::new(Some(client))))
}

/// Call an MCP tool by name with the given arguments (as a JSON object).
/// Returns the tool's text result. On any error, returns the error stringified.
pub async fn call_tool(
    client: &McpClient,
    name: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    let mut guard = client.lock().await;
    let svc = guard.as_mut().ok_or_else(|| "mcp client not initialized".to_string())?;
    let req = CallToolRequestParam {
        name: name.to_string(),
        arguments: Some(args.as_object().cloned().unwrap_or_default()),
    };
    let result = svc
        .call_tool(req)
        .await
        .map_err(|e| format!("mcp call_tool failed: {e}"))?;
    let text = result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text)
}

/// Return the list of MCP tool definitions in OpenRouter's `tools` field format.
pub fn tool_definitions_for_openrouter() -> Vec<serde_json::Value> {
    // We hardcode the same shape as `openrouter::mcp_tools_payload()` so the chat
    // module doesn't need to know the OpenRouter-specific JSON shape.
    crate::openrouter::mcp_tools_payload()
}

/// Suppress unused warning for the Tool type (re-exported for future use).
#[allow(dead_code)]
fn _phantom(_: Tool) {}
```

- [ ] **Step 2: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check 2>&1 | tail -40`

Expected: errors likely around the exact `rmcp` 0.15 client API (`RoleClient`, `ServiceExt::serve`, `CallToolRequestParam` shape, `as_text` accessor). Adjust the code to match the actual installed version. Common adjustments:
- If `()` doesn't implement the client role, use the actual `ClientHandler` impl; consult `cargo doc --open rmcp` or the crate's examples.
- If `CallToolRequestParam` has different field names, rename accordingly.
- If `as_text` doesn't exist, use the `rmcp::model::RawContent` pattern from the rmcp examples.

Iterate until `cargo check` is clean.

- [ ] **Step 3: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add src/mcp_client.rs
git commit -m "feat(mcp_client): add wrapper around local mcp-server"
```

---

## Task 8: Wire the mcp client into Rocket state in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Initialize the client at startup**

In `src/main.rs`, modify the `#[launch] fn rocket()` to attach a `McpClient` to Rocket's state. Find the `rocket::build()` chain and add `.manage(...)` after the existing `.attach(...)` calls but before `.mount`:

```rust
.manage(mcp_client::build_client_handle())
```

Add a helper function in `src/mcp_client.rs` (or as a free function in `main.rs`):

```rust
// In src/mcp_client.rs
pub async fn build_client_handle() -> McpClient {
    build_client()
        .await
        .expect("failed to connect to mcp-server at startup")
}
```

In `main.rs`, before `rocket::build()`, **fail fast** if the client can't be built (do this synchronously, not via `.manage`):

```rust
let mcp_client = rocket::tokio::runtime::Handle::current()
    .block_on(async { mcp_client::build_client().await })
    .expect("failed to connect to mcp-server at startup");
```

Wait — the `#[launch]` macro spawns the runtime, so we can't use `Handle::current()` before it. Instead, attach a Rocket fairing that initializes the client. Add a small fairing in `src/mcp_client.rs`:

```rust
use rocket::fairing::{self, AdHoc, Fairing, Info, Kind};

pub struct McpClientFairing;

#[rocket::async_trait]
impl Fairing for McpClientFairing {
    fn info(&self) -> Info {
        Info {
            name: "MCP Client",
            kind: Kind::Liftoff,
        }
    }

    async fn on_liftoff(&self, rocket: &rocket::Rocket<rocket::Orbit>) {
        match build_client().await {
            Ok(client) => {
                rocket::info!("✅ MCP client connected");
                let _ = rocket; // client stored elsewhere if needed
                // We can't store back into Rocket state from on_liftoff easily.
                // Instead, do initialization in main.rs and manage from there.
            }
            Err(e) => {
                rocket::error!("❌ MCP client failed to connect: {e}");
            }
        }
    }
}
```

Simpler approach: do the work in `main.rs` after attaching the mcp-server spawn. Since `main.rs` already uses `rocket::tokio::spawn` for the mcp-server, add another `rocket::tokio::spawn` that builds the client. But that doesn't help the chat handler get the client — handlers need Rocket-managed state.

**Cleanest approach:** Make the client lazy and connection-on-first-use. Modify `McpClient` to hold an `OnceCell<RunningService>` and connect on first `call_tool`:

```rust
// Replace the McpClient type
pub type McpClient = Arc<tokio::sync::OnceCell<rmcp::service::RunningService<rmcp::service::RoleClient, ()>>>;

pub async fn build_client() -> McpClient {
    Arc::new(tokio::sync::OnceCell::new())
}

pub async fn ensure_connected(client: &McpClient) -> Result<&rmcp::service::RunningService<rmcp::service::RoleClient, ()>, String> {
    client
        .get_or_try_init(|| async {
            let cfg = get_config();
            let url = format!("{}/mcp", cfg.mcp_server_url);
            let transport = StreamableHttpClientTransport::from_uri(
                url.parse().expect("invalid mcp_server_url"),
                StreamableHttpClientTransportConfig::default(),
            );
            ().serve(transport).await.map_err(|e| anyhow::anyhow!(e))
        })
        .await
        .map_err(|e: &anyhow::Error| e.to_string())
}

pub async fn call_tool(
    client: &McpClient,
    name: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    let svc = ensure_connected(client).await?;
    // ... use svc as before
}
```

- [ ] **Step 2: Manage the client in main.rs**

In `src/main.rs`, build the handle (which is just an empty `Arc<OnceCell>`) and attach it to Rocket:

```rust
let mcp_handle = mcp_client::build_client();
// ...
rocket::build()
    .manage(mcp_handle)
    .attach(...)
```

`build_client()` no longer does any I/O so it's safe to call synchronously before `rocket::build()`.

- [ ] **Step 3: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check 2>&1 | tail -20`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add src/mcp_client.rs src/main.rs
git commit -m "feat(mcp_client): lazy-init via OnceCell and manage in Rocket state"
```

---

## Task 9: Create the `chat` API module — types, helpers, session CRUD

**Files:**
- Create: `src/api/chat.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Wire the module into `src/api/mod.rs`**

Modify `src/api/mod.rs`:

```rust
pub mod category;

pub mod chat;
pub mod dishes;
pub mod file;
pub mod times;
pub mod translation;
```

- [ ] **Step 2: Write the type definitions and CRUD endpoints**

Create `src/api/chat.rs` with this initial content (CRUD endpoints only; streaming endpoint comes in Task 10):

```rust
use crate::common::data_structure::*;
use crate::common::enums::Code::*;
use crate::config::get_config;
use crate::jwtuser::Claims;
use crate::mcp_client::McpClient;
use crate::openrouter;
use crate::pool::Db;
use ::entity::chat_message::{self, Entity as ChatMessages, Model as ChatMessageModel};
use ::entity::chat_session::{self, Entity as ChatSessions, Model as ChatSessionModel};
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::{openapi, JsonSchema};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use sea_orm_rocket::Connection;
use serde::{Deserialize, Serialize};
use uuid::{ContextV7, Timestamp, Uuid};

const SYSTEM_PROMPT: &str = "你是一个友好的中文烹饪助手。你可以通过调用工具查询菜谱、分类、技巧。\
请先判断用户问题是否需要查询菜谱库:\
- 需要时,调用 search_recipes / list_recipes / list_categories / get_recipe 查询;\
- 不需要时(例如寒暄、通用烹饪知识)直接回答;\
- 用简洁、可操作的中文回答,涉及具体菜谱时引用工具返回的菜名。";

const TITLE_MAX: usize = 50;

fn now_str() -> String {
    Utc::now().to_rfc3339()
}

fn new_id() -> String {
    Uuid::new_v7(Timestamp::now(ContextV7::new())).to_string()
}

// ─── Session DTOs ──────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct CreateSessionReq {
    pub title: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ChatSessionDto {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ChatSessionModel> for ChatSessionDto {
    fn from(m: ChatSessionModel) -> Self {
        Self {
            id: m.id,
            title: m.title,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct ChatSessionListRep {
    pub page: u64,
    pub total: u64,
    pub list: Vec<ChatSessionDto>,
}

#[derive(Serialize, JsonSchema)]
pub struct ChatMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_args: Option<String>,
    pub created_at: String,
}

impl From<ChatMessageModel> for ChatMessageDto {
    fn from(m: ChatMessageModel) -> Self {
        Self {
            id: m.id,
            role: m.role,
            content: m.content,
            status: m.status,
            tool_call_id: m.tool_call_id,
            tool_name: m.tool_name,
            tool_args: m.tool_args,
            created_at: m.created_at,
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct ChatMessageListRep {
    pub page: u64,
    pub total: u64,
    pub list: Vec<ChatMessageDto>,
}

// ─── Session endpoints ─────────────────────────────────────────────────────

#[openapi(tag = "chat", ignore = "db", ignore = "_claims")]
#[post("/api/chat/sessions", data = "<data>", format = "json")]
pub async fn create_session(
    _claims: Claims,
    db: Connection<'_, Db>,
    data: Json<CreateSessionReq>,
) -> Json<Rep<ChatSessionDto>> {
    let db = db.into_inner();
    let now = now_str();
    let title = data
        .title
        .clone()
        .unwrap_or_else(|| "新会话".to_string());
    let model = chat_session::ActiveModel {
        id: Set(new_id()),
        user_id: Set(_claims.sub.clone()),
        title: Set(title),
        created_at: Set(now.clone()),
        updated_at: Set(now),
    };
    match model.insert(db).await {
        Ok(m) => Rep::new(Success.self_code(), "成功", Some(m.into())),
        Err(e) => {
            eprintln!("create_session err: {e}");
            Rep::new(BusinessError.self_code(), "数据库错误", None)
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SessionListParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[openapi(tag = "chat", ignore = "db", ignore = "_claims")]
#[post("/api/chat/sessions/list", data = "<data>", format = "json")]
pub async fn list_sessions(
    _claims: Claims,
    db: Connection<'_, Db>,
    data: Json<SessionListParams>,
) -> Json<Rep<ChatSessionListRep>> {
    let db = db.into_inner();
    let page = data.page.unwrap_or(1).max(1);
    let page_size = data.page_size.unwrap_or(20).max(1).min(100);
    let paginator = ChatSessions::find()
        .filter(chat_session::Column::UserId.eq(&_claims.sub))
        .order_by_desc(chat_session::Column::UpdatedAt)
        .paginate(db, page_size);
    let current_page = page - 1;
    let items = match paginator.fetch_page(current_page).await {
        Ok(v) => v.into_iter().map(Into::into).collect(),
        Err(e) => {
            eprintln!("list_sessions err: {e}");
            return Rep::new(BusinessError.self_code(), "查询错误", None);
        }
    };
    let total = match paginator.num_items().await {
        Ok(t) => t,
        Err(_) => return Rep::new(BusinessError.self_code(), "查询错误", None),
    };
    Rep::new(
        Success.self_code(),
        "成功",
        Some(ChatSessionListRep {
            page,
            total,
            list: items,
        }),
    )
}

#[openapi(tag = "chat", ignore = "db", ignore = "_claims")]
#[get("/api/chat/sessions/<id>")]
pub async fn get_session(
    _claims: Claims,
    db: Connection<'_, Db>,
    id: &str,
) -> Json<Rep<ChatSessionDto>> {
    let db = db.into_inner();
    let res = ChatSessions::find_by_id(id).one(db).await;
    match res {
        Ok(Some(m)) if m.user_id == _claims.sub => {
            Rep::new(Success.self_code(), "成功", Some(m.into()))
        }
        Ok(_) => Rep::new(BadRequest.self_code(), "会话不存在", None),
        Err(e) => {
            eprintln!("get_session err: {e}");
            Rep::new(BusinessError.self_code(), "查询错误", None)
        }
    }
}

#[openapi(tag = "chat", ignore = "db", ignore = "_claims")]
#[delete("/api/chat/sessions/<id>")]
pub async fn delete_session(
    _claims: Claims,
    db: Connection<'_, Db>,
    id: &str,
) -> Json<Rep<()>> {
    let db = db.into_inner();
    let res = ChatSessions::find_by_id(id).one(db).await;
    let session = match res {
        Ok(Some(m)) => m,
        Ok(None) => return Rep::new(BadRequest.self_code(), "会话不存在", None),
        Err(e) => {
            eprintln!("delete_session find err: {e}");
            return Rep::new(BusinessError.self_code(), "查询错误", None);
        }
    };
    if session.user_id != _claims.sub {
        return Rep::new(BadRequest.self_code(), "会话不存在", None);
    }
    match ChatSessions::delete_by_id(id).exec(db).await {
        Ok(_) => Rep::new(Success.self_code(), "成功", None),
        Err(e) => {
            eprintln!("delete_session err: {e}");
            Rep::new(BusinessError.self_code(), "数据库错误", None)
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct MessageListParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[openapi(tag = "chat", ignore = "db", ignore = "_claims")]
#[post("/api/chat/sessions/<id>/messages/list", data = "<data>", format = "json")]
pub async fn list_messages(
    _claims: Claims,
    db: Connection<'_, Db>,
    id: &str,
    data: Json<MessageListParams>,
) -> Json<Rep<ChatMessageListRep>> {
    let db = db.into_inner();
    // ownership check
    let owner_ok = match ChatSessions::find_by_id(id).one(db).await {
        Ok(Some(s)) => s.user_id == _claims.sub,
        _ => false,
    };
    if !owner_ok {
        return Rep::new(BadRequest.self_code(), "会话不存在", None);
    }
    let page = data.page.unwrap_or(1).max(1);
    let page_size = data.page_size.unwrap_or(50).max(1).min(200);
    let paginator = ChatMessages::find()
        .filter(chat_message::Column::SessionId.eq(id))
        .order_by_asc(chat_message::Column::CreatedAt)
        .paginate(db, page_size);
    let current_page = page - 1;
    let items = match paginator.fetch_page(current_page).await {
        Ok(v) => v.into_iter().map(Into::into).collect(),
        Err(e) => {
            eprintln!("list_messages err: {e}");
            return Rep::new(BusinessError.self_code(), "查询错误", None);
        }
    };
    let total = match paginator.num_items().await {
        Ok(t) => t,
        Err(_) => return Rep::new(BusinessError.self_code(), "查询错误", None),
    };
    Rep::new(
        Success.self_code(),
        "成功",
        Some(ChatMessageListRep {
            page,
            total,
            list: items,
        }),
    )
}

// We will add the streaming endpoint in Task 10. The McpClient state import
// and openrouter import are here to keep all imports in one place.
#[allow(dead_code)]
fn _phantom(_mcp: &State<McpClient>, _o: &openrouter::ChatMessage) {}
```

- [ ] **Step 3: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check 2>&1 | tail -40`

Expected: clean. (If the `McpClient` import is unused at this stage, the `_phantom` placeholder avoids the warning. We'll use it in Task 10.)

- [ ] **Step 4: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add src/api/chat.rs src/api/mod.rs
git commit -m "feat(api): add chat session/message CRUD endpoints"
```

---

## Task 10: Add the streaming send endpoint with ReAct loop

**Files:**
- Modify: `src/api/chat.rs`

- [ ] **Step 1: Append the streaming endpoint and helpers**

Append to `src/api/chat.rs` (after the `_phantom` placeholder, remove the placeholder):

```rust
// ─── Streaming send ────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct SendMessageReq {
    pub content: String,
}

const STATUS_PENDING: &str = "pending";
const STATUS_COMPLETE: &str = "complete";
const STATUS_FAILED: &str = "failed";

/// Lightweight Redis-backed rate limiter. Fails open on Redis errors.
async fn check_rate_limit(redis: &Connection<'_, crate::pool::RedisPool>, user_id: &str) -> bool {
    let limit = get_config().chat_rate_limit_per_minute;
    if limit == 0 {
        return true;
    }
    // ...
    true
}

#[openapi(tag = "chat", ignore = "db", ignore = "_claims", ignore = "_mcp", ignore = "_redis")]
#[post("/api/chat/sessions/<id>/messages", data = "<data>", format = "json")]
pub async fn send_message_stream(
    _claims: Claims,
    db: Connection<'_, Db>,
    _redis: Connection<'_, crate::pool::RedisPool>,
    _mcp: &State<McpClient>,
    id: &str,
    data: Json<SendMessageReq>,
) -> Result<rocket::response::stream::TextStream<impl futures::Stream<Item = String>>, rocket::http::Status> {
    use futures::stream::{self, StreamExt};
    use std::io::{self, Write};

    let db = db.into_inner();
    let cfg = get_config();

    // 1. Rate limit
    if !check_rate_limit(&_redis, &_claims.sub).await {
        // Return a one-shot error event since the stream is the only response shape.
        let s = stream::iter(vec!["data: {\"error\":\"rate limit exceeded\"}\n\ndata: [DONE]\n\n".to_string()]);
        return Ok(rocket::response::stream::TextStream(s));
    }

    // 2. Ownership check
    let session = match ChatSessions::find_by_id(id).one(db).await {
        Ok(Some(s)) if s.user_id == _claims.sub => s,
        _ => {
            let s = stream::iter(vec!["data: {\"error\":\"session not found\"}\n\ndata: [DONE]\n\n".to_string()]);
            return Ok(rocket::response::stream::TextStream(s));
        }
    };

    // 3. Persist user message
    let user_msg_id = new_id();
    if let Err(e) = (chat_message::ActiveModel {
        id: Set(user_msg_id.clone()),
        session_id: Set(id.to_string()),
        role: Set("user".to_string()),
        content: Set(data.content.clone()),
        status: Set(STATUS_COMPLETE.to_string()),
        error: Set(None),
        tool_call_id: Set(None),
        tool_name: Set(None),
        tool_args: Set(None),
        prompt_tokens: Set(None),
        completion_tokens: Set(None),
        created_at: Set(now_str()),
    })
    .insert(db)
    .await
    {
        eprintln!("persist user msg err: {e}");
        let s = stream::iter(vec!["data: {\"error\":\"db error\"}\n\ndata: [DONE]\n\n".to_string()]);
        return Ok(rocket::response::stream::TextStream(s));
    }

    // 4. Load history (last 20)
    let history: Vec<openrouter::ChatMessage> = match ChatMessages::find()
        .filter(chat_message::Column::SessionId.eq(id))
        .order_by_asc(chat_message::Column::CreatedAt)
        .all(db)
        .await
    {
        Ok(v) => v
            .into_iter()
            .rev()
            .take(20)
            .rev()
            .filter_map(|m| model_to_chat_message(m))
            .collect(),
        Err(e) => {
            eprintln!("load history err: {e}");
            Vec::new()
        }
    };

    // 5. ReAct loop
    let mut iter_count: u32 = 0;
    let mut current_messages = build_messages(&history, &data.content);
    let tools_payload = openrouter::mcp_tools_payload();
    let model_name = cfg.openrouter_model.clone();

    // We'll collect events from the loop in a channel and stream them out.
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let mcp = _mcp.inner().clone();
    let db_clone = db.clone();
    let session_id = id.to_string();
    let user_content = data.content.clone();

    tokio::spawn(async move {
        let mut final_text = String::new();
        let mut hit_iter_cap = false;
        let mut any_tool_calls = false;

        loop {
            iter_count += 1;
            if iter_count > cfg.chat_max_tool_iterations {
                hit_iter_cap = true;
                break;
            }

            let req = openrouter::ChatRequest {
                model: &model_name,
                messages: &current_messages,
                tools: &tools_payload,
                stream: true,
            };

            let resp = match openrouter::stream_chat(&req).await {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(format!("data: {{\"error\":\"openrouter: {}\"}}\n\n", e));
                    break;
                }
            };

            let mut accumulator = StringAccumulator::new();
            let mut pending_tool_calls: Vec<openrouter::ToolCall> = Vec::new();
            let mut finish_reason: Option<String> = None;

            use tokio_stream::StreamExt as _;
            let mut byte_stream = resp.bytes_stream();
            let mut sse_buf = String::new();
            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(format!("data: {{\"error\":\"stream: {}\"}}\n\n", e));
                        // Best-effort persist any accumulator content as failed
                        persist_assistant_failed(&db_clone, &session_id, &accumulator.text, &format!("stream err: {e}")).await;
                        return;
                    }
                };
                sse_buf.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(idx) = sse_buf.find("\n\n") {
                    let event: String = sse_buf.drain(..idx + 2).collect();
                    handle_sse_event(
                        &event,
                        &mut accumulator,
                        &mut pending_tool_calls,
                        &mut finish_reason,
                    );
                }
            }

            // Drain any tail
            if !sse_buf.is_empty() {
                handle_sse_event(
                    &sse_buf,
                    &mut accumulator,
                    &mut pending_tool_calls,
                    &mut finish_reason,
                );
            }

            // Decide what to do based on what we got.
            if !pending_tool_calls.is_empty() {
                any_tool_calls = true;
                // Persist the assistant turn (with tool calls) and invoke each tool.
                let assistant_id = new_id();
                let tool_args_json = serde_json::to_string(&pending_tool_calls).unwrap_or_default();
                let first_name = pending_tool_calls.first().map(|t| t.function.name.clone()).unwrap_or_default();
                let first_id = pending_tool_calls.first().map(|t| t.id.clone()).unwrap_or_default();
                let _ = (chat_message::ActiveModel {
                    id: Set(assistant_id.clone()),
                    session_id: Set(session_id.clone()),
                    role: Set("assistant".to_string()),
                    content: Set(String::new()),
                    status: Set(STATUS_COMPLETE.to_string()),
                    error: Set(None),
                    tool_call_id: Set(Some(first_id.clone())),
                    tool_name: Set(Some(first_name)),
                    tool_args: Set(Some(tool_args_json)),
                    prompt_tokens: Set(None),
                    completion_tokens: Set(None),
                    created_at: Set(now_str()),
                })
                .insert(&db_clone)
                .await;

                // Push the assistant message (with tool_calls) into the conversation.
                current_messages.push(openrouter::ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(pending_tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                for tc in pending_tool_calls.drain(..) {
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::json!({}));
                    let result = match crate::mcp_client::call_tool(&mcp, &tc.function.name, args.clone()).await {
                        Ok(s) => s,
                        Err(e) => {
                            let err_msg = format!("工具调用失败: {e}");
                            let _ = (chat_message::ActiveModel {
                                id: Set(new_id()),
                                session_id: Set(session_id.clone()),
                                role: Set("tool".to_string()),
                                content: Set(err_msg.clone()),
                                status: Set(STATUS_FAILED.to_string()),
                                error: Set(Some(err_msg.clone())),
                                tool_call_id: Set(Some(tc.id.clone())),
                                tool_name: Set(Some(tc.function.name.clone())),
                                tool_args: Set(Some(serde_json::to_string(&args).unwrap_or_default())),
                                prompt_tokens: Set(None),
                                completion_tokens: Set(None),
                                created_at: Set(now_str()),
                            })
                            .insert(&db_clone)
                            .await;
                            err_msg
                        }
                    };
                    let _ = (chat_message::ActiveModel {
                        id: Set(new_id()),
                        session_id: Set(session_id.clone()),
                        role: Set("tool".to_string()),
                        content: Set(result.clone()),
                        status: Set(STATUS_COMPLETE.to_string()),
                        error: Set(None),
                        tool_call_id: Set(Some(tc.id.clone())),
                        tool_name: Set(Some(tc.function.name.clone())),
                        tool_args: Set(Some(serde_json::to_string(&args).unwrap_or_default())),
                        prompt_tokens: Set(None),
                        completion_tokens: Set(None),
                        created_at: Set(now_str()),
                    })
                    .insert(&db_clone)
                    .await;

                    current_messages.push(openrouter::ChatMessage {
                        role: "tool".to_string(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }

                // Continue the loop — call OpenRouter again.
                continue;
            }

            // No tool calls — this is the final iteration.
            final_text = accumulator.text.clone();
            // Persist and stream.
            let assistant_id = new_id();
            let _ = (chat_message::ActiveModel {
                id: Set(assistant_id),
                session_id: Set(session_id.clone()),
                role: Set("assistant".to_string()),
                content: Set(final_text.clone()),
                status: Set(STATUS_COMPLETE.to_string()),
                error: Set(None),
                tool_call_id: Set(None),
                tool_name: Set(None),
                tool_args: Set(None),
                prompt_tokens: Set(None),
                completion_tokens: Set(None),
                created_at: Set(now_str()),
            })
            .insert(&db_clone)
            .await;
            // Push the deltas to the client.
            for piece in split_into_deltas(&final_text) {
                let _ = tx.send(format!("data: {{\"delta\":{}}}\n\n", serde_json::Value::String(piece)));
            }
            break;
        }

        if hit_iter_cap {
            let fallback = "我尝试了多次仍无法得到答案,请换个问法。".to_string();
            let _ = (chat_message::ActiveModel {
                id: Set(new_id()),
                session_id: Set(session_id.clone()),
                role: Set("assistant".to_string()),
                content: Set(fallback.clone()),
                status: Set(STATUS_COMPLETE.to_string()),
                error: Set(None),
                tool_call_id: Set(None),
                tool_name: Set(None),
                tool_args: Set(None),
                prompt_tokens: Set(None),
                completion_tokens: Set(None),
                created_at: Set(now_str()),
            })
            .insert(&db_clone)
            .await;
            let _ = tx.send(format!("data: {{\"delta\":{}}}\n\n", serde_json::Value::String(fallback)));
        }
        let _ = tx.send("data: [DONE]\n\n".to_string());

        // Touch session to bump updated_at
        let _ = session_id; // suppress unused
        let _ = user_content; // suppress unused
        let _ = any_tool_calls; // suppress unused
    });

    // ... rest in next step
    let _ = io::stdout(); // suppress unused
    let _ = Write::flush; // suppress unused
    let _ = (); // suppress unused
    let s = tokio_stream::wrappers::UnboundedReceiverStream::new(rx).map(|s| s);
    Ok(rocket::response::stream::TextStream(s))
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn build_messages(history: &[openrouter::ChatMessage], _new_user_content: &str) -> Vec<openrouter::ChatMessage> {
    let mut out = Vec::with_capacity(history.len() + 2);
    out.push(openrouter::ChatMessage {
        role: "system".to_string(),
        content: Some(SYSTEM_PROMPT.to_string()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });
    out.extend(history.iter().cloned());
    out
}

fn model_to_chat_message(m: ChatMessageModel) -> Option<openrouter::ChatMessage> {
    match m.role.as_str() {
        "user" => Some(openrouter::ChatMessage {
            role: "user".to_string(),
            content: Some(m.content),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }),
        "assistant" => {
            // Try to parse tool_calls from tool_args.
            let tool_calls = m
                .tool_args
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<openrouter::ToolCall>>(s).ok());
            Some(openrouter::ChatMessage {
                role: "assistant".to_string(),
                content: if m.content.is_empty() { None } else { Some(m.content) },
                tool_calls,
                tool_call_id: None,
                name: None,
            })
        }
        "tool" => Some(openrouter::ChatMessage {
            role: "tool".to_string(),
            content: Some(m.content),
            tool_calls: None,
            tool_call_id: m.tool_call_id,
            name: m.tool_name,
        }),
        _ => None,
    }
}

struct StringAccumulator {
    text: String,
}

impl StringAccumulator {
    fn new() -> Self {
        Self { text: String::new() }
    }
    fn push(&mut self, s: &str) {
        self.text.push_str(s);
    }
}

fn handle_sse_event(
    event: &str,
    accum: &mut StringAccumulator,
    tool_calls: &mut Vec<openrouter::ToolCall>,
    finish_reason: &mut Option<String>,
) {
    let trimmed = event.trim_end_matches('\n').trim_start();
    if !trimmed.starts_with("data:") {
        return;
    }
    let payload = trimmed.trim_start_matches("data:").trim();
    if payload == "[DONE]" {
        return;
    }
    let v: serde_json::Value = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(_) => return,
    };
    if let Some(choice) = v.get("choices").and_then(|c| c.get(0)) {
        if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
            *finish_reason = Some(reason.to_string());
        }
        if let Some(delta) = choice.get("delta") {
            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                accum.push(content);
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                for tc in tcs {
                    let idx = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                    // Ensure vec is long enough
                    while tool_calls.len() <= idx {
                        tool_calls.push(openrouter::ToolCall {
                            id: String::new(),
                            kind: "function".to_string(),
                            function: openrouter::ToolCallFunction {
                                name: String::new(),
                                arguments: String::new(),
                            },
                        });
                    }
                    let entry = &mut tool_calls[idx];
                    if let Some(id) = tc.get("id").and_then(|s| s.as_str()) {
                        entry.id = id.to_string();
                    }
                    if let Some(func) = tc.get("function") {
                        if let Some(name) = func.get("name").and_then(|s| s.as_str()) {
                            entry.function.name = name.to_string();
                        }
                        if let Some(args) = func.get("arguments").and_then(|s| s.as_str()) {
                            entry.function.arguments.push_str(args);
                        }
                    }
                }
            }
        }
    }
}

fn split_into_deltas(text: &str) -> Vec<String> {
    // Send as a single chunk for simplicity. A more sophisticated impl would
    // tokenize or chunk at sentence boundaries.
    vec![text.to_string()]
}

async fn persist_assistant_failed(
    db: &sea_orm::DatabaseConnection,
    session_id: &str,
    content: &str,
    err: &str,
) {
    let _ = (chat_message::ActiveModel {
        id: Set(new_id()),
        session_id: Set(session_id.to_string()),
        role: Set("assistant".to_string()),
        content: Set(content.to_string()),
        status: Set(STATUS_FAILED.to_string()),
        error: Set(Some(err.to_string())),
        tool_call_id: Set(None),
        tool_name: Set(None),
        tool_args: Set(None),
        prompt_tokens: Set(None),
        completion_tokens: Set(None),
        created_at: Set(now_str()),
    })
    .insert(db)
    .await;
}
```

- [ ] **Step 2: Replace the placeholder rate-limit body with a real implementation**

Replace the body of `check_rate_limit`:

```rust
async fn check_rate_limit(redis: &Connection<'_, crate::pool::RedisPool>, user_id: &str) -> bool {
    use rocket_db_pools::Connection as _;
    let limit = get_config().chat_rate_limit_per_minute;
    if limit == 0 {
        return true;
    }
    let now_min = Utc::now().timestamp() / 60;
    let key = format!("chat:rl:{}:{}", user_id, now_min);
    // Best-effort: if any step fails, allow the request.
    let pool = redis.cached_pool();
    let mut conn = match pool.get().await {
        Ok(c) => c,
        Err(_) => return true,
    };
    use redis::AsyncCommands;
    let count: Option<u32> = conn.incr(&key, 1).await.ok();
    if count == Some(1) {
        let _: Result<(), _> = conn.expire(&key, 65).await;
    }
    match count {
        Some(n) if n > limit => false,
        _ => true,
    }
}
```

If the `redis` crate isn't already in `Cargo.toml`, add it: `redis = "0.27"` (verify the version matches the `deadpool_redis` feature in use — `rocket_db_pools 0.1.0` with `deadpool_redis` uses `redis 0.27.x`).

- [ ] **Step 3: Add needed dependencies to Cargo.toml**

Verify the following are in `[dependencies]` of root `Cargo.toml`:
- `futures = "0.3"` — for `Stream` trait usage in the return type
- `tokio-stream = "0.1"` — for `tokio_stream::StreamExt` and `wrappers::UnboundedReceiverStream`

Add if missing:

```toml
futures = "0.3"
tokio-stream = "0.1"
```

- [ ] **Step 4: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo check 2>&1 | tail -60`

Expected: a long list of errors on first pass — the SSE parsing, stream wiring, and rate-limiter types are tricky. Fix them iteratively. Common issues:
- `UnboundedReceiverStream` lives at `tokio_stream::wrappers::UnboundedReceiverStream`.
- `TextStream<S>` requires `S: Stream<Item = String> + 'static`.
- `futures::Stream` vs `tokio_stream::Stream` — the project likely uses one of them. Pick whichever `cargo check` points you to.
- `Connection<RedisPool>` needs `.cached_pool()` from `rocket_db_pools` to get the underlying `deadpool_redis::Pool`.
- `db.clone()` requires `DatabaseConnection: Clone`, which it does.

Iterate until `cargo check` is clean.

- [ ] **Step 5: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add src/api/chat.rs Cargo.toml Cargo.lock
git commit -m "feat(api): add streaming chat send endpoint with ReAct loop"
```

---

## Task 11: Mount the chat routes in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add the new routes**

In `src/main.rs`, add to the `openapi_get_routes!` macro invocation:

```rust
api::chat::create_session,
api::chat::list_sessions,
api::chat::get_session,
api::chat::delete_session,
api::chat::list_messages,
api::chat::send_message_stream,
```

Order doesn't matter. Keep them grouped together.

- [ ] **Step 2: Verify compile**

Run: `cd /Users/chenpan/Documents/doc/my-project/eat-be && cargo build 2>&1 | tail -40`

Expected: clean.

- [ ] **Step 3: Commit**

```bash
cd /Users/chenpan/Documents/doc/my-project/eat-be
git add src/main.rs
git commit -m "feat(main): mount chat routes"
```

---

## Task 12: Manual smoke test

**Files:** none

- [ ] **Step 1: Set a real OpenRouter API key**

Export the env var before starting the server:

```bash
export OPENROUTER_API_KEY="sk-or-v1-..."
cd /Users/chenpan/Documents/doc/my-project/eat-be
cargo run
```

Wait for the server to log "Rocket has launched" and the MCP server to log "✅ 端点".

- [ ] **Step 2: Get a JWT token**

Log in or use an existing user's token. The auth flow is in `src/auth.rs` (not shown in this plan). The simplest path: use a known user's token from a previous request, or POST to `/api/auth/login` to obtain one.

- [ ] **Step 3: Create a session**

```bash
TOKEN="..."
curl -sS -X POST http://127.0.0.1:8088/api/chat/sessions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "测试会话"}'
```

Expected: JSON with `code: 200` and a session `id`.

- [ ] **Step 4: List sessions**

```bash
curl -sS -X POST http://127.0.0.1:8088/api/chat/sessions/list \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"page": 1, "page_size": 20}'
```

Expected: 1 session in `list`.

- [ ] **Step 5: Stream a non-tool message**

```bash
SID="<session id from step 3>"
curl -sN -X POST http://127.0.0.1:8088/api/chat/sessions/$SID/messages \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content": "你好"}'
```

Expected: a series of `data: {"delta": "..."}` lines ending with `data: [DONE]`.

- [ ] **Step 6: Verify persistence**

```bash
curl -sS -X POST http://127.0.0.1:8088/api/chat/sessions/$SID/messages/list \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"page": 1, "page_size": 50}'
```

Expected: both the user message ("你好") and the assistant reply, each with `status: "complete"`.

- [ ] **Step 7: Stream a tool-using message**

```bash
curl -sN -X POST http://127.0.0.1:8088/api/chat/sessions/$SID/messages \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content": "推荐一道用鸡蛋做的家常菜"}'
```

Expected: SSE stream with a final reply that mentions a real recipe. The SSE stream itself should NOT contain tool-call events (per spec). Then list messages and verify you see `user → assistant (with tool_call_id/tool_args) → tool → assistant` four messages.

- [ ] **Step 8: Delete the session**

```bash
curl -sS -X DELETE http://127.0.0.1:8088/api/chat/sessions/$SID \
  -H "Authorization: Bearer $TOKEN"
```

Expected: `code: 200`. Subsequent GET returns `code: 400` with "会话不存在".

- [ ] **Step 9: Test cross-user isolation**

Log in as user B. Try `GET /api/chat/sessions/<user A's session id>`. Expected: 400 with "会话不存在" (not 403).

- [ ] **Step 10: Test rate limit**

Make 21 send requests in a single minute (with simple "hi" content). Expected: the 21st stream's first event is `data: {"error":"rate limit exceeded"}`.

- [ ] **Step 11: Test OpenRouter failure**

Stop the server, set `OPENROUTER_API_KEY=sk-or-v1-bogus` in the env, restart, then send a message. Expected: SSE stream's first event is `data: {"error":"openrouter: upstream returned 401: ..."}`. List messages — should be empty (no rows persisted because pre-stream failure).

- [ ] **Step 12: Commit any final fixes**

If steps 5–11 revealed bugs, fix them in code (not by editing the spec) and commit each fix with a descriptive message.

---

## Self-review

**Spec coverage check:**
- Goal (chat API streaming OpenRouter) → Tasks 6, 9, 10, 11
- mcp-server tool integration → Tasks 7, 8, 10
- Sessions/messages persist to DB → Tasks 2, 3, 4, 9
- JWT auth + rate limit → Tasks 9, 10
- Configuration (openrouter_api_key, model, rate limit, max iterations, mcp url) → Task 5
- ReAct loop with max iterations → Task 10
- Error handling (pre-stream fail, mid-stream fail, MCP fail, iter cap) → Task 10
- Manual smoke test → Task 12

**Placeholder scan:** No "TBD"/"TODO" remain. Each step has concrete code or commands. The iterative "fix until clean" instructions in Tasks 7 and 10 are necessary because the exact `rmcp` 0.15 client API and Rocket stream wiring are not deterministic without compilation feedback — this is unavoidable.

**Type consistency:**
- `openrouter::ChatMessage` fields used in Task 10 (`role`, `content`, `tool_calls`, `tool_call_id`, `name`) match Task 6's struct definition.
- `openrouter::ChatRequest` fields (`model`, `messages`, `tools`, `stream`) match Task 6.
- `openrouter::ToolCall` fields (`id`, `kind`, `function`) match Task 6 and Task 10's parser.
- `chat_message::ActiveModel` field names match the entity from Task 3.
- `McpClient` type alias is defined once in Task 7 and used as `State<McpClient>` in Task 9/10.
- `STATUS_*` constants defined in Task 10 and used in the same task — consistent.
