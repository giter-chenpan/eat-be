# OpenRouter 聊天集成 — 设计规格

**日期:** 2026-06-16
**状态:** 草案
**目标文件:** `src/api/chat.rs`(目前为空)

## 目标

为 `eatbe` 后端添加一个聊天 API,流式接收 OpenRouter 的 LLM 响应,并将本仓库的 `mcp-server`(HowToCook 中文菜谱库)以工具调用的形式接入,让 LLM 在回答时能按需查询菜谱、分类、技巧。**会话和消息持久化到数据库,工具调用与结果也会持久化**,方便历史回放与调试。端点需要 JWT 鉴权,并按用户做限流。

## 范围

- 新增一个模块:`src/api/chat.rs`(完整的会话/消息 CRUD,以及一个流式发送端点)
- 新增一个顶层模块:`src/openrouter.rs`(基于 `reqwest` 的薄 HTTP 客户端,支持 tool 字段)
- 新增一个顶层模块:`src/mcp_client.rs`(调用本机 `mcp-server` 的 MCP 客户端,复用 `rmcp` 库)
- 新增两个 SeaORM 实体:`chat_sessions`、`chat_messages`
- 新增一个 migration:`m2025XXXX_create_chat_tables`(会被 `Migrator` 自动发现)
- 新增配置项:`openrouter_api_key`、`openrouter_model`、`chat_rate_limit_per_minute`、`chat_max_tool_iterations`、`mcp_server_url`
- 在 `Rocket.toml` / 环境变量中接入上述配置

## 非目标

- 系统提示词自定义(烹饪领域的 prompt 在模块内硬编码)
- 客户端可选模型(模型由配置固定)
- 工具/函数调用、视觉输入、JSON-mode 响应格式(本次只使用工具调用,不涉及其他高级特性)
- 前端 UI(本次只做后端)
- 单元/集成测试套件(项目本身没有测试约定,验证通过手动冒烟测试完成)
- 让客户端看到工具调用过程(本次不向前端暴露工具事件)

## 架构

### 组件

- **`src/api/chat.rs`** — 聊天端点的 Rocket 路由。协调 OpenRouter 与 mcp-server,实现带工具调用的 ReAct 循环;以 SSE 形式将助手**最终**回复流式回传给客户端;并发地将用户消息、工具调用、工具结果、助手消息持久化到数据库。
- **`src/openrouter.rs`**(新增) — OpenRouter HTTP 客户端。提供 `stream_chat(req: OpenRouterRequest) -> Result<reqwest::Response, OpenRouterError>` 与 `build_tools_payload() -> serde_json::Value`(把 4 个 MCP 工具的定义序列化为 OpenRouter 的 `tools` 字段)。负责对 `https://openrouter.ai/api/v1/chat/completions` 发起 HTTP 调用,支持 `stream: true`。返回原始 response,由调用方负责 pipe 其 body。
- **`src/mcp_client.rs`**(新增) — 连接到本机 `mcp-server`(`http://<mcp_server_url>/mcp`)的 MCP 客户端,使用 `rmcp` 库。提供 `call_tool(name, args) -> Result<String, McpError>` 同步方法,内部走 rmcp 客户端的 streamable HTTP transport。在 `main.rs` 启动时建立长连接并共享给所有请求处理器(通过 Rocket 的状态)。
- **`entity/src/chat_session.rs`**、**`entity/src/chat_message.rs`**(新增) — SeaORM 实体。主键使用 UUIDv7(与项目其他表保持一致)。
- **Migration** — 新建上述两张表及对应索引。

### 关键依赖

`Cargo.toml` 新增 `rmcp` 客户端依赖(版本与 `mcp-server` 一致,`0.15`):

```toml
rmcp = { version = "0.15", features = ["client", "transport-streamable-http-client"] }
```

`reqwest` 已经在依赖中,不需要新增。

