## 执行计划

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `lsp_recommendation_menu.rs::InstallState` 3个变体 | ❌ **保留** — 测试枚举变体；生产 `InstallState` 不同 | 保持 `#[cfg(test)]` |
| 2 | `lsp_recommendation_menu.rs::LspRecommendation` 结构体+impl | ❌ **保留** — 测试构造 | 保持 `#[cfg(test)]` |
| 3 | `lsp_recommendation_menu.rs::render_lsp_recommendation_menu()` | ❌ **保留** — 测试专用渲染 | 保持 `#[cfg(test)]` |
| 4 | `lsp_recommendation_menu.rs::LspRecommendationPromptState` 6个方法 | ❌ **保留** — 测试模拟方法 | 保持 `#[cfg(test)]` |
| 5 | `lsp_recommendation_menu.rs::LspRecommendationChoice::label` 字段+方法 | ❌ **保留** — 测试专用字段 | 保持 `#[cfg(test)]` |

### 需要修改的生产文件
无 — 全部保留。
