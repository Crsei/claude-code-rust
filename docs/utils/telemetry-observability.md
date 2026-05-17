# 遥测/可观测性移植缺口

对应: `claude-code-bun/src/utils/telemetry/`  
Rust 对应: `cc-observability/`

## Bun 端规模

| 文件 | 行数 | 说明 |
|------|------|------|
| instrumentation.ts | 831 | OpenTelemetry 初始化设置 |
| sessionTracing.ts | 927 | 会话追踪 API |
| perfettoTracing.ts | 1,120 | Perfetto 高性能追踪 |
| betaSessionTracing.ts | 491 | Beta 会话追踪 |
| events.ts | 75 | OTEL 事件日志 |
| pluginTelemetry.ts | 289 | 插件遥测 |
| bigqueryExporter.ts | 252 | BigQuery 导出器 |
| logger.ts | 26 | OTEL 诊断日志 |
| skillLoadedEvent.ts | 39 | 技能加载事件 |
| **合计** | **~4,050** | |

## Rust 已实现

### cc-observability/ (完整度 ~20%)
- `lib.rs`: 模块声明
- `event.rs`: 基础事件类型
- `context.rs`: 可观测性上下文
- `sink.rs`: 事件接收器

## Rust 缺失的主要功能

### 1. OpenTelemetry 完整集成 (831 行) — 完全缺失
- OTLP/Prometheus 导出器动态加载
- 资源检测 (env, host, OS)
- Tracer/Meter/Logger 提供者初始化
- 代理支持
- mTLS 配置
- 多导出器: OTLP, Prometheus, BigQuery, Console

### 2. 会话追踪 (927 行) — 完全缺失
- Interaction spans (每个用户输入的根 span)
- LLM 请求 spans
- Tool spans (blocked_on_user, execution)
- Hook spans
- AsyncLocalStorage 上下文管理
- Beta 增强遥测集成

### 3. Perfetto 追踪 (1,120 行) — 完全缺失
- 高性能追踪事件发射
- Interaction/LLM/Tool/User input span 管理
- 追踪文件输出
- ConfigScreenshots

### 4. Beta 会话追踪 (491 行) — 完全缺失
- 详细属性追踪
- Interaction/LLM request/response/tool 属性
- 内容截断隐私保护
- PII 标记

### 5. BigQuery 导出器 (252 行) — 完全缺失
- BigQuery 指标导出

### 6. 插件遥测 (289 行) — 完全缺失
- 双列隐私模式 (raw + redacted)
- 插件 ID 哈希 (固定盐)

## Rust vs Bun 遥测策略差异

Bun 版本有完整的 OpenTelemetry 集成，而 Rust 版本选择了不同的路线：
- Bun: 生产级 OTel 集成，多导出器，Perfetto 高性能追踪
- Rust: 基本事件类型定义，无 OTel 导出器集成

这可能是有意的简化 — Rust 版本可能计划使用不同的遥测基础设施。

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| 事件类型 | 完整 | event.rs + context.rs | ✅ 部分 |
| 事件接收 | 完整 | sink.rs | ✅ 部分 |
| **OTel 初始化** | **831 行** | **无** | **核心缺口** |
| **会话追踪** | **927 行** | **无** | **核心缺口** |
| **Perfetto** | **1,120 行** | **无** | **功能缺口** |
| **Beta 追踪** | **491 行** | **无** | **功能缺口** |
| **BigQuery** | **252 行** | **无** | **功能缺口** |
| **插件遥测** | **289 行** | **无** | **功能缺口** |
