// Comment out or remove unused anyhow import
// use anyhow::anyhow;
use reqwest;
use rust_mcp_schema::{
    CallToolResult,
    schema_utils::CallToolError,
};
use rust_mcp_sdk::{
    macros::{mcp_tool, JsonSchema},
    tool_box,
};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct SimpleError(String);

/// Helper to wrap string-like errors into CallToolError
fn to_call_error<E: std::fmt::Display>(e: E) -> CallToolError {
    CallToolError::new(SimpleError(e.to_string()))
}

const GITHUB_API_BASE: &str = "https://api.github.com/repos/Anduin2017/HowToCook";
const RAW_CONTENT_BASE: &str = "https://raw.githubusercontent.com/Anduin2017/HowToCook/master";

/// Helper function to create an HTTP client with proper headers
fn create_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("HowToCook-MCP-Server/0.1.0")
        .build()
        .unwrap_or_default()
}

// ========================
//  ListCategoriesTool
// ========================
#[mcp_tool(
    name = "list_categories",
    description = "列出 HowToCook 仓库中所有菜谱分类目录。返回菜谱的分类列表，如素菜、荤菜、早餐、主食、汤与粥等。"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ListCategoriesTool {}

impl ListCategoriesTool {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let client = create_client();
        let url = format!("{}/contents/dishes", GITHUB_API_BASE);

        let response = client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(to_call_error)?;

        let items: Vec<Value> = response
            .json()
            .await
            .map_err(to_call_error)?;

        let mut categories = Vec::new();
        // Category name mapping for better readability
        let name_map: Vec<(&str, &str)> = vec![
            ("vegetable_dish", "素菜"),
            ("meat_dish", "荤菜"),
            ("aquatic", "水产"),
            ("breakfast", "早餐"),
            ("staple", "主食"),
            ("semi-finished", "半成品加工"),
            ("soup", "汤与粥"),
            ("drink", "饮料"),
            ("condiment", "酱料和其它材料"),
            ("dessert", "甜品"),
        ];

        for item in &items {
            if item["type"].as_str() == Some("dir") {
                let dir_name = item["name"].as_str().unwrap_or("unknown");
                let chinese_name = name_map
                    .iter()
                    .find(|(en, _)| *en == dir_name)
                    .map(|(_, cn)| *cn)
                    .unwrap_or(dir_name);
                categories.push(format!("📁 {} ({})", chinese_name, dir_name));
            }
        }

        if categories.is_empty() {
            categories.push("未找到任何菜谱分类。".to_string());
        }

        let result = format!(
            "🍳 程序员做饭指南 - 菜谱分类\n\
             ================================\n\
             共 {} 个分类：\n\n{}",
            categories.len(),
            categories.join("\n")
        );

        Ok(CallToolResult::text_content(result, None))
    }
}

// ========================
//  ListRecipesTool
// ========================
#[mcp_tool(
    name = "list_recipes",
    description = "列出指定分类下的所有菜谱名称。需要提供分类目录名，如 vegetable_dish（素菜）、meat_dish（荤菜）、aquatic（水产）、breakfast（早餐）、staple（主食）、semi-finished（半成品加工）、soup（汤与粥）、drink（饮料）、condiment（酱料和其它材料）、dessert（甜品）。"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ListRecipesTool {
    /// 分类目录名称，如 vegetable_dish, meat_dish, aquatic, breakfast, staple, semi-finished, soup, drink, condiment, dessert
    category: String,
}

impl ListRecipesTool {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let client = create_client();
        let url = format!("{}/contents/dishes/{}", GITHUB_API_BASE, self.category);

        let response = client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(to_call_error)?;

        if !response.status().is_success() {
            return Err(to_call_error(format!(
                "分类 '{}' 不存在或请求失败 (状态码: {})",
                self.category,
                response.status()
            )));
        }

        let items: Vec<Value> = response
            .json()
            .await
            .map_err(to_call_error)?;