### `POST /api/chat/sessions/<id>/messages`(流式发送)的数据流

整个流程被组织成 **ReAct 循环**,每个回合都包含一次 OpenRouter 流式请求,但只有**最后一个回合**的内容会流给客户端;中间回合(工具调用)对客户端不可见。

```
loop iteration 1..N (N ≤ chat_max_tool_iterations):
  1. 鉴权(JWT) + 限流(Redis 计数器,每次 user message 仅 +1,不限 OpenRouter 调用次数)
  2. 加载会话,校验所有权(404 if not owner)
  3. 加载该会话最近 20 条历史消息
  4. 构造 OpenRouter 请求:
     - messages = [system, ...history, new user msg]
     - tools = mcp_client 的 4 个工具定义
     - stream = true
     - model = 配置的默认值
  5. 调用 OpenRouter,消费流式响应
  6. 若流中包含 tool_calls 事件:
     a. 解析所有 tool_calls(name + arguments + id)
     b. 对每个 tool_call:
        - 持久化一条 role='assistant'、status='complete'、附带 tool_name/tool_args/tool_call_id 的消息
        - 同步调用 mcp_client::call_tool(name, args) 拿到结果文本
        - 持久化一条 role='tool'、status='complete'、附带 tool_call_id 与 content(结果)的消息
     c. 失败时:把 tool_call 持久化为 role='assistant'、status='failed';把"工具调用失败"作为 role='tool' 的 content 返回给 LLM,让它回应一句道歉
     d. 继续下一次 iteration
  7. 若流中只包含文本 delta(没有 tool_calls 或 stream-end 前的 finish_reason='stop'):
     a. 累积所有 delta
     b. 持久化一条 role='assistant'、status='complete'、content=完整回复 的消息
     c. 把累积文本通过 SSE 推给客户端(以 `data: {"delta": "..."}\n\n` 形式)
     d. 发送 `data: [DONE]\n\n`,退出循环
  8. 达到 chat_max_tool_iterations 仍未拿到纯文本:强制流一段固定消息"我尝试了多次仍无法得到答案,请换个问法",持久化,退出
```

**对客户端的 SSE 格式**(本次不暴露工具调用过程):

```
data: {"delta": "你好"}

data: {"delta": ",推荐你做"}

data: {"delta": "西红柿炒鸡蛋。"}

data: [DONE]
```

如果中途失败:

```
data: {"error": "upstream connection lost"}

data: [DONE]
```

### 首次流前失败(OpenRouter 在 iteration 1 之前就失败)的处理

- 仅**不持久化**任何消息(同上一版 spec 的处理)。
- 如果是 iteration 2..N 才发生(说明已经有持久化的历史工具调用/结果),也不回滚;把当前的 assistant 消息行(若有)标记为 `status='failed'` 并继续。

## 数据模型

```sql
CREATE TABLE chat_sessions (
    id          TEXT PRIMARY KEY,           -- UUIDv7
    user_id     TEXT NOT NULL,              -- JWT 中的 sub
    title       TEXT NOT NULL,              -- 第一条用户消息,截断到 50 字
    created_at  TEXT NOT NULL,              -- RFC3339
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_chat_sessions_user_updated
    ON chat_sessions(user_id, updated_at DESC);

CREATE TABLE chat_messages (
    id                TEXT PRIMARY KEY,     -- UUIDv7
    session_id        TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role              TEXT NOT NULL,        -- 'user' | 'assistant' | 'tool'
    content           TEXT NOT NULL,
    status            TEXT NOT NULL,        -- 'complete' | 'pending' | 'failed'
    error             TEXT,                 -- 可空;当 status='failed' 时填充
    tool_call_id      TEXT,                 -- 可空;关联 OpenRouter 的 tool_call.id
    tool_name         TEXT,                 -- 可空;当 role='tool' 或 assistant 触发工具时填充
    tool_args         TEXT,                 -- 可空;JSON 序列化的工具参数
    prompt_tokens     INTEGER,
    completion_tokens INTEGER,
    created_at        TEXT NOT NULL         -- RFC3339
);

CREATE INDEX idx_chat_messages_session_created
    ON chat_messages(session_id, created_at);
```

