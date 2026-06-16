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
