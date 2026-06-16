use crate::config::get_config;
use rmcp::model::{CallToolRequestParams, Content};
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Lazily-initialized MCP client handle. The inner `OnceCell` is empty until the
/// first tool call triggers a connection, so the application can boot even if
/// the mcp-server is briefly unavailable.
pub type McpClient = Arc<OnceCell<RunningService<RoleClient, ()>>>;

/// Build an MCP client handle. This is intentionally synchronous and performs
/// no I/O — it only constructs the transport configuration and wraps it in a
/// `OnceCell`. The actual connection is deferred to `ensure_connected`, which
/// runs on the first tool call.
pub fn build_client() -> McpClient {
    Arc::new(OnceCell::new())
}

/// Connect to the MCP server on first call, returning the running service.
/// Subsequent calls return the cached service without re-connecting.
async fn ensure_connected(client: &McpClient) -> Result<&RunningService<RoleClient, ()>, String> {
    client
        .get_or_try_init(|| async {
            let cfg = get_config();
            let url = format!("{}/mcp", cfg.mcp_server_url);
            let transport = StreamableHttpClientTransport::with_client(
                reqwest::Client::default(),
                StreamableHttpClientTransportConfig::with_uri(url),
            );
            let svc = ().serve(transport).await.map_err(|e| {
                format!("mcp client connect failed: {e}")
            })?;
            Ok::<_, String>(svc)
        })
        .await
}

/// Call an MCP tool by name with the given arguments (as a JSON object).
/// Returns the tool's text result. On any error, returns the error stringified.
pub async fn call_tool(
    client: &McpClient,
    name: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    let svc = ensure_connected(client).await?;
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
