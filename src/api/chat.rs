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

// We will add the streaming endpoint in Task 10. The McpClient state import
// and openrouter import are here to keep all imports in one place.
#[allow(dead_code)]
fn _phantom(_mcp: &State<McpClient>, _o: &openrouter::ChatMessage) {}