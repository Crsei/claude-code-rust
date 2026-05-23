# mcp/client.rs 重构计划

> 文件: `crates/cc-mcp/src/client.rs`
> 当前行数: **2052 行**
> 生成日期: 2026-05-23

---

## 1. 文件概览

`client.rs` 是 MCP 客户端的核心模块，负责与 MCP 服务器通过 stdio / SSE / Streamable HTTP 三种传输协议通信。文件包含以下内容：

| 分类 | 行号范围 | 内容 |
|------|---------|------|
| imports + 常量 | 1–52 | 外部依赖、`STREAMABLE_HTTP_PROTOCOL_VERSION` |
| 类型别名 + Auth 错误 | 54–87 | `PendingRequest`, `PendingRequests`, `McpAuthNeededError` struct/impl, `is_auth_needed_error()` |
| `McpClient` struct | 89–121 | 核心客户端结构体（17 个字段：name, server_name, pending, reader_handle, write_tx, capabilities, connection_state, tools, resources, session_id, auth_provider, shared_event_sink 等） |
| `McpClient::connect()` impl | 123–819 | 核心方法块，包含：`connect()`, `initialize()`, `list_tools()`, `call_tool()`, `list_resources()`, `read_resource()`, `disconnect()`, 以及大量内部辅助方法（stdio spawn, SSE connect, Streamable HTTP connect, JSON-RPC 发送/接收, 初始化握手, 超时处理等） |
| SSE 配置验证 | 821–855 | `validate_sse_config()`, `validate_streamable_http_config()` |
| Streamable HTTP 传输层 | 856–1232 | `StreamableHttpTarget` struct/impl, `StreamableHttpSender` struct/impl, `process_streamable_http_event_stream()`, `handle_streamable_http_sse_event()`, `dispatch_streamable_http_json_message()`, `json_rpc_request_id()` |
| SSE 传输层 | 1234–1559 | `SseConnectTarget` enum/impl, `SsePostTarget` enum, `SseHttpTarget` struct/impl, `RemoteSseHttpTarget` struct/impl, `SseHttpSender` struct/impl, `post_loopback_sse_json()`, `post_remote_https_sse_json()`, `connect_loopback_sse_stream()`, `connect_remote_https_sse_stream()`, `remote_sse_http_client()` |
| HTTP 客户端构建 | 1561–1571 | `streamable_http_client()` |
| SSE Reader 辅助 | 1573–1625 | `spawn_sse_reader()`, `receive_sse_endpoint()` |
| 状态码处理 | 1626–1730 | `handle_sse_event_stream_status()`, `handle_sse_post_status()`, `handle_streamable_http_status()` |
| HTTP 工具函数 | 1731–2050 | `reqwest_header_map()`, `reqwest_error_to_io()`, `parse_authority()`, `parse_port()`, `is_loopback_host()`, `strip_fragment()`, `normalized_http_transport_headers()`, `normalized_http_transport_headers_with_auth()`, `build_sse_get_request()`, `build_sse_post_request()`, `append_user_headers()`, `read_http_response_head()`, 各种 `validate_*()` 函数, `same_origin()`, `is_loopback_url()`, `is_reserved_http_transport_header()`, `is_event_stream_response()`, `response_content_type()`, `redact_url_for_log()`, `Drop for McpClient` |
| 测试模块声明 | 2052 | `mod client_tests;` |

---

## 2. 拆分方案

将 `client.rs` 拆分为目录模块 `client/`，共 **7 个文件**：

```
cc-mcp/src/client/
├── mod.rs               # McpClient struct + 核心方法（connect/initialize/call_tool/disconnect）+ pub use 重导出
├── auth_error.rs        # McpAuthNeededError + is_auth_needed_error()
├── stdio.rs             # stdio 传输：spawn 子进程、stdin/stdout 读写
├── streamable_http.rs   # Streamable HTTP 传输层（StreamableHttpTarget/Sender, event stream 处理）
├── sse.rs               # SSE 传输层（SseConnectTarget/PostTarget/HttpTarget/Sender, loopback/remote）
├── http_utils.rs        # HTTP 工具函数（header 构建、状态码验证、URL 解析验证）
└── client_tests.rs      # 测试（已独立为文件，保持不变）
```