## API 列表

所有端点都需要有效的 JWT(`Claims` 提取器)。统一使用 `Rep<T>` JSON 响应包装(参考现有的 `translation.rs` 和 `category.rs`)。

| 方法 | 路径 | 请求体 | 响应 | 说明 |
|---|---|---|---|---|
| `POST` | `/api/chat/sessions` | `{ "title": "可选" }` | `Rep<ChatSession>` | 不传 title 时,从首条用户消息自动生成 |
| `GET` | `/api/chat/sessions?page=1&page_size=20` | — | `Rep<ChatSessionListRep>` | 分页,按 `updated_at DESC`。仅返回当前调用者的会话。 |
| `GET` | `/api/chat/sessions/<id>` | — | `Rep<ChatSession>` | 非拥有者返回 404 |
| `DELETE` | `/api/chat/sessions/<id>` | — | `Rep<()>` | 级联删除消息 |
| `GET` | `/api/chat/sessions/<id>/messages?page=1&page_size=50` | — | `Rep<ChatMessageListRep>` | 分页,按时间正序 |
| `POST` | `/api/chat/sessions/<id>/messages` | `{ "content": "..." }` | SSE 流 | 流式返回助手最终回复;持久化用户消息、工具调用、工具结果、助手消息 |

`GET /api/chat/sessions/<id>/messages` 的响应中,客户端可以看到完整的消息历史(含 `tool` 角色的消息),但 SSE 流本身不暴露工具事件。

## 配置

### `Rocket.toml`(支持环境变量覆盖)

```toml
openrouter_api_key = "..."               # 通过环境变量 OPENROUTER_API_KEY 覆盖
openrouter_model = "openai/gpt-4o-mini"   # 通过环境变量 OPENROUTER_MODEL 覆盖
chat_rate_limit_per_minute = 20           # 通过环境变量 CHAT_RATE_LIMIT_PER_MINUTE 覆盖
chat_max_tool_iterations = 5              # 通过环境变量 CHAT_MAX_TOOL_ITERATIONS 覆盖
mcp_server_url = "http://127.0.0.1:8081"  # 通过环境变量 MCP_SERVER_URL 覆盖
```

`AppConfig::new()` 通过 `Config::figment().extract_inner::<...>()` 读取这些字段——与现有 `translation_url` 模式一致。未设置 `OPENROUTER_API_KEY` 时,启动会以清晰的 `expect` 消息失败。

### 系统提示词

在 `src/api/chat.rs` 模块顶部硬编码:

```rust
const SYSTEM_PROMPT: &str = "你是一个友好的中文烹饪助手。你可以通过调用工具查询菜谱、分类、技巧。\
请先判断用户问题是否需要查询菜谱库:\
- 需要时,调用 search_recipes / list_recipes / list_categories / get_recipe 查询;\
- 不需要时(例如寒暄、通用烹饪知识)直接回答;\
- 用简洁、可操作的中文回答,涉及具体菜谱时引用工具返回的菜名。";
```

由服务端注入,客户端不可控制。客户端传入的内容只作为 user 角色处理。

## 错误处理

### 类别

1. **鉴权 / 校验 / 限流**(尚未开始流式响应)——使用现有的 `Code` 枚举(`Unauthorized`、`BadRequest`、`BusinessError`)返回标准 `Rep<()>` JSON。

2. **OpenRouter 流前失败**(非 2xx、网络错误、收到首个响应前的超时,且发生在 iteration 1)——同样的 JSON 格式。**不持久化**任何用户消息或助手消息(我们不知道 LLM 是否收到请求)。如果 OpenRouter 返回 200 但第一个 SSE 事件中带有错误,按中途失败(类别 3)处理——用户和待定助手消息都已持久化,助手行随后被更新为 `status='failed'`。

