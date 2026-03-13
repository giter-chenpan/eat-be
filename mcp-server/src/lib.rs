use anyhow::Context;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::process::Command;

// ─── 常量 ────────────────────────────────────────────────────────────────────

const REPO_URL: &str = "https://github.com/Anduin2017/HowToCook.git";
pub const DEFAULT_REPO_DIR: &str = "./mcp-server/howtocook-data";
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8081";
pub const MCP_PATH: &str = "/mcp";

// ─── 数据结构 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RecipeInfo {
    pub name: String,
    pub category: String,
    pub content: String,
}

// ─── 工具参数 ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRecipesParams {
    /// 可选：按分类过滤，例如 "aquatic"、"staple"
    pub category: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRecipeParams {
    /// 菜谱名称，例如 "西红柿炒鸡蛋"
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRecipesParams {
    /// 搜索关键词（同时匹配名称和内容）
    pub keyword: String,
}

// ─── MCP 服务器 ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct HowToCookServer {
    recipes: Arc<HashMap<String, RecipeInfo>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl HowToCookServer {
    pub fn new(recipes: HashMap<String, RecipeInfo>) -> Self {
        Self {
            recipes: Arc::new(recipes),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "列出 HowToCook 仓库中所有目录，含菜谱分类、难度等级、技巧指南")]
    async fn list_categories(&self) -> Result<CallToolResult, McpError> {
        let mut categories: Vec<String> = self
            .recipes
            .values()
            .map(|r| r.category.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        categories.sort();
        Ok(CallToolResult::success(vec![Content::text(
            categories.join("\n"),
        )]))
    }

    #[tool(description = "列出所有内容名称（菜谱、难度或技巧），可通过 category 参数按分类过滤（例如：“难度等级”、“技巧指南”）")]
    async fn list_recipes(
        &self,
        Parameters(params): Parameters<ListRecipesParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut names: Vec<String> = self
            .recipes
            .values()
            .filter(|r| params.category.as_deref().map_or(true, |c| r.category == c))
            .map(|r| format!("[{}] {}", r.category, r.name))
            .collect();
        names.sort();

        let text = if names.is_empty() {
            "未找到对应内容".to_string()
        } else {
            names.join("\n")
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(description = "根据名称获取完整 Markdown 内容（支持菜谱、难度系统和技巧指南）")]
    async fn get_recipe(
        &self,
        Parameters(params): Parameters<GetRecipeParams>,
    ) -> Result<CallToolResult, McpError> {
        self.recipes
            .get(&params.name)
            .map(|r| CallToolResult::success(vec![Content::text(r.content.clone())]))
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("未找到名称为 '{}' 的内容，请先用 list_recipes 确认", params.name),
                    None,
                )
            })
    }

    #[tool(description = "在菜谱、难度和技巧的名称及内容中搜索关键词，返回匹配列表")]
    async fn search_recipes(
        &self,
        Parameters(params): Parameters<SearchRecipesParams>,
    ) -> Result<CallToolResult, McpError> {
        let kw = params.keyword.to_lowercase();
        let mut results: Vec<String> = self
            .recipes
            .values()
            .filter(|r| {
                r.name.to_lowercase().contains(&kw) || r.content.to_lowercase().contains(&kw)
            })
            .map(|r| format!("[{}] {}", r.category, r.name))
            .collect();
        results.sort();

        let text = if results.is_empty() {
            format!("未找到包含 '{}' 的内容", params.keyword)
        } else {
            format!("找到 {} 条结果：\n{}", results.len(), results.join("\n"))
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

#[tool_handler]
impl ServerHandler for HowToCookServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "howtocook-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            instructions: Some(
                "HowToCook MCP — 中文菜谱、难度系统及厨房技巧查询。\n\
                 工具：list_categories / list_recipes / get_recipe / search_recipes"
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

// ─── 仓库管理 ────────────────────────────────────────────────────────────────

/// 确保 HowToCook 仓库存在并是最新的（异步，不阻塞 tokio 线程）
async fn ensure_repo(repo_path: &Path) -> anyhow::Result<()> {
    let git_dir = repo_path.join(".git");

    if git_dir.exists() {
        tracing::info!("[MCP] 仓库已存在，拉取最新菜谱 …");
        let out = Command::new("git")
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .args(["-C", repo_path.to_str().unwrap_or("."), "pull", "--ff-only"])
            .output()
            .await;
        match out {
            Ok(o) if o.status.success() => tracing::info!("[MCP] 拉取成功"),
            Ok(o) => tracing::warn!("[MCP] git pull：{}", String::from_utf8_lossy(&o.stderr)),
            Err(e) => tracing::warn!("[MCP] git pull 失败：{e}"),
        }
        return Ok(());
    }

    // 残留目录（上次克隆中断），先清理
    if repo_path.exists() {
        tracing::warn!("[MCP] 清理残留目录 {} …", repo_path.display());
        tokio::fs::remove_dir_all(repo_path)
            .await
            .with_context(|| format!("无法删除 {}", repo_path.display()))?;
    }

    tracing::info!("[MCP] 克隆 HowToCook（跳过 LFS）…");
    let out = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            // 禁用 LFS 过滤器，无需安装 git-lfs
            "--config",
            "filter.lfs.process=",
            "--config",
            "filter.lfs.smudge=cat",
            "--config",
            "filter.lfs.required=false",
            REPO_URL,
            repo_path.to_str().context("路径含非 UTF-8 字符")?,
        ])
        .output()
        .await
        .context("[MCP] git clone 失败，请确认已安装 git")?;

    anyhow::ensure!(
        out.status.success(),
        "[MCP] git clone 非零退出 ({})\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    tracing::info!("[MCP] 克隆完成");
    Ok(())
}

// ─── 菜谱加载 ────────────────────────────────────────────────────────────────

/// 递归扫描指定目录，并将其内容加入菜谱 Map
fn scan_dir(
    dir: &Path,
    map: &mut HashMap<String, RecipeInfo>,
    // 如果为 None，则以第一层子目录名作为 category
    // 如果为 Some，则使用该值作为固定的 category 前缀
    fixed_category: Option<&str>,
) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    if let Some(cat) = fixed_category {
        // 固定分类模式：递归扫描该目录下所有 .md 文件
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current_path) = stack.pop() {
            for entry in std::fs::read_dir(current_path)?.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        map.insert(
                            name.to_string(),
                            RecipeInfo {
                                name: name.to_string(),
                                category: cat.to_string(),
                                content: std::fs::read_to_string(&path).unwrap_or_default(),
                            },
                        );
                    }
                }
            }
        }
    } else {
        // 第一层子目录作为分类模式
        for entry in std::fs::read_dir(dir)?.flatten() {
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let category = entry.file_name().to_string_lossy().to_string();
            let mut stack = vec![entry.path()];
            while let Some(current_path) = stack.pop() {
                for sub_entry in std::fs::read_dir(current_path)?.flatten() {
                    let path = sub_entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                            map.insert(
                                name.to_string(),
                                RecipeInfo {
                                    name: name.to_string(),
                                    category: category.clone(),
                                    content: std::fs::read_to_string(&path).unwrap_or_default(),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// 加载所有数据：菜谱 (dishes)、难度系统 (starsystem) 和 技巧指南 (tips)
pub async fn load_recipes(repo_dir: &Path) -> anyhow::Result<HashMap<String, RecipeInfo>> {
    let dir = repo_dir.to_path_buf();
    let recipes = tokio::task::spawn_blocking(move || {
        let mut map = HashMap::new();

        // 1. 加载菜谱 (dishes) - 按第一层目录分类
        if let Err(e) = scan_dir(&dir.join("dishes"), &mut map, None) {
            tracing::warn!("[MCP] 加载 dishes 失败: {e}");
        }

        // 2. 加载难度系统 (starsystem) - 固定分类 "难度等级"
        if let Err(e) = scan_dir(&dir.join("starsystem"), &mut map, Some("难度等级")) {
            tracing::warn!("[MCP] 加载 starsystem 失败: {e}");
        }

        // 3. 加载技巧指南 (tips) - 固定分类 "技巧指南"
        if let Err(e) = scan_dir(&dir.join("tips"), &mut map, Some("技巧指南")) {
            tracing::warn!("[MCP] 加载 tips 失败: {e}");
        }

        Ok::<_, std::io::Error>(map)
    })
    .await
    .context("文件扫描任务崩溃")?
    .context("读取菜谱目录失败")?;

    tracing::info!("[MCP] 总计加载了 {} 条内容（含菜谱、难度和技巧）", recipes.len());
    Ok(recipes)
}

// ─── 公共启动入口 ─────────────────────────────────────────────────────────────

pub async fn start(bind_addr: &str) -> anyhow::Result<()> {
    start_with_repo(bind_addr, DEFAULT_REPO_DIR).await
}

pub async fn start_with_repo(bind_addr: &str, repo_dir: &str) -> anyhow::Result<()> {
    let repo_path = Path::new(repo_dir);

    ensure_repo(repo_path).await?;

    let recipes = load_recipes(repo_path).await?;
    let server = Arc::new(HowToCookServer::new(recipes));

    let config = StreamableHttpServerConfig {
        stateful_mode: false,
        ..Default::default()
    };

    let mcp_service = StreamableHttpService::new(
        move || Ok((*Arc::clone(&server)).clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );

    let router = axum::Router::new().route_service(MCP_PATH, mcp_service);
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("[MCP] 绑定 {bind_addr} 失败"))?;

    tracing::info!("[MCP] ✅ 端点：http://{bind_addr}{MCP_PATH}");
    axum::serve(listener, router).await?;
    Ok(())
}