        let mut recipes = Vec::new();
        for item in &items {
            let name = item["name"].as_str().unwrap_or("unknown");
            let item_type = item["type"].as_str().unwrap_or("");
            // Handle both .md files and directories containing .md files
            if item_type == "file" && name.ends_with(".md") {
                let recipe_name = name.trim_end_matches(".md");
                recipes.push(format!("🍽️  {}", recipe_name));
            } else if item_type == "dir" {
                recipes.push(format!("🍽️  {}", name));
            }
        }

        if recipes.is_empty() {
            recipes.push("该分类下未找到菜谱。".to_string());
        }

        let category_cn = match self.category.as_str() {
            "vegetable_dish" => "素菜",
            "meat_dish" => "荤菜",
            "aquatic" => "水产",
            "breakfast" => "早餐",
            "staple" => "主食",
            "semi-finished" => "半成品加工",
            "soup" => "汤与粥",
            "drink" => "饮料",
            "condiment" => "酱料和其它材料",
            "dessert" => "甜品",
            _ => &self.category,
        };

        let result = format!(
            "🍳 {} ({}) - 菜谱列表\n\
             ================================\n\
             共 {} 道菜谱：\n\n{}",
            category_cn,
            self.category,
            recipes.len(),
            recipes.join("\n")
        );

        Ok(CallToolResult::text_content(result, None))
    }
}

// ========================
//  GetRecipeTool
// ========================
#[mcp_tool(
    name = "get_recipe",
    description = "获取指定菜谱的详细内容（Markdown 格式）。需要提供分类目录名和菜谱名称。例如：分类为 meat_dish，菜谱名为 红烧鸡翅。"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct GetRecipeTool {
    /// 分类目录名称，如 vegetable_dish, meat_dish 等
    category: String,
    /// 菜谱名称（中文名），如 红烧鸡翅、番茄炒蛋
    recipe_name: String,
}

impl GetRecipeTool {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let client = create_client();

        // First, list the directory to find the recipe file
        let dir_url = format!("{}/contents/dishes/{}", GITHUB_API_BASE, self.category);
        let dir_response = client
            .get(&dir_url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(to_call_error)?;

        let items: Vec<Value> = dir_response
            .json()
            .await
            .map_err(to_call_error)?;

        // Try to find recipe - it could be a .md file or a subdirectory
        let mut recipe_path = None;

        for item in &items {
            let name = item["name"].as_str().unwrap_or("");
            let item_type = item["type"].as_str().unwrap_or("");

            // Direct .md file match
            if item_type == "file" && name.ends_with(".md") {
                let base_name = name.trim_end_matches(".md");
                if base_name == self.recipe_name {
                    recipe_path = Some(item["path"].as_str().unwrap_or("").to_string());
                    break;
                }
            }
            // Directory match - look inside for .md file
            if item_type == "dir" && name == self.recipe_name {
                // Look inside the directory for the .md file
                let sub_url = format!(
                    "{}/contents/dishes/{}/{}",
                    GITHUB_API_BASE, self.category, name
                );
                if let Ok(sub_response) = client
                    .get(&sub_url)
                    .header("Accept", "application/vnd.github.v3+json")
                    .send()
                    .await
                {
                    if let Ok(sub_items) = sub_response.json::<Vec<Value>>().await {
                        for sub_item in &sub_items {
                            let sub_name = sub_item["name"].as_str().unwrap_or("");
                            if sub_name.ends_with(".md") {
                                recipe_path =
                                    Some(sub_item["path"].as_str().unwrap_or("").to_string());
                                break;
                            }
                        }
                    }
                }
                break;
            }
        }

        let path = recipe_path.ok_or_else(|| {
            to_call_error(format!(
                "未找到菜谱 '{}' (分类: {})",
                self.recipe_name, self.category
            ))
        })?;

        // Fetch the recipe content via raw URL
        let raw_url = format!(
            "{}/{}",
            RAW_CONTENT_BASE,
            path
        );
        let content_response = client
            .get(&raw_url)
            .send()
            .await
            .map_err(to_call_error)?;

        let content = content_response
            .text()
            .await
            .map_err(to_call_error)?;

        let result = format!(
            "📖 菜谱: {}\n📁 分类: {}\n🔗 来源: https://github.com/Anduin2017/HowToCook/blob/master/{}\n\
             ================================\n\n{}",
            self.recipe_name, self.category, path, content
        );

        Ok(CallToolResult::text_content(result, None))
    }
}

