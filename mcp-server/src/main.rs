use howtocook_mcp_server::run_mcp_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_mcp_server().await
}