---

## 3. 每个新文件的包含内容

### `mod.rs` (~350 行)
- 模块声明 + `pub use` 重导出
- `PendingRequest`, `PendingRequests` 类型别名 (行 54–55)
- `McpClient` struct 定义 (行 89–121)
- `McpClient` 核心公共方法 impl (行 123–819 的主要骨架):
  - `connect()` — 路由到 stdio/SSE/streamable HTTP
  - `initialize()` — JSON-RPC 初始化握手
  - `list_tools()`, `call_tool()`, `list_resources()`, `read_resource()`
  - `disconnect()`
  - `send_json_rpc()`, `send_notification()` 等内部通信方法
- `Drop for McpClient` impl (行 2039–2050)
- `mod client_tests;`

### `auth_error.rs` (~40 行)
- `McpAuthNeededError` struct (行 58–61)
- `impl fmt::Display for McpAuthNeededError` (行 63–74)
- `impl std::error::Error for McpAuthNeededError` (行 76)
- `pub fn is_auth_needed_error()` (行 78–87)

### `stdio.rs` (~150 行)
- 从 `connect()` impl 中提取 stdio 相关逻辑:
  - 子进程 spawn (`tokio::process::Command`)
  - stdin write 半部
  - stdout read 循环 (`reader_loop`)
  - 进程退出处理
- 保持为 `pub(crate)` 函数或 McpClient 的关联函数

### `streamable_http.rs` (~380 行)
- `StreamableHttpTarget` struct + impl (行 856–868)
- `StreamableHttpSender` struct + impl (行 870–1074)
- `process_streamable_http_event_stream()` (行 1076–1143)
- `handle_streamable_http_sse_event()` (行 1145–1184)
- `dispatch_streamable_http_json_message()` (行 1186–1232)
- `json_rpc_request_id()` (行 1234–1238)
- `streamable_http_client()` (行 1561–1571)
- `handle_streamable_http_status()` (行 1691–1729)

### `sse.rs` (~330 行)
- `SseConnectTarget` enum + impl (行 1241–1265)
- `SsePostTarget` enum (行 1268–1272)
- `SseHttpTarget` struct + impl (行 1274–1341)
- `RemoteSseHttpTarget` struct + impl (行 1343–1376)
- `SseHttpSender` struct + impl (行 1378–1422)
- `post_loopback_sse_json()` (行 1424–1456)
- `post_remote_https_sse_json()` (行 1458–1480)
- `connect_loopback_sse_stream()` (行 1482–1507)
- `connect_remote_https_sse_stream()` (行 1509–1551)
- `remote_sse_http_client()` (行 1553–1559)
- `spawn_sse_reader()` (行 1573–1590)
- `receive_sse_endpoint()` (行 1592–1624)
- `handle_sse_event_stream_status()` (行 1626–1649)
- `handle_sse_post_status()` (行 1651–1689)

### `http_utils.rs` (~330 行)
- `validate_sse_config()` (行 821–836)
- `validate_streamable_http_config()` (行 838–854)
- `reqwest_header_map()` (行 1731–1741)
- `reqwest_error_to_io()` (行 1743–1745)
- `parse_authority()` (行 1747–1770)
- `parse_port()` (行 1772–1775)
- `is_loopback_host()` (行 1777–1779)
- `strip_fragment()` (行 1781–1783)
- `normalized_http_transport_headers()` (行 1785–1799)
- `normalized_http_transport_headers_with_auth()` (行 1801–1815)
- `build_sse_get_request()` (行 1817–1825)
- `build_sse_post_request()` (行 1827–1842)
- `append_user_headers()` (行 1844–1851)
- `read_http_response_head()` (行 1853–1887)
- `validate_sse_url()` (行 1889–1916)
- `validate_streamable_http_url()` (行 1918–1932)
- `validate_header_name()` (行 1934–1949)
- `validate_header_value()` (行 1951–1956)
- `validate_remote_https_url()` (行 1958–1969)
- `validate_session_id()` (行 1971–1976)
- `same_origin()` (行 1978–1982)
- `is_loopback_url()` (行 1984–1986)
- `is_reserved_http_transport_header()` (行 1988–2000)
- `is_event_stream_response()` (行 2002–2006)
- `response_content_type()` (行 2008–2020)
- `redact_url_for_log()` (行 2022–2037)

