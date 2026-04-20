# 测试文档索引

本目录记录 `tests/` 下所有集成测试的作用、运行方式和测试清单。

| 文档 | 内容 |
|------|------|
| [e2e.md](e2e.md) | E2E 黑盒测试 — CLI、环境、工具、压缩、审计、API |
| [pty.md](pty.md) | PTY 终端测试 — 伪终端 UI 渲染、输入、流式、resize |

## 测试总览

| 类别 | 测试文件 | 测试数 | 需要 API Key |
|------|----------|--------|-------------|
| E2E | `e2e_cli.rs` | 16 | 否 |
| E2E | `e2e_env.rs` | 13 | 否 |
| E2E | `e2e_tools.rs` | 31 | 否 |
| E2E | `e2e_compact.rs` | 10 | 否 |
| E2E | `e2e_audit_export.rs` | 6 | 否 |
| E2E | `e2e_session_export.rs` | 9 | 否 |
| E2E | `e2e_services.rs` | 12 | 部分 |
| E2E | `e2e_live_api.rs` | 17 | 是 |
| E2E | `e2e_terminal.rs` | 14 | 部分 |
| PTY | `e2e_pty.rs` | 6 | 部分 |
| PTY | `pty_ui/` | 29 | 部分 |
| | **合计** | **163** | |

## 快速运行

```bash
# 所有 offline 测试
cargo test

# 仅 E2E
cargo test --test e2e_cli --test e2e_env --test e2e_tools

# 仅 PTY UI
cargo test --test pty_ui

# live 测试 (需要 API key)
cargo test -- --ignored
```
