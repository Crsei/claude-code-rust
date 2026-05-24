## 执行计划

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `skills_menu.rs::SkillMenuItem::new()` | ❌ **保留** — 测试构造辅助 | 保持 `#[cfg(test)]` |
| 2 | `skills_menu.rs::render_skills_menu()` | ❌ **保留** — 测试专用渲染，生产Skill通过 `CommandSurface::Skills` | 保持 `#[cfg(test)]` |

### 需要修改的生产文件
无 — 全部保留。
