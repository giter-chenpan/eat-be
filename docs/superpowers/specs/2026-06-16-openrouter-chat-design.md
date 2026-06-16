# OpenRouter 聊天集成 — 设计规格

**日期:** 2026-06-16
**状态:** 草案
**目标文件:** `src/api/chat.rs` (目前为空)

## 目标

为 `eatbe` 后端添加一个聊天 API,流式接收 OpenRouter 的 LLM 响应。会话和消息持久化到数据库。端点需要 JWT 鉴权,并按用户做限流。

## 范围

- 新增一个模块:`src/api/chat.rs`(完整的会话/消息 CRUD,以及一个流式发送端点)
- 新增一个顶层模块:`src/openrouter.rs`(基于 `reqwest` 的薄 HTTP 客户端)
- 新增两个 SeaORM 实体:`chat_sessions`、`chat_messages`
- 新增一个 migration:`m2025XXXX_create_chat_tables`(会被 `Migrator` 自动发现)
- 新增配置项:`openrouter_api_key`、`openrouter_model`、`chat_rate_limit_per_minute`
- 在 `Rocket.toml` / 环境变量中接入上述配置

## 非目标

- 系统提示词自定义(烹饪领域的 prompt 在模块内硬编码)
- 客户端可选模型(模型由配置固定)
- 工具/函数调用、视觉输入、JSON-mode 响应格式
- 前端 UI(本次只做后端)
- 单元/集成测试套件(项目本身没有测试约定,验证通过手动冒烟测试完成)

## 架构

### 组件

- **`src/api/chat.rs`** — 聊天端点的 Rocket 路由。以 SSE 形式将助手回复流式回传给客户端,同时并发地将用户消息和助手消息持久化到数据库。
- **`src/openrouter.rs`**(新增) — `pub async fn stream_chat(req: OpenRouterRequest) -> Result<reqwest::Response, OpenRouterError>`。负责对 `https://openrouter.ai/api/v1/chat/completions` 发起 HTTP 调用,设置 `stream: true`。返回原始 response,由调用方负责 pipe 其 body。
- **`entity/src/chat_session.rs`**、**`entity/src/chat_message.rs`**(新增) — SeaORM 实体。主键使用 UUIDv7(与项目其他表保持一致)。
- **Migration** — 新建上述两张表及对应索引。

### `POST /api/chat/sessions/<id>/messages`(流式发送)的数据流

1. 通过 `Claims` 鉴权(JWT,强制要求)。增加 Redis 限流计数;超过每分钟上限时以 `Rep` JSON 形式拒绝。
2. 根据 `id` 加载会话行。如果不存在或 `user_id != claims.sub`,返回 404。
3. 加载该会话最近的 20 条消息,按 `created_at ASC` 排序(过滤 `role IN ('user','assistant')` — 防御性写法,防止存在 `system` 行)。
4. 构造 OpenRouter 请求:messages = `[system, ...history, new user msg]`,`stream: true`,model 使用配置的默认值。
5. 发出请求。发生 HTTP/连接/超时失败时,立即返回 JSON `Rep` 错误,**不持久化**任何消息(我们不知道 LLM 是否收到了请求)。
6. 成功后,先将新的用户消息持久化到 `chat_messages`,`status='complete'`;再插入一条"待定"助手消息行(`status='pending'`,内容为空)。
7. 向客户端打开 SSE 响应(`Content-Type: text/event-stream`,`Cache-Control: no-cache`)。
8. 并发地 `tokio::spawn` 一个任务,消费 OpenRouter `reqwest::Response` 的字节流:
   - 每收到一个 SSE 事件,把 `delta.content` 追加到累加器,同时以 `data: <delta>\n\n` 形式转发给客户端。
   - 流结束时,`UPDATE` 待定助手行为完整内容 + `status='complete'`,然后向客户端发送 `data: [DONE]\n\n`。
   - 流出现错误时,把那行标记为 `status='failed'` 并写入错误文本,先发送 `data: {"error":"..."}\n\n` 再发送 `data: [DONE]\n\n`。
9. handler `return` 一个 Rocket `Stream<Bytes>`,由框架接管驱动。

数据库写入与流式响应并发执行,所以用户能在 OpenRouter 生成第一个 token 时立刻看到。handler 协调两侧:`tokio::spawn` 拥有写侧,响应流是读侧。

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
    role              TEXT NOT NULL,        -- 'user' | 'assistant'
    content           TEXT NOT NULL,
    status            TEXT NOT NULL,        -- 'complete' | 'pending' | 'failed'
    error             TEXT,                 -- 可空;当 status='failed' 时填充
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
| `POST` | `/api/chat/sessions/<id>/messages` | `{ "content": "..." }` | SSE 流 | 流式返回助手回复;同时持久化用户消息和助手消息 |

