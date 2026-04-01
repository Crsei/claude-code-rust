// Phase 1: 查询状态机 — 完整模块
//
// query loop 是整个系统的核心:
//   while(true) { setup → context_mgmt → api_call → tool_exec → attachments → continue }
//
// 在 Rust 中, TypeScript 的 AsyncGenerator 映射为 impl Stream<Item = QueryYield>
// 使用 async_stream::stream! 宏

pub mod deps;
pub mod token_budget;
pub mod stop_hooks;
pub mod loop_impl;

// 重导出核心函数, 方便外部使用
#[allow(unused_imports)]
pub use loop_impl::query;
#[allow(unused_imports)]
pub use deps::QueryDeps;