---

## 4. 模块间依赖关系

```
mod.rs (McpClient struct + 核心方法)
 ├── auth_error.rs        (独立叶子，无内部依赖)
 ├── stdio.rs             (依赖 mod.rs 的 McpClient, super::transport::reader_loop)
 ├── streamable_http.rs   (依赖 mod.rs, http_utils, super::transport::streamable_http_sse_reader_loop)
 ├── sse.rs               (依赖 mod.rs, http_utils, super::transport::sse_reader_loop)
 └── http_utils.rs        (独立叶子，仅依赖 reqwest/url/anyhow)
```

- `auth_error.rs` 和 `http_utils.rs` 是完全独立的叶子模块
- `stdio.rs`, `streamable_http.rs`, `sse.rs` 都需要访问 `McpClient` 的字段（`pending`, `write_tx`, `server_name` 等）
  - 方案 A：将这些函数实现为 `McpClient` 的关联方法（推荐）
  - 方案 B：将需要的字段通过参数传入（破坏封装）
  - **推荐方案 A**：在 `mod.rs` 中 `impl McpClient`，在子模块中通过 `impl McpClient` 块扩展方法
- `super::transport` 模块（已独立）提供 `reader_loop`, `sse_reader_loop`, `streamable_http_sse_reader_loop` 等，被 stdio/sse/streamable_http 分别依赖

---

## 5. 重构步骤（迁移顺序）

1. **创建 `client/` 目录**，将 `client.rs` 改名为 `client/mod.rs`
2. **提取 `auth_error.rs`** — 最小最独立，提取后验证编译
3. **提取 `http_utils.rs`** — 纯工具函数，无内部状态依赖，提取后验证编译
4. **提取 `streamable_http.rs`** — Streamable HTTP 传输层（包含独立的 struct/impl）
5. **提取 `sse.rs`** — SSE 传输层（结构与 streamable_http 类似）
6. **提取 `stdio.rs`** — stdio 传输逻辑
7. **清理 `mod.rs`** — 仅保留 McpClient struct 定义 + 核心公共方法骨架 + pub use
8. **验证 `client_tests.rs` 仍然正确引用** — 测试模块声明保持不变
9. **`cargo build --release`** 验证编译
10. **`cargo test -p cc-mcp`** 验证测试通过

---

## 6. 注意事项

1. **`impl McpClient` 跨文件扩展**: Rust 允许同一 crate 内不同文件对同一 struct 写 `impl` 块，但必须在同一个 crate 中。子模块文件放在 `client/` 目录下属于 `cc-mcp` crate，没问题。
2. **可见性**: `McpClient` 的字段当前是 `pub(super)` 或 crate-private，子模块中的 `impl McpClient` 可以直接访问同 crate 的私有字段。
3. **`PendingRequests` 类型共享**: 被 mod.rs 和 stdio.rs/sse.rs/streamable_http.rs 共用，放在 `mod.rs` 中通过 `use super::PendingRequests` 引用。
4. **transport 模块引用**: 子模块需要 `use super::super::transport::*` 或通过 mod.rs re-export transport 的依赖项。
5. **测试文件 `client_tests.rs`**: 已经独立为文件，`mod client_tests;` 声明保留在 `mod.rs` 中即可。
6. **渐进式重构**: 每步提取后立即 `cargo build`，避免累积错误。
7. **`McpAuthNeededError` 的 `is_auth_needed_error()`**: 该函数被 MCP 命令处理层使用，提取为 `pub` 函数需确保 `mod.rs` 中有 `pub use auth_error::is_auth_needed_error;`。
