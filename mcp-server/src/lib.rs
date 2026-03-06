pub mod handler;
pub mod tools;

use handler::CookbookServerHandler;
use rust_mcp_schema::{
    Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
    LATEST_PROTOCOL_VERSION,
};

use rust_mcp_sdk::mcp_server::{hyper_server::create_server, HyperServerOptions};

pub async fn run_mcp_server() -> anyhow::Result<()> {
    // Define server details and capabilities
    let server_details = InitializeResult {
        server_info: Implementation {
            name: "HowToCook MCP Server".to_string(),
            version: "0.1.0".to_string(),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools {
                list_changed: None,
            }),
            ..Default::default()
        },
        meta: None,
        instructions: Some(
            "这是一个读取 HowToCook (程序员做饭指南) GitHub 仓库内容的 MCP 服务。\n\
             提供以下工具：\n\
             - list_categories: 列出所有菜谱分类\n\
             - list_recipes: 列出指定分类下的所有菜谱\n\
             - get_recipe: 获取指定菜谱的详细内容\n\
             - search_recipes: 搜索菜谱\n\
             - get_tips: 获取烹饪技巧"
                .to_string(),
        ),
        protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
    };

    // Instantiate our custom handler
    let handler = CookbookServerHandler::new();

    // Configuration for SSE Transport via hyper server
    let server_options = HyperServerOptions {
        host: "127.0.0.1".to_string(),
        port: 8081, // Use port 8081 (Rocket uses 8000)
        custom_sse_endpoint: Some("/sse".to_string()),
        ..Default::default()
    };

    let server = create_server(server_details, handler, server_options);

    // Start the server
    server.start().await.map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(())
}
