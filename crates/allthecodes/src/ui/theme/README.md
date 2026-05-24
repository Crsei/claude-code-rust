# `theme/` — cfg(test) Audit

除 test 函数以外的 `#[cfg(test)]` 使用情况：

## `mod.rs`
- **Lines 33, 43, 55, 60, 124**: `ThemeName` 枚举方法/常量（`ALL`, `label`, `is_dark`, `as_settings_value`）
- **Lines 499, 511, 519, 537, 542, 557, 562, 568, 576**: `Theme` / `ThemeSetting` 的字段、方法（`setting`, `with_name`, `name`, `set_theme`, `set_setting`, `refresh_auto`, `all_themes`）
- **Lines 665–703**: `write_theme_setting_to_path()` 辅助函数
- **Line 765**: `use super::*;` 导入

## `color.rs`
- **Lines 222–232**: `ColorExt` trait 及 `impl ColorExt for Color`
- **Lines 248–249**: `use` 导入
- **Line 251**: `dark_colors()` 辅助函数
