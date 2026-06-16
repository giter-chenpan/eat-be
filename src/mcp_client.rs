use crate::config::get_config;
use rmcp::model::{CallToolRequestParams, Content, Tool};
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type McpClient = Arc<Mutex<Option<RunningService<RoleClient, ()>>>>;

/// Build a transport pointed at the local mcp-server. Caller is responsible for
/// connecting it via `.serve(...)` and storing the result.
pub async fn build_client() -> anyhow::Result<McpClient> {
    let cfg = get_config();
    let url = format!("{}/mcp", cfg.mcp_server_url);
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::default(),
        StreamableHttpClientTransportConfig::with_uri(url),
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
    let req = CallToolRequestParams {
        name: name.to_string().into(),
        arguments: Some(args.as_object().cloned().unwrap_or_default()),
        meta: None,
        task: None,
    };
    let result = svc
        .call_tool(req)
        .await
        .map_err(|e| format!("mcp call_tool failed: {e}"))?;
    let text = result
        .content
        .iter()
        .filter_map(|c: &Content| c.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text)
}

/// Return the list of MCP tool definitions in OpenRouter's `tools` field format.
pub fn tool_definitions_for_openrouter() -> Vec<serde_json::Value> {
    crate::openrouter::mcp_tools_payload()
}

/// Suppress unused warning for the Tool type (re-exported for future use).
#[allow(dead_code)]
fn _phantom(_: Tool) {}
