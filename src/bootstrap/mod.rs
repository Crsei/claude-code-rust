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

// bootstrap/ 处于分阶段迁移中 (见 architecture/bootstrap.md)。
// Phase 1 (骨架) 和 Phase 2 (SessionId) 已完成；
// Phase 3-5 (CWD 收编、计费统计、初始化适配) 尚未集成，
// 因此 model/signal/diagnostics/timing 及 state 中部分字段暂未被外部消费。
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
// Phase 3-5 集成后这些 re-export 将被外部消费
pub use ids::SessionId;
#[allow(unused_imports)]
pub use model::{ModelSetting, ModelStrings, ModelTier};
#[allow(unused_imports)]
pub use signal::Signal;
pub use state::{PROCESS_STATE, init as init_process_state};
#[allow(unused_imports)]
pub use timing::DurationTracker;
