use crate::common::data_structure::*;
use crate::common::enums::Code::*;
use crate::config::get_config;
use crate::jwtuser::Claims;
use crate::mcp_client::McpClient;
use crate::openrouter;
use crate::pool::{Db, RedisPool};
use ::entity::chat_message::{self, Entity as ChatMessages, Model as ChatMessageModel};
use ::entity::chat_session::{self, Entity as ChatSessions, Model as ChatSessionModel};
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::State;
use rocket_db_pools::deadpool_redis;
use rocket_okapi::{openapi, JsonSchema};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set};
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

// ─── Streaming send ────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct SendMessageReq {
    pub content: String,
}

const STATUS_PENDING: &str = "pending";
const STATUS_COMPLETE: &str = "complete";
const STATUS_FAILED: &str = "failed";

type SseStream = std::pin::Pin<Box<dyn futures::Stream<Item = String> + Send + 'static>>;

fn error_stream(msg: &str) -> SseStream {
    let payload = format!("data: {{\"error\":\"{msg}\"}}\n\ndata: [DONE]\n\n");
    Box::pin(futures::stream::once(async move { payload }))
}

/// Lightweight Redis-backed rate limiter. Fails open on Redis errors.
async fn check_rate_limit(
    pool: &deadpool_redis::Pool,
    user_id: &str,
) -> bool {
    let limit = get_config().chat_rate_limit_per_minute;
    if limit == 0 {
        return true;
    }
    let now_min = Utc::now().timestamp() / 60;
    let key = format!("chat:rl:{}:{}", user_id, now_min);
    let mut conn: deadpool_redis::Connection = match pool.get().await {
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

#[openapi(
    tag = "chat",
    ignore = "db",
    ignore = "_claims",
    ignore = "_mcp",
    ignore = "_redis"
)]
#[post("/api/chat/sessions/<id>/messages", data = "<data>", format = "json")]
pub async fn send_message_stream(
    _claims: Claims,
    db: Connection<'_, Db>,
    _redis: &State<RedisPool>,
    _mcp: &State<McpClient>,
    id: &str,
    data: Json<SendMessageReq>,
) -> Result<rocket::response::stream::TextStream<SseStream>, rocket::http::Status> {
    use futures::stream::StreamExt;
    use tokio::sync::mpsc;

    let db = db.into_inner();
    let cfg = get_config();

    // 1. Rate limit
    if !check_rate_limit(&**_redis, &_claims.sub).await {
        return Ok(rocket::response::stream::TextStream(error_stream(
            "rate limit exceeded",
        )));
    }

    // 2. Ownership check
    let _session = match ChatSessions::find_by_id(id).one(db).await {
        Ok(Some(s)) if s.user_id == _claims.sub => s,
        _ => {
            return Ok(rocket::response::stream::TextStream(error_stream(
                "session not found",
            )));
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
        return Ok(rocket::response::stream::TextStream(error_stream("db error")));
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
    let mut current_messages = build_messages(&history);
    let tools_payload = openrouter::mcp_tools_payload();
    let model_name = cfg.openrouter_model.clone();

    let (tx, rx) = mpsc::unbounded_channel::<String>();

    let mcp = _mcp.inner().clone();
    let db_clone = db.clone();
    let session_id = id.to_string();
    let user_content = data.content.clone();
    let max_iterations = cfg.chat_max_tool_iterations;

    tokio::spawn(async move {
        let mut final_text = String::new();
        let mut hit_iter_cap = false;
        let mut last_finish_reason: Option<String> = None;

        loop {
            iter_count += 1;
            if iter_count > max_iterations {
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
                    let _ = tx.send(format!(
                        "data: {{\"error\":\"openrouter: {}\"}}\n\n",
                        e
                    ));
                    break;
                }
            };

            let mut accumulator = StringAccumulator::new();
            let mut pending_tool_calls: Vec<openrouter::ToolCall> = Vec::new();
            let mut finish_reason: Option<String> = None;

            let mut byte_stream = resp.bytes_stream();
            let mut sse_buf = String::new();
            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(format!(
                            "data: {{\"error\":\"stream: {}\"}}\n\n",
                            e
                        ));
                        persist_assistant_failed(
                            &db_clone,
                            &session_id,
                            &accumulator.text,
                            &format!("stream err: {e}"),
                        )
                        .await;
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

            if !sse_buf.is_empty() {
                handle_sse_event(
                    &sse_buf,
                    &mut accumulator,
                    &mut pending_tool_calls,
                    &mut finish_reason,
                );
            }

            last_finish_reason = finish_reason;

            if !pending_tool_calls.is_empty() {
                let assistant_id = new_id();
                let tool_args_json =
                    serde_json::to_string(&pending_tool_calls).unwrap_or_default();
                let first_name = pending_tool_calls
                    .first()
                    .map(|t| t.function.name.clone())
                    .unwrap_or_default();
                let first_id = pending_tool_calls
                    .first()
                    .map(|t| t.id.clone())
                    .unwrap_or_default();
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

                current_messages.push(openrouter::ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(pending_tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                for tc in pending_tool_calls.drain(..) {
                    let args: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::json!({}));
                    let result = match crate::mcp_client::call_tool(
                        &mcp,
                        &tc.function.name,
                        args.clone(),
                    )
                    .await
                    {
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
                                tool_args: Set(Some(
                                    serde_json::to_string(&args).unwrap_or_default(),
                                )),
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
                        tool_args: Set(Some(
                            serde_json::to_string(&args).unwrap_or_default(),
                        )),
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

                continue;
            }

            // No tool calls — this is the final iteration.
            final_text = accumulator.text.clone();
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
            for piece in split_into_deltas(&final_text) {
                let _ = tx.send(format!(
                    "data: {{\"delta\":{}}}\n\n",
                    serde_json::Value::String(piece)
                ));
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
            let _ = tx.send(format!(
                "data: {{\"delta\":{}}}\n\n",
                serde_json::Value::String(fallback)
            ));
        }
        let _ = tx.send("data: [DONE]\n\n".to_string());

        // Suppress unused-variable warnings
        let _ = (user_content, last_finish_reason);
    });

    let s: SseStream = Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx));
    Ok(rocket::response::stream::TextStream(s))
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn build_messages(history: &[openrouter::ChatMessage]) -> Vec<openrouter::ChatMessage> {
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
            let tool_calls = m
                .tool_args
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<openrouter::ToolCall>>(s).ok());
            Some(openrouter::ChatMessage {
                role: "assistant".to_string(),
                content: if m.content.is_empty() {
                    None
                } else {
                    Some(m.content)
                },
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
        Self {
            text: String::new(),
        }
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