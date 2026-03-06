use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::CallToolError, CallToolRequest, CallToolResult, ListToolsRequest,
    ListToolsResult, RpcError,
};
use rust_mcp_sdk::{mcp_server::ServerHandler, McpServer};

use crate::tools::CookbookTools;

/// Custom handler for the HowToCook MCP Server
pub struct CookbookServerHandler;

impl CookbookServerHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServerHandler for CookbookServerHandler {
    /// Handle ListToolsRequest: return all available cookbook tools
    async fn handle_list_tools_request(
        &self,
        _request: ListToolsRequest,
        _runtime: &dyn McpServer,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: CookbookTools::tools(),
        })
    }

    /// Handle CallToolRequest: dispatch to the right tool
    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        _runtime: &dyn McpServer,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        // Convert request parameters into CookbookTools enum
        let tool_params: CookbookTools =
            CookbookTools::try_from(request.params).map_err(CallToolError::new)?;

        // Match the tool variant and execute its corresponding logic
        match tool_params {
            CookbookTools::ListCategoriesTool(tool) => tool.call_tool().await,
            CookbookTools::ListRecipesTool(tool) => tool.call_tool().await,
            CookbookTools::GetRecipeTool(tool) => tool.call_tool().await,
            CookbookTools::SearchRecipesTool(tool) => tool.call_tool().await,
            CookbookTools::GetTipsTool(tool) => tool.call_tool().await,
            CookbookTools::GetReadmeTool(tool) => tool.call_tool().await,
        }
    }
}
