# Computer Use 移植缺口

对应: `claude-code-bun/src/utils/computerUse/`  
Rust 对应: `cc-computer-use/`

## Bun 端规模

| 文件 | 行数 | 说明 |
|------|------|------|
| executor.ts | 728 | macOS ComputerExecutor |
| executorCrossPlatform.ts | 1,154 | Win/Linux 执行器 |
| mcpServer.ts | 107 | MCP 服务器 |
| hostAdapter.ts | 117 | 主机适配器 |
| common.ts | 67 | 共享常量 |
| computerUseLock.ts | 215 | 并发锁 |
| drainRunLoop.ts | 80 | macOS CGEvent drain |
| escHotkey.ts | 55 | 退出热键检测 |
| appNames.ts | 196 | 应用名映射 |
| gates.ts | 69 | 功能开关 |
| inputLoader.ts | 30 | 输入加载器 |
| swiftLoader.ts | 21 | Swift 加载器 |
| toolRendering.tsx | 157 | 工具 UI 渲染 |
| wrapper.tsx | 452 | 包装组件 |
| **platforms/** | | |
| platforms/win32.ts | 984 | Windows 平台 |
| platforms/linux.ts | 517 | Linux 平台 |
| platforms/darwin.ts | 158 | macOS 平台 |
| platforms/types.ts | 153 | 平台类型 |
| **win32/** | | |
| win32/*.ts (13 文件) | ~3,000+ | Win32 特定实现 |
| **合计** | **~14,294** | |

## Rust 已实现

### cc-computer-use/ (完整度 ~90%)
- `input/mod.rs` + `input/darwin.rs` + `input/linux.rs` + `input/win32.rs`: 跨平台输入
- `screenshot/mod.rs` + `screenshot/darwin.rs` + `screenshot/linux.rs` + `screenshot/win32.rs`: 跨平台截图
- `detection.rs`: 平台检测
- `setup.rs`: 设置
- `tools.rs`: 工具定义

## Rust 缺失的主要功能

### 1. Win32 COM 自动化 (Word/Excel) — 缺失
- `win32/comWord.ts` (448 行): Microsoft Word COM 自动化
- `win32/comExcel.ts` (324 行): Microsoft Excel COM 自动化
- **Rust 中无对应** — 操作 Office 应用的能力缺失

### 2. Win32 UI 自动化 (353 行) — 缺失
- `win32/uiAutomation.ts`: Windows UI Automation 树导航
- 辅助功能快照
- 用于跨应用元素识别

### 3. 虚拟光标 (268 行) — 缺失
- `win32/virtualCursor.ts`: Windows 虚拟光标管理
- 用于窗口绑定的光标状态

### 4. 窗口边框和管理 — 缺失
- `win32/windowBorder.ts`: 窗口边框管理
- `win32/shared.ts`: Win32 共享工具
- `win32/inputIndicator.ts`: 输入指示器

### 5. 事件循环 drain (80 行) — 缺失
- `drainRunLoop.ts`: macOS CGEvent drain run loop
- 用于确保输入事件已被系统处理的轮询

### 6. 退出热键 (55 行) — 缺失
- `escHotkey.ts`: 特殊退出热键检测
- 用于紧急中断计算机操作

### 7. 并发锁 (215 行) — 缺失
- `computerUseLock.ts`: 计算机操作并发控制
- 防止多个工具同时操作鼠标/键盘

### 8. 应用名映射 (196 行) — 缺失
- `appNames.ts`: 应用名→可执行文件名映射
- 用于应用启动和管理

### 9. 主机适配器 (117 行) — 缺失
- `hostAdapter.ts`: 桌面应用主机集成

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| macOS 输入 | executor.ts (728 行) | input/darwin.rs | ✅ 已移植 |
| Linux 输入 | executorCrossPlatform.ts | input/linux.rs | ✅ 已移植 |
| Win32 输入 | executorCrossPlatform.ts | input/win32.rs | ✅ 已移植 |
| macOS 截图 | (通过 Swift) | screenshot/darwin.rs | ✅ 已移植 |
| Linux 截图 |  | screenshot/linux.rs | ✅ 已移植 |
| Win32 截图 | (PrintWindow) | screenshot/win32.rs | ✅ 已移植 |
| 平台检测 | platforms/ | detection.rs | ✅ 已移植 |
| 工具定义 | mcpServer.ts | tools.rs | ✅ 已移植 |
| **Word COM 自动化** | **win32/comWord.ts (448 行)** | **无** | **功能缺口** |
| **Excel COM 自动化** | **win32/comExcel.ts (324 行)** | **无** | **功能缺口** |
| **UI 自动化** | **win32/uiAutomation.ts (353 行)** | **无** | **功能缺口** |
| **虚拟光标** | **win32/virtualCursor.ts (268 行)** | **无** | **功能缺口** |
| **并发锁** | **computerUseLock.ts (215 行)** | **无** | **安全缺口** |
| **退出热键** | **escHotkey.ts (55 行)** | **无** | **UX 缺口** |
| 应用名映射 | appNames.ts | 无 | 功能缺口 |