3. **OpenRouter 中途失败**(iteration 1 之后的流中断、连接丢失)——流已经打开。发送最后一个 `data: {"error": "..."}\n\n` 事件,接着发送 `data: [DONE]\n\n`。**已经持久化**的工具调用/结果**不回滚**(它们是真实发生的事件)。如果当前 iteration 的 assistant 消息行已创建,标记为 `status='failed'`。

4. **MCP 工具调用失败**(mcp-server 连接失败、工具返回错误)——把 tool_call 持久化为 `status='failed'`,把"工具调用失败:{错误描述}"作为 `role='tool'` 的 content 返回给 LLM,继续下一次 iteration。LLM 通常会向用户道歉并提供尽力而为的回答。如果连续 2 次工具调用都失败,直接给客户端返回通用错误消息"暂时无法查询菜谱库",结束 SSE。

5. **达到 max iterations 仍未拿到纯文本**——把最后一次 assistant 消息标记为 `status='failed'`,流一段固定文本"我尝试了多次仍无法得到答案,请换个问法"给客户端,持久化为 `status='complete'`。

6. **客户端断开**——OpenRouter 流在收到干净的结束事件前结束。把已累积的内容以 `status='complete'` 持久化(尽力而为);若没有累积到任何内容,则以 `error="client disconnected"` 标记为 `status='failed'`。

7. **启动时缺少配置**——在 `AppConfig::new()` 中以清晰的 `expect("OPENROUTER_API_KEY env var must be set")` 报错。与现有 `translation_url` 模式一致。

8. **Redis 限流失败**——失败开放(放行请求),记录警告。限流是尽力而为的保护;聊天功能不应该因为 Redis 挂了就不可用。

### 数据库写入失败

尽力而为。助手消息最终的 `UPDATE` 是副作用,不属于用户可见的响应契约的一部分。失败时记录日志并继续——用户已经拿到流式回复;在回复之后又抛出一个 DB 错误会让人困惑。

## 验证计划

不写自动化测试。手动冒烟测试:

1. **编译检查** — 工作空间内执行 `cargo build`,必须通过。
2. **创建** — `POST /api/chat/sessions` 返回一个新会话。
3. **列表** — `GET /api/chat/sessions` 中包含新建的会话。
4. **普通对话流式** — `POST /api/chat/sessions/<id>/messages` body 为 "你好",用 `curl -N` 得到一串 `data: {...}` 事件并以 `data: [DONE]` 结尾。
5. **工具调用流式** — body 为 "推荐一道用鸡蛋做的菜",观察 SSE 流;`GET /messages` 应该看到 `user → assistant(tool_call) → tool(result) → assistant(final)` 四条消息。
6. **持久化** — `GET /api/chat/sessions/<id>/messages` 看到完整历史。
7. **删除** — `DELETE /api/chat/sessions/<id>` 返回 200;再 GET 返回 404。
8. **鉴权边界** — 用其他用户的 session_id 发请求,返回 404(不是 403,避免泄露存在性)。
9. **限流** — 一分钟内发 21 次请求,第 21 次返回限流错误。
10. **OpenRouter 失败** — 配一个错误的 API key,发送端点返回 JSON 错误而非流,且不会持久化任何消息行(流前失败场景)。
11. **MCP 失败** — 关闭 mcp-server 后发送请求,观察 SSE 是否流出一句道歉,且 `GET /messages` 中对应 `tool` 角色消息的 status='failed' 或 content 含错误描述。
12. **Iteration 上限** — 构造一个让 LLM 反复触发工具调用的 prompt(或不依赖 LLM,直接验证配置文件:把 `chat_max_tool_iterations=1` 后,即便 LLM 想调工具也只能调一次)。

## 待澄清问题

草案阶段暂无。
