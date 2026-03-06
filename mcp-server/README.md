# HowToCook MCP Server 🍳

> **让 AI 成为你的私人主厨** —— 一个基于 [Model Context Protocol (MCP)](https://modelcontextprotocol.io) 的 Rust 实现，深度集成 [HowToCook（程序员做饭指南）](https://github.com/Anduin2017/HowToCook) 仓库。

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/protocol-MCP-blue.svg)](https://modelcontextprotocol.io)
[![License](https://img.shields.io/badge/license-Unlicense-green.svg)](https://unlicense.org)

## ✨ 通读指南

本服务允许 AI 模型直接检索和分析 GitHub 上的“程序员做饭指南”。无论是想寻找周五晚上的浪漫晚餐（比如**可乐鸡翅**），还是想学习如何科学地**处理食材**，AI 都能通过这些工具为你提供实时、准确的建议。

## 🛠️ 核心工具

| 工具名称 | 描述 |
|:---|:---|
| `list_categories` | **浏览分类**：列出所有菜谱目录（素菜、荤菜、水产、早餐等） |
| `list_recipes` | **查找菜谱**：展示指定分类下的所有精品菜谱名称 |
| `get_recipe` | **获取做法**：检索完整 Markdown 内容，包括原料清单、计算公式及操作步骤 |
| `search_recipes` | **全局搜索**：根据关键词（如“鸡蛋”、“牛肉”）跨分类搜索匹配的菜谱 |
| `get_tips` | **大厨秘笈**：获取库中的烹饪技巧、刀工及食材处理等进阶知识 |
| `get_readme` | **项目概览**：读取仓库主页，获取整体索引和项目介绍 |

## 🚀 运行与配置

本服务器支持 **SSE (Server-Sent Events)** 和 **Stdio** 两种通信模式。

### 1. 构建项目

确保已安装 [Rust](https://www.rust-lang.org/tools/install)：

```bash
cd mcp-server
cargo build --release
```

### 2. SSE 模式配置（推荐）

当你在根目录运行 `cargo run` 时，SSE 服务器会自动在 `8081` 端口启动。

#### Claude Desktop 配置
编辑 `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "howtocook-sse": {
      "url": "http://127.0.0.1:8081/sse"
    }
  }
}
```

### 3. Stdio 模式配置

如果你希望将其作为本地二进制脚本调用：

#### Cursor 配置
编辑 `.cursor/mcp.json` (或在 GUI 设置中添加):

```json
{
  "mcpServers": {
    "howtocook": {
      "command": "/你的绝对路径/eat-be/target/release/eatbe",
      "args": []
    }
  }
}
```

## 🍱 示例场景

> [!TIP]
> **场景 1 (浪漫晚餐)**: "今天是周五，推荐两道适合两个人的菜，并告诉我具体做法。"
>
> **场景 2 (精准搜索)**: "我想用冰箱里的西红柿和牛肉做点什么。"
>
> **场景 3 (技能提升)**: "有哪些关于处理肉类的刀工技巧？"

## 🛡️ 技术栈

- **Rust** + **Tokio** (高效异步运行时)
- **rust-mcp-sdk** (现代 MCP 协议适配)
- **Hyper** (SSE 高性能传输端点)
- **Reqwest** (与 GitHub API 流畅交互)

## 🤝 许可证与贡献

本项目遵循 **Unlicense** 许可证（与 [HowToCook](https://github.com/Anduin2017/HowToCook) 保持一致）。欢迎提交 Issue 或 Pull Request！

---
*Powered by DeepSeek / Gemini via Antigravity 🚀*