流式发送端点的响应是 `text/event-stream`,不使用标准的 `Rep<T>` 包装。

### SSE 事件格式

```
data: {"delta": "第一段内容 "}

data: {"delta": "的剩余部分。"}

data: [DONE]
```

中途出错时:

```
data: {"error": "上游连接中断"}

data: [DONE]
```

## 配置

### `Rocket.toml`(支持环境变量覆盖)

```toml
openrouter_api_key = "..."               # 通过环境变量 OPENROUTER_API_KEY 覆盖
openrouter_model = "openai/gpt-4o-mini"   # 通过环境变量 OPENROUTER_MODEL 覆盖
chat_rate_limit_per_minute = 20           # 通过环境变量 CHAT_RATE_LIMIT_PER_MINUTE 覆盖
```

`AppConfig::new()` 通过 `Config::figment().extract_inner::<...>()` 读取这些字段——与现有 `translation_url` 模式一致。未设置 `OPENROUTER_API_KEY` 时,启动会以清晰的 `expect` 消息失败。

### 系统提示词

在 `src/api/chat.rs` 模块顶部硬编码:

```rust
const SYSTEM_PROMPT: &str = "你是一个友好的烹饪助手,擅长根据用户的问题和已有的对话历史,给出实用、简洁的中文回答。";
```

由服务端注入,客户端不可控制。客户端传入的内容只作为 user 角色处理。

## 错误处理

### 类别

1. **鉴权 / 校验 / 限流**(尚未开始流式响应)——使用现有的 `Code` 枚举(`Unauthorized`、`BadRequest`、`BusinessError`)返回标准 `Rep<()>` JSON。

2. **OpenRouter 流前失败**(非 2xx、网络错误、收到首个响应前的超时)——同样的 JSON 格式。**不持久化**任何用户消息或助手消息(我们不知道 LLM 是否收到请求)。如果 OpenRouter 返回 200 但第一个 SSE 事件中带有错误,按中途失败(类别 3)处理——用户和待定助手消息都已持久化,助手行随后被更新为 `status='failed'`。

3. **中途失败**(连接断开、部分响应)——流已经打开。发送最后一个 `data: {"error": "..."}\n\n` 事件,接着发送 `data: [DONE]\n\n`。把助手行标记为 `status='failed'`,并写入已累积的内容和错误文本。

4. **客户端断开**——OpenRouter 流在收到干净的结束事件前结束。把已累积的内容以 `status='complete'` 持久化(尽力而为);若没有累积到任何内容,则以 `error="client disconnected"` 标记为 `status='failed'`。

5. **启动时缺少配置**——在 `AppConfig::new()` 中以清晰的 `expect("OPENROUTER_API_KEY env var must be set")` 报错。与现有 `translation_url` 模式一致。

6. **Redis 限流失败**——失败开放(放行请求),记录警告。限流是尽力而为的保护;聊天功能不应该因为 Redis 挂了就不可用。

### 数据库写入失败

尽力而为。助手消息最终的 `UPDATE` 是副作用,不属于用户可见的响应契约的一部分。失败时记录日志并继续——用户已经拿到流式回复;在回复之后又抛出一个 DB 错误会让人困惑。

## 验证计划

不写自动化测试。手动冒烟测试:

1. **编译检查** — 工作空间内执行 `cargo build`,必须通过。
2. **创建** — `POST /api/chat/sessions` 返回一个新会话。
3. **列表** — `GET /api/chat/sessions` 中包含新建的会话。
4. **流式** — 用 `curl -N` 调 `POST /api/chat/sessions/<id>/messages`,得到一串 `data: {...}` 事件并以 `data: [DONE]` 结尾。
5. **持久化** — `GET /api/chat/sessions/<id>/messages` 同时看到用户消息和完整的助手消息。
6. **删除** — `DELETE /api/chat/sessions/<id>` 返回 200;再 GET 返回 404。
7. **鉴权边界** — 用其他用户的 session_id 发请求,返回 404(不是 403,避免泄露存在性)。
8. **限流** — 一分钟内发 21 次请求,第 21 次返回限流错误。
9. **OpenRouter 失败** — 配一个错误的 API key,发送端点返回 JSON 错误而非流,且不会持久化任何消息行(流前失败场景)。

## 待澄清问题

草案阶段暂无。
