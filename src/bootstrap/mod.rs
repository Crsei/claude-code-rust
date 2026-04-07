//! bootstrap/ — 进程级全局单例层
//!
//! 导入 DAG 叶节点: 任何模块可以依赖 bootstrap，
//! 但 bootstrap 绝不依赖应用层模块 (engine, query, tools, api, ui, ...)。
//!
//! 对应 TypeScript: src/bootstrap/
//!
//! ## 与 AppState 的分工
//!
//! | | ProcessState (bootstrap) | AppState (types/app_state.rs) |
//! |---|---|---|
//! | 生命周期 | 进程级，启动到退出 | 会话级，可跨 QueryEngine 共享 |
//! | 可变性 | 身份信息不可变，统计只增不减 | UI/配置状态随时变更 |
//! | 访问方式 | `PROCESS_STATE` 全局静态 | `Arc<RwLock<AppState>>` 实例传递 |
//! | 内容 | 路径、session ID、计费、诊断 | settings、model、permission context |

// Phase 1-5 迁移已全部完成 (见 architecture/bootstrap.md)。
// SessionId、ProcessState init、CWD 收编、计费/耗时统计均已集成。
// model/signal 类型尚未被外部模块直接消费；diagnostics/timing/state 的
// 部分读取方法尚未被调用 (写入路径已集成)。保留 dead_code 允许。
#[allow(dead_code)]
pub mod diagnostics;
#[allow(dead_code)]
pub mod ids;
#[allow(dead_code)]
pub mod model;
#[allow(dead_code)]
pub mod signal;
#[allow(dead_code)]
pub mod state;
#[allow(dead_code)]
pub mod timing;

// 公开常用类型，方便其他模块 `use crate::bootstrap::*`
pub use ids::SessionId;
#[allow(unused_imports)]
pub use model::{ModelSetting, ModelStrings, ModelTier};
#[allow(unused_imports)]
pub use signal::Signal;
pub use state::{PROCESS_STATE, init as init_process_state};
#[allow(unused_imports)]
pub use timing::DurationTracker;
