## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs::pub mod team_status` | ✅ **接入** — `CommandSurface::Team` 存在，但缺少概览页面；`team_status` 显示代理计数/运行状态 | 移除 `#[cfg(test)]`；在 `TeamSurface::render()` 中调用 `render_team_status()` |
| 2 | `teams_dialog.rs::render_teams_dialog()` | ✅ **接入** — 团队对话框渲染，`CommandSurface::Team` 的持有者 | 移除 `#[cfg(test)]`；在 `TeamSurface::handle_key()` 中根据状态切换到 `render_teams_dialog()` |

### 需要修改的生产文件
- `teams/mod.rs`: 移除 L2-3 的 `#[cfg(test)]`
- `teams/teams_dialog.rs`: 移除 L36 的 `#[cfg(test)]`
- `command_surface/surfaces/team.rs`: 在 `render()`/`handle_key()` 中接入 `team_status` 概览和 `teams_dialog` 详情页

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::teams
cargo build --workspace --release
```