// ========================
//  SearchRecipesTool
// ========================
#[mcp_tool(
    name = "search_recipes",
    description = "在 HowToCook 仓库中搜索菜谱。根据关键词搜索菜谱名称。例如：搜索 '鸡蛋' 会返回所有包含 '鸡蛋' 的菜谱。"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SearchRecipesTool {
    /// 搜索关键词，如 "鸡蛋"、"红烧"、"豆腐" 等
    keyword: String,
}

impl SearchRecipesTool {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let client = create_client();

        // Use GitHub search API to find matching files
        let search_url = format!(
            "https://api.github.com/search/code?q={}+in:path+repo:Anduin2017/HowToCook+path:dishes+extension:md",
            self.keyword
        );

        let response = client
            .get(&search_url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(to_call_error)?;

        if !response.status().is_success() {
            // Fallback: iterate through categories and search
            return self.fallback_search(&client).await;
        }

        let search_result: Value = response
            .json()
            .await
            .map_err(to_call_error)?;

        let items = search_result["items"].as_array();
        let mut results = Vec::new();

        if let Some(items) = items {
            for item in items.iter().take(20) {
                let name = item["name"].as_str().unwrap_or("unknown");
                let path = item["path"].as_str().unwrap_or("");
                if name.ends_with(".md") && name != "README.md" {
                    let recipe_name = name.trim_end_matches(".md");
                    results.push(format!("🍽️  {} (📁 {})", recipe_name, path));
                }
            }
        }

        if results.is_empty() {
            // Try fallback search
            return self.fallback_search(&client).await;
        }

        let result = format!(
            "🔍 搜索关键词: \"{}\"\n\
             ================================\n\
             找到 {} 个结果：\n\n{}",
            self.keyword,
            results.len(),
            results.join("\n")
        );

        Ok(CallToolResult::text_content(result, None))
    }

    async fn fallback_search(&self, client: &reqwest::Client) -> Result<CallToolResult, CallToolError> {
        let categories = vec![
            "vegetable_dish", "meat_dish", "aquatic", "breakfast",
            "staple", "semi-finished", "soup", "drink", "condiment", "dessert",
        ];

        let mut results = Vec::new();

        for category in categories {
            let url = format!("{}/contents/dishes/{}", GITHUB_API_BASE, category);
            if let Ok(response) = client
                .get(&url)
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .await
            {
                if let Ok(items) = response.json::<Vec<Value>>().await {
                    for item in &items {
                        let name = item["name"].as_str().unwrap_or("");
                        let decoded_name = if name.ends_with(".md") {
                            name.trim_end_matches(".md")
                        } else {
                            name
                        };
                        if decoded_name.contains(&self.keyword) {
                            let category_cn = match category {
                                "vegetable_dish" => "素菜",
                                "meat_dish" => "荤菜",
                                "aquatic" => "水产",
                                "breakfast" => "早餐",
                                "staple" => "主食",
                                "semi-finished" => "半成品加工",
                                "soup" => "汤与粥",
                                "drink" => "饮料",
                                "condiment" => "酱料和其它材料",
                                "dessert" => "甜品",
                                _ => category,
                            };
                            results.push(format!(
                                "🍽️  {} (📁 {})",
                                decoded_name, category_cn
                            ));
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            results.push(format!("未找到包含 \"{}\" 的菜谱。", self.keyword));
        }

        let result = format!(
            "🔍 搜索关键词: \"{}\"\n\
             ================================\n\
             找到 {} 个结果：\n\n{}",
            self.keyword,
            results.len(),
            results.join("\n")
        );

        Ok(CallToolResult::text_content(result, None))
    }
}

// ========================
//  GetTipsTool
// ========================
#[mcp_tool(
    name = "get_tips",
    description = "获取烹饪技巧和进阶知识。提供 HowToCook 仓库中的烹饪小贴士，包括厨房准备、食材选购、刀工技巧等。"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct GetTipsTool {
    /// 可选，指定要查看的技巧文件名。留空则列出所有可用技巧。
    #[serde(default)]
    tip_name: Option<String>,
}

impl GetTipsTool {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let client = create_client();

        match &self.tip_name {
            Some(name) if !name.is_empty() => {
                // Fetch specific tip
                let raw_url = format!("{}/tips/{}.md", RAW_CONTENT_BASE, name);
                let response = client
                    .get(&raw_url)
                    .send()
                    .await
                    .map_err(to_call_error)?;

                if !response.status().is_success() {
                    // Try the tips/tips-for-xxx directory
                    let dir_url = format!("{}/contents/tips", GITHUB_API_BASE);
                    let dir_response = client
                        .get(&dir_url)
                        .header("Accept", "application/vnd.github.v3+json")
                        .send()
                        .await
                        .map_err(to_call_error)?;

                    let items: Vec<Value> = dir_response
                        .json()
                        .await
                        .map_err(to_call_error)?;

                    // Search for matching file
                    for item in &items {
                        let item_name = item["name"].as_str().unwrap_or("");
                        if item_name.contains(name) && item_name.ends_with(".md") {
                            let raw_url = format!("{}/tips/{}", RAW_CONTENT_BASE, item_name);
                            if let Ok(resp) = client.get(&raw_url).send().await {
                                if let Ok(content) = resp.text().await {
                                    return Ok(CallToolResult::text_content(format!(
                                        "💡 烹饪技巧: {}\n================================\n\n{}",
                                        item_name.trim_end_matches(".md"),
                                        content
                                    ), None));
                                }
                            }
                        }
                    }

                    return Err(to_call_error(format!("未找到技巧文件: {}", name)));
                }

                let content = response
                    .text()
                    .await
                    .map_err(to_call_error)?;

                Ok(CallToolResult::text_content(format!(
                    "💡 烹饪技巧: {}\n================================\n\n{}",
                    name, content
                ), None))
            }
            _ => {
                // List all tips
                let url = format!("{}/contents/tips", GITHUB_API_BASE);
                let response = client
                    .get(&url)
                    .header("Accept", "application/vnd.github.v3+json")
                    .send()
                    .await
                    .map_err(to_call_error)?;

                let items: Vec<Value> = response
                    .json()
                    .await
                    .map_err(to_call_error)?;

                let mut tips = Vec::new();
                for item in &items {
                    let name = item["name"].as_str().unwrap_or("unknown");
                    if name.ends_with(".md") {
                        let tip_name = name.trim_end_matches(".md");
                        tips.push(format!("💡 {}", tip_name));
                    }
                }

                if tips.is_empty() {
                    tips.push("未找到任何烹饪技巧。".to_string());
                }

                let result = format!(
                    "💡 烹饪技巧列表\n\
                     ================================\n\
                     共 {} 个技巧：\n\n{}\n\n\
                     提示: 使用 get_tips 工具并指定 tip_name 参数来查看具体内容。",
                    tips.len(),
                    tips.join("\n")
                );

                Ok(CallToolResult::text_content(result, None))
            }
        }
    }
}

// ========================
//  GetReadmeTool
// ========================
#[mcp_tool(
    name = "get_readme",
    description = "获取 HowToCook 仓库的 README 主页内容，包含全部菜谱索引和项目介绍。"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct GetReadmeTool {}

impl GetReadmeTool {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let client = create_client();
        let url = format!("{}/README.md", RAW_CONTENT_BASE);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(to_call_error)?;

        let content = response
            .text()
            .await
            .map_err(to_call_error)?;

        let result = format!(
            "📖 HowToCook - 程序员做饭指南\n\
             🔗 https://github.com/Anduin2017/HowToCook\n\
             ================================\n\n{}",
            content
        );

        Ok(CallToolResult::text_content(result, None))
    }
}

// ========================
//  Tool Box
// ========================
tool_box!(CookbookTools, [
    ListCategoriesTool,
    ListRecipesTool,
    GetRecipeTool,
    SearchRecipesTool,
    GetTipsTool,
    GetReadmeTool
]);
